//! On-disk state: depot keys, saved apps, cached manifest codes, settings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
    pub settings: Settings,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            depot_keys: BTreeMap::new(),
            apps: BTreeMap::new(),
            codes: BTreeMap::new(),
            settings: Settings::default(),
        }
    }
}

pub fn state_dir() -> PathBuf {
    crate::paths::state_dir()
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

pub fn save(state: &State) -> Result<(), String> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, format!("{text}\n")).map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|error| format!("cannot replace {}: {error}", path.display()))
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

pub fn key_for(state: &State, depot_id: u64) -> Option<&String> {
    state.depot_keys.get(&depot_id.to_string())
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