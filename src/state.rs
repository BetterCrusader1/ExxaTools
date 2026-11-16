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
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            manifest_providers: [
                "steamrun",
                "20770407",
                "wudrm",
                "manifestdex",
                "opensteamtool",
            ]
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
            theme: "dark".into(),
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
    /// Plugin id -> enabled. Missing ids default to enabled.
    #[serde(default)]
    pub plugins: BTreeMap<String, bool>,
    /// Plugin id -> its small key/value store, written through the host bridge.
    #[serde(default)]
    pub plugin_config: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
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
            plugins: BTreeMap::new(),
            plugin_config: BTreeMap::new(),
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
        // Migration writes too. Recheck under both writer locks so a first
        // read cannot replace state another thread or process just created.
        return with_write_lock(&path, || load_locked(&path));
    }
    read_state(&path)
}

fn load_locked(path: &Path) -> Result<State, String> {
    if !path.is_file() {
        migrate_legacy(path)?;
    }
    read_state(path)
}

fn read_state(path: &Path) -> Result<State, String> {
    if !path.is_file() {
        return Ok(State::default());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    // A file written before permissions were enforced is repaired on the next read.
    if let Some(parent) = path.parent() {
        let _ = lock_down(parent, 0o700);
    }
    let _ = lock_down(path, 0o600);
    serde_json::from_str(&text).map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

// Called only with both writer locks held. Publish migration by rename too:
// lock-free readers must never see a partially copied legacy state.
fn migrate_legacy(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else { return Ok(()) };
    let Some(root) = parent.parent() else { return Ok(()) };
    let legacy = root.join("vapordepot/state.json");
    if !legacy.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&legacy)
        .map_err(|error| format!("cannot read {}: {error}", legacy.display()))?;
    write_state_locked(path, &text)
}

/// Keeps the last few versions of the state next to it, so a bad import or a
/// half-written plan never costs the keys a user collected by hand.
fn snapshot(path: &Path) -> std::io::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let dir = parent.join("backups");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let target = dir.join(format!("state-{stamp}.json"));
    std::fs::copy(path, &target)?;
    let _ = lock_down(&dir, 0o700);
    let _ = lock_down(&target, 0o600);
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
    state_path()
        .parent()
        .map(|path| path.join("backups"))
        .unwrap_or_else(|| PathBuf::from("backups"))
}

/// Always acquire the process mutex before the persistent sidecar file lock.
/// The state file itself is replaced by rename, so it cannot be the lock inode.
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static TEMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn with_write_lock<T>(path: &Path, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| "state lock is poisoned".to_string())?;
    let parent = path.parent().ok_or_else(|| "state path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    lock_down(parent, 0o700)?;
    let lock_path = path.with_extension("lock");
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock_file = options.open(&lock_path)
        .map_err(|error| format!("cannot open {}: {error}", lock_path.display()))?;
    lock_down(&lock_path, 0o600)?;
    lock_file.lock()
        .map_err(|error| format!("cannot lock {}: {error}", lock_path.display()))?;
    // Never rename or delete this sidecar. Closing releases the OS lock before
    // the process mutex is dropped, including error returns and unwinding.
    f()
}

/// Replaces the whole state. A stale snapshot passed here can still overwrite
/// newer changes; use `update` for every read-modify-write operation.
pub fn save(state: &State) -> Result<(), String> {
    let path = state_path();
    with_write_lock(&path, || save_locked(state, &path))
}

/// One cross-process transaction: both locks cover the read, change and write.
/// The closure must not call load/save/update/ensure_token or do network I/O;
/// it edits only the supplied fresh state, and an error discards its changes.
pub fn update<T>(f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
    let path = state_path();
    with_write_lock(&path, || {
        let mut state = load_locked(&path)?;
        let out = f(&mut state)?;
        save_locked(&state, &path)?;
        Ok(out)
    })
}

fn save_locked(state: &State, path: &Path) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
    write_state_locked(path, &format!("{text}\n"))
}

