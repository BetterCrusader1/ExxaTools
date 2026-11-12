//! Plugin registry: manifests on disk, validation, enable state, CLI and API views.
//!
//! A plugin is a folder under `<state dir>/plugins/<id>` with an `exxa-plugin.json`
//! manifest and a JavaScript entry file. The manifest is the contract: the host reads
//! it, refuses anything that points outside its own folder, and only injects entries it
//! has validated. Plugin code itself runs in an isolated CDP world; permissions listed
//! here decide which host calls Rust will answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MANIFEST: &str = "exxa-plugin.json";
pub const MAX_ENTRY_BYTES: u64 = 512 * 1024;
pub const MAX_PLUGINS: usize = 20;
pub const PERMISSIONS: [&str; 4] = ["config", "plugin-data", "ui", "steam"];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub entry: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub entry: String,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub valid: bool,
    pub error: Option<String>,
    pub dir: String,
}

/// The folder plugins are installed into; sits next to `state.json`.
pub fn plugins_dir() -> PathBuf {
    crate::paths::state_dir().join("plugins")
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-'))
        && id
            .chars()
            .next()
            .map(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
            .unwrap_or(false)
}

/// A manifest entry has to stay inside the plugin folder: no absolute paths, no
/// `..`, no empty segments. Symlinks are resolved separately when the file is read.
pub fn safe_entry(entry: &str) -> bool {
    if entry.is_empty() || entry.len() > 128 || entry.starts_with('/') || entry.contains('\\') {
        return false;
    }
    Path::new(entry)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
}

fn validate(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    if manifest.schema != 1 {
        return Err(format!("unsupported schema {}", manifest.schema));
    }
    if !valid_id(&manifest.id) {
        return Err("id must be lowercase letters, digits, dot, dash or underscore".into());
    }
    if manifest.name.trim().is_empty() {
        return Err("name is required".into());
    }
    if !safe_entry(&manifest.entry) {
        return Err("entry must be a relative path inside the plugin folder".into());
    }
    let unknown: Vec<&String> = manifest
        .permissions
        .iter()
        .filter(|permission| !PERMISSIONS.contains(&permission.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(format!("unknown permission: {}", unknown[0]));
    }
    let entry = dir.join(&manifest.entry);
    let canonical_dir = dir.canonicalize().map_err(|error| error.to_string())?;
    let canonical_entry = entry
        .canonicalize()
        .map_err(|_| format!("entry not found: {}", manifest.entry))?;
    if !canonical_entry.starts_with(&canonical_dir) {
        return Err("entry escapes the plugin folder".into());
    }
    let meta = std::fs::metadata(&canonical_entry).map_err(|error| error.to_string())?;
    if !meta.is_file() {
        return Err("entry is not a file".into());
    }
    if meta.len() > MAX_ENTRY_BYTES {
        return Err(format!("entry is larger than {} KiB", MAX_ENTRY_BYTES / 1024));
    }
    Ok(())
}

fn enabled_map() -> BTreeMap<String, bool> {
    crate::state::load()
        .map(|state| state.plugins)
        .unwrap_or_default()
}

fn read_plugin(dir: &Path, enabled: &BTreeMap<String, bool>) -> Option<Plugin> {
    let raw = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let manifest: Manifest = match serde_json::from_str(&raw) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Some(Plugin {
                id: dir.file_name()?.to_string_lossy().to_string(),
                name: String::new(),
                version: String::new(),
                author: String::new(),
                description: String::new(),
                entry: String::new(),
                permissions: Vec::new(),
                enabled: false,
                valid: false,
                error: Some(format!("manifest is not valid JSON: {error}")),
                dir: dir.display().to_string(),
            })
        }
    };
    let error = validate(dir, &manifest).err();
    let id = manifest.id.clone();
    Some(Plugin {
        id: id.clone(),
        name: manifest.name,
        version: manifest.version,
        author: manifest.author,
        description: manifest.description,
        entry: manifest.entry,
        permissions: manifest.permissions,
        enabled: enabled.get(&id).copied().unwrap_or(true) && error.is_none(),
        valid: error.is_none(),
        error,
        dir: dir.display().to_string(),
    })
}

pub fn scan() -> Result<Vec<Plugin>, String> {
    let root = plugins_dir();
    std::fs::create_dir_all(&root).map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let enabled = enabled_map();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    let mut plugins: Vec<Plugin> = dirs.iter().filter_map(|dir| read_plugin(dir, &enabled)).collect();
    plugins.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(plugins)
}

pub fn list_json() -> Value {
    let plugins = scan().unwrap_or_default();
    let valid = plugins.iter().filter(|plugin| plugin.valid).count();
    let enabled = plugins.iter().filter(|plugin| plugin.enabled).count();
    json!({
        "dir": plugins_dir().display().to_string(),
        "limit": MAX_PLUGINS,
        "counts": { "total": plugins.len(), "valid": valid, "enabled": enabled },
        "plugins": plugins,
    })
}

