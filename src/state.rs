//! On-disk state: depot keys, saved apps, cached manifest codes, settings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::Digest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub manifest_providers: Vec<String>,
    pub key_source_url: String,
    pub country: String,
    pub default_os: String,
    pub default_source: String,
    pub sources: BTreeMap<String, serde_json::Value>,
    pub hubcap_key: String,
    pub ryuu_key: String,
    pub plan_sources: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            manifest_providers: ["steamrun", "20770407", "wudrm", "manifestdex", "opensteamtool"]
                .iter()
                .map(|value| value.to_string())
                .collect(),
            key_source_url: String::new(),
            country: "us".into(),
            default_os: "linux".into(),
            default_source: "auto".into(),
            sources: BTreeMap::new(),
            hubcap_key: String::new(),
            ryuu_key: String::new(),
            plan_sources: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub depot_keys: BTreeMap<String, String>,
    pub apps: BTreeMap<String, serde_json::Value>,
    pub codes: BTreeMap<String, u64>,
    /// appid -> depot -> recent branch gids, newest first
    pub build_history: BTreeMap<String, BTreeMap<String, Vec<serde_json::Value>>>,
    pub settings: Settings,
    /// Shared secret for the local API; empty until the server first runs.
    #[serde(default)]
    pub token: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            depot_keys: BTreeMap::new(),
            apps: BTreeMap::new(),
            codes: BTreeMap::new(),
            build_history: BTreeMap::new(),
            settings: Settings::default(),
            token: String::new(),
        }
    }
}

pub fn state_path() -> PathBuf {
    crate::paths::state_path()
}

pub fn load() -> Result<State, String> {
    let path = state_path();
    if !path.is_file() {
        migrate_legacy(&path);
    }
    if !path.is_file() {
        return Ok(State::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn migrate_legacy(path: &Path) {
    let Some(parent) = path.parent() else { return };
    let Some(root) = parent.parent() else { return };
    let legacy = root.join("vapordepot/state.json");
    if !legacy.is_file() {
        return;
    }
    if let Ok(text) = std::fs::read_to_string(&legacy) {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(path, text);
    }
}

/// Keeps the last few versions of the state next to it, so a bad import or a
/// half-written plan never costs the keys a user collected by hand.
fn snapshot(path: &Path) -> std::io::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let Some(parent) = path.parent() else { return Ok(()) };
    let dir = parent.join("backups");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let target = dir.join(format!("state-{stamp}.json"));
    std::fs::copy(path, target)?;
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|entry| entry.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect();
    entries.sort();
    while entries.len() > 5 {
        if let Some(oldest) = entries.first() {
            let _ = std::fs::remove_file(oldest);
        }
        entries.remove(0);
    }
    Ok(())
}

pub fn backup_dir() -> PathBuf {
    state_path().parent().map(|path| path.join("backups")).unwrap_or_else(|| PathBuf::from("backups"))
}

pub fn save(state: &State) -> Result<(), String> {
    let path = state_path();
    let _ = snapshot(&path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, format!("{text}\n")).map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|error| format!("cannot replace {}: {error}", path.display()))
}

/// One shape for an app as every surface shows it: the library route, the status
/// route and the Steam panel all read these fields, so they cannot drift apart.
pub fn app_summary(state: &State, record: &serde_json::Value) -> serde_json::Value {
    let appid = record.get("appid").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let depots = record.get("depots").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    let missing: Vec<u64> = depots
        .iter()
        .filter_map(|depot| depot.get("depot_id").and_then(serde_json::Value::as_u64))
        .filter(|id| !state.depot_keys.contains_key(&id.to_string()))
        .collect();
    serde_json::json!({
        "appid": appid,
        "name": record.get("name").cloned().unwrap_or(serde_json::Value::Null),
        "branch": record.get("branch").cloned().unwrap_or_else(|| serde_json::json!("public")),
        "installed": crate::vfsys::script_path(appid).exists(),
        "depot_count": depots.len(),
        "missing_keys": missing,
        "dlc": record.get("dlc").cloned().unwrap_or_else(|| serde_json::json!([])),
        "updated": record.get("updated").cloned().unwrap_or(serde_json::Value::Null),
    })
}

/// The server keeps one token in state so a restarted UI keeps working, and a
/// fresh one appears only when the file has none.
pub fn ensure_token() -> Result<String, String> {
    let mut state = load()?;
    if state.token.len() == 64 {
        return Ok(state.token);
    }
    let mut seed = format!("{}-{}-{:?}", std::process::id(), super::vfsys::stamp(), std::time::SystemTime::now());
    let mut token = String::with_capacity(64);
    while token.len() < 64 {
        let digest = sha2::Sha256::digest(seed.as_bytes());
        token.push_str(&format!("{digest:x}"));
        seed.push('x');
    }
    token.truncate(64);
    state.token = token.clone();
    save(&state)?;
    Ok(token)
}

/// A depot key is 64 hex characters; anything else is not a key.
pub fn valid_key(value: &str) -> Option<String> {
    let text: String = value.trim().to_lowercase().chars().filter(|ch| !ch.is_whitespace()).collect();
    if text.len() == 64 && text.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(text)
    } else {
        None
    }
}