fn write_state_locked(path: &Path, text: &str) -> Result<(), String> {
    let _ = snapshot(path);
    let parent = path
        .parent()
        .ok_or_else(|| "state path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    lock_down(parent, 0o700)?;
    // A unique temporary name: a shared `state.tmp` raced between writers.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_nanos())
        .unwrap_or(0);
    let seq = TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = parent.join(format!(".state-{}-{nanos}-{seq}.tmp", std::process::id()));
    std::fs::write(&temp, text)
        .map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    lock_down(&temp, 0o600)?;
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("cannot replace {}: {error}", path.display()));
    }
    lock_down(path, 0o600)
}

#[cfg(unix)]
fn lock_down(path: &std::path::Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| format!("cannot set mode {mode:o} on {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn lock_down(_path: &std::path::Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

/// One shape for an app as every surface shows it: the library route, the status
/// route and the Steam panel all read these fields, so they cannot drift apart.
pub fn app_summary(state: &State, record: &serde_json::Value) -> serde_json::Value {
    let appid = record
        .get("appid")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let depots = record
        .get("depots")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let missing: Vec<u64> = depots
        .iter()
        .filter(|depot| {
            depot
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        })
        .filter_map(|depot| depot.get("depot_id").and_then(serde_json::Value::as_u64))
        .filter(|id| !state.depot_keys.contains_key(&id.to_string()))
        .collect();
    let script = crate::vfsys::script_path(appid);
    let status = if crate::vfsys::disabled_path(appid).exists() && !script.exists() {
        "disabled"
    } else if !script.exists() {
        "planned"
    } else if missing.is_empty() {
        "ready"
    } else {
        "keys_missing"
    };
    serde_json::json!({
        "appid": appid,
        "name": record.get("name").cloned().unwrap_or(serde_json::Value::Null),
        "branch": record.get("branch").cloned().unwrap_or_else(|| serde_json::json!("public")),
        "installed": crate::vfsys::script_path(appid).exists(),
        "depot_count": depots.len(),
        "missing_keys": missing,
        "dlc": record.get("dlc").cloned().unwrap_or_else(|| serde_json::json!([])),
        "updated": record.get("updated").cloned().unwrap_or(serde_json::Value::Null),
        "state": status,
    })
}

/// The server keeps one token in state so a restarted UI keeps working, and a
/// fresh one appears only when the file has none.
pub fn ensure_token() -> Result<String, String> {
    let state = load()?;
    if state.token.len() == 64 {
        return Ok(state.token);
    }
    let mut seed = format!(
        "{}-{}-{:?}",
        std::process::id(),
        super::vfsys::stamp(),
        std::time::SystemTime::now()
    );
    let mut token = String::with_capacity(64);
    while token.len() < 64 {
        let digest = sha2::Sha256::digest(seed.as_bytes());
        token.push_str(&format!("{digest:x}"));
        seed.push('x');
    }
    token.truncate(64);
    update(|fresh| {
        // Another initializer may have won while this token was generated.
        if fresh.token.len() != 64 {
            fresh.token = token;
        }
        Ok(fresh.token.clone())
    })
}

/// A depot key is 64 hex characters; anything else is not a key.
pub fn valid_key(value: &str) -> Option<String> {
    let text: String = value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if text.len() == 64 && text.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(text)
    } else {
        None
    }
}

pub fn set_key(state: &mut State, depot_id: u64, key: Option<&str>) {
    match key {
        Some(value) => {
            state
                .depot_keys
                .insert(depot_id.to_string(), value.to_string());
        }
        None => {
            state.depot_keys.remove(&depot_id.to_string());
        }
    }
}
/// Remember a branch gid we observed, newest first, deduplicated, capped.
pub fn record_gid(
    state: &mut State,
    appid: u64,
    depot_id: u64,
    gid: &str,
    size: Option<u64>,
    branch: &str,
) -> bool {
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
        assert_eq!(
            entries[0].get("gid").and_then(serde_json::Value::as_str),
            Some("222")
        );
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

    #[test]
    fn a_disabled_depot_is_not_reported_as_missing_a_key() {
        let state = State::default();
        let record = json!({
            "appid": 620,
            "depots": [
                {"depot_id": 621, "enabled": false},
                {"depot_id": 622, "enabled": true}
            ]
        });
        let view = app_summary(&state, &record);
        assert_eq!(view["missing_keys"], json!([622]));
        assert_eq!(view["depot_count"], 2);
    }
}