pub fn set_enabled(id: &str, enabled: bool) -> Result<Value, String> {
    if !valid_id(id) {
        return Err("invalid plugin id".into());
    }
    if !plugins_dir().join(id).is_dir() {
        return Err(format!("no plugin named {id}"));
    }
    crate::state::update(|state| {
        state.plugins.insert(id.to_string(), enabled);
        Ok(())
    })?;
    Ok(list_json())
}

pub fn remove(id: &str) -> Result<Value, String> {
    if !valid_id(id) {
        return Err("invalid plugin id".into());
    }
    let root = plugins_dir().canonicalize().map_err(|error| error.to_string())?;
    let target = root.join(id);
    if !target.is_dir() {
        return Err(format!("no plugin named {id}"));
    }
    let canonical = target.canonicalize().map_err(|error| error.to_string())?;
    if !canonical.starts_with(&root) {
        return Err("refusing to remove a path outside the plugins folder".into());
    }
    std::fs::remove_dir_all(&canonical).map_err(|error| format!("cannot remove {}: {error}", canonical.display()))?;
    let _ = crate::state::update(|state| {
        state.plugins.remove(id);
        Ok(())
    });
    Ok(json!({ "removed": id, "plugins": list_json() }))
}

/// Enabled, valid plugins with their JavaScript entry, ready for injection.
pub fn frontends() -> Vec<(Plugin, String)> {
    let plugins = scan().unwrap_or_default();
    let mut out = Vec::new();
    for plugin in plugins.into_iter().filter(|plugin| plugin.valid && plugin.enabled) {
        if out.len() >= MAX_PLUGINS {
            break;
        }
        let path = Path::new(&plugin.dir).join(&plugin.entry);
        if let Ok(source) = std::fs::read_to_string(&path) {
            out.push((plugin, source));
        }
    }
    out
}

/// Answer one bridged call. The permission check lives here, not in the page:
/// plugin JavaScript can lie about what it declared, the manifest cannot.
pub fn dispatch(plugin_id: &str, method: &str, params: &Value) -> Result<Value, String> {
    if !valid_id(plugin_id) {
        return Err("invalid plugin id".into());
    }
    let plugins = scan()?;
    let plugin = plugins
        .iter()
        .find(|plugin| plugin.id == plugin_id && plugin.valid && plugin.enabled)
        .ok_or_else(|| format!("plugin {plugin_id} is not enabled"))?;
    let permission = match method {
        "config.get" | "config.set" => "config",
        "steam.status" => "steam",
        _ => return Err(format!("unknown method: {method}")),
    };
    if !plugin.permissions.iter().any(|entry| entry == permission) {
        return Err(format!("permission denied: {permission}"));
    }
    match method {
        "config.get" => {
            let key = params.get("key").and_then(Value::as_str).unwrap_or("");
            if key.is_empty() || key.len() > 64 {
                return Err("config key must be 1..64 characters".into());
            }
            let state = crate::state::load()?;
            Ok(state
                .plugin_config
                .get(plugin_id)
                .and_then(|store| store.get(key))
                .cloned()
                .unwrap_or(Value::Null))
        }
        "config.set" => {
            let key = params.get("key").and_then(Value::as_str).unwrap_or("");
            if key.is_empty() || key.len() > 64 {
                return Err("config key must be 1..64 characters".into());
            }
            let value = params.get("value").cloned().unwrap_or(Value::Null);
            if serde_json::to_string(&value).map(|text| text.len() > 4096).unwrap_or(true) {
                return Err("config value is too large".into());
            }
            crate::state::update(|state| {
                state
                    .plugin_config
                    .entry(plugin_id.to_string())
                    .or_default()
                    .insert(key.to_string(), value);
                Ok(())
            })?;
            Ok(json!(true))
        }
        "steam.status" => {
            let state = crate::state::load().unwrap_or_default();
            Ok(json!({
                "steam_running": crate::steamctl::is_running(),
                "engine": crate::engine::detect(),
                "plans": state.apps.len(),
            }))
        }
        _ => Err(format!("unknown method: {method}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{safe_entry, valid_id};

    #[test]
    fn ids_are_folder_safe() {
        for good in ["hello", "exxatools.core", "a_b-c", "p1"] {
            assert!(valid_id(good), "{good}");
        }
        for bad in ["", "Hello", "../x", "a/b", "-lead", ".dot", "a b"] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn entries_stay_inside() {
        for good in ["frontend.js", "js/main.js", "dist/index.js"] {
            assert!(safe_entry(good), "{good}");
        }
        for bad in ["", "/etc/passwd", "../x.js", "a/../../b.js", "a\\b.js"] {
            assert!(!safe_entry(bad), "{bad}");
        }
    }
}