pub fn set_key(state: &mut State, depot_id: u64, key: Option<&str>) {
    match key {
        Some(value) => {
            state.depot_keys.insert(depot_id.to_string(), value.to_string());
        }
        None => {
            state.depot_keys.remove(&depot_id.to_string());
        }
    }
}
/// Remember a branch gid we observed, newest first, deduplicated, capped.
pub fn record_gid(state: &mut State, appid: u64, depot_id: u64, gid: &str, size: Option<u64>, branch: &str) -> bool {
    if gid.is_empty() {
        return false;
    }
    let depots = state.build_history.entry(appid.to_string()).or_default();
    let entries = depots.entry(depot_id.to_string()).or_default();
    if let Some(last) = entries.first() {
        if last.get("gid").and_then(serde_json::Value::as_str) == Some(gid) {
            return false;
        }
    }
    entries.insert(
        0,
        serde_json::json!({
            "gid": gid,
            "size": size,
            "branch": branch,
            "seen_at": crate::vfsys::stamp(),
        }),
    );
    entries.truncate(20);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_dedupes() {
        let mut state = State::default();
        assert!(record_gid(&mut state, 620, 621, "111", Some(10), "public"));
        assert!(!record_gid(&mut state, 620, 621, "111", Some(10), "public"));
        assert!(record_gid(&mut state, 620, 621, "222", Some(11), "public"));
        let entries = state.build_history["620"]["621"].clone();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].get("gid").and_then(serde_json::Value::as_str), Some("222"));
    }

    #[test]
    fn caps_history() {
        let mut state = State::default();
        for index in 0..30 {
            record_gid(&mut state, 1, 2, &format!("{index}"), None, "public");
        }
        assert_eq!(state.build_history["1"]["2"].len(), 20);
    }

    #[test]
    fn rejects_empty_gid() {
        let mut state = State::default();
        assert!(!record_gid(&mut state, 1, 2, "", None, "public"));
    }
}


#[cfg(test)]
mod summary_tests {
    use super::{app_summary, State};
    use serde_json::json;

    #[test]
    fn summary_reports_missing_keys_and_count() {
        let mut state = State::default();
        state.depot_keys.insert("621".into(), "a".repeat(64));
        let record = json!({
            "appid": 620,
            "name": "Portal 2",
            "branch": "public",
            "depots": [
                {"depot_id": 621, "gid": "1"},
                {"depot_id": 622, "gid": "2"}
            ],
            "dlc": [323180],
        });
        let view = app_summary(&state, &record);
        assert_eq!(view["appid"], 620);
        assert_eq!(view["depot_count"], 2);
        assert_eq!(view["missing_keys"], json!([622]));
        assert_eq!(view["dlc"], json!([323180]));
        assert_eq!(view["branch"], "public");
    }
}
