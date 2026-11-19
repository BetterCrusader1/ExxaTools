//! Everything ExxaTools writes into Vapor Forge's directories.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::lua;
use crate::paths;

pub fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_secs())
        .unwrap_or(0);
    format!("{now}")
}

pub fn script_path(appid: u64) -> PathBuf {
    paths::vf_scripts().join(format!("exxatools-{appid}.lua"))
}

pub fn disabled_path(appid: u64) -> PathBuf {
    paths::vf_scripts().join(format!("exxatools-{appid}.lua.disabled"))
}

pub fn provider_path() -> PathBuf {
    paths::vf_scripts().join(lua::PROVIDER_FILE)
}

pub fn write_script(appid: u64, text: &str, enabled: bool) -> Result<PathBuf, String> {
    std::fs::create_dir_all(paths::vf_scripts())
        .map_err(|error| format!("cannot create {}: {error}", paths::vf_scripts().display()))?;
    let target = if enabled {
        script_path(appid)
    } else {
        disabled_path(appid)
    };
    let other = if enabled {
        disabled_path(appid)
    } else {
        script_path(appid)
    };
    std::fs::write(&target, text)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    let _ = std::fs::remove_file(other);
    Ok(target)
}

pub fn remove_script(appid: u64) -> bool {
    let mut removed = false;
    for path in [script_path(appid), disabled_path(appid)] {
        if path.exists() && std::fs::remove_file(&path).is_ok() {
            removed = true;
        }
    }
    removed
}

pub fn toggle_script(appid: u64, enabled: bool) -> Option<PathBuf> {
    let (from, to) = if enabled {
        (disabled_path(appid), script_path(appid))
    } else {
        (script_path(appid), disabled_path(appid))
    };
    if !from.is_file() {
        return if to.is_file() { Some(to) } else { None };
    }
    std::fs::rename(&from, &to).ok()?;
    Some(to)
}

pub fn write_provider(text: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(paths::vf_scripts())
        .map_err(|error| format!("cannot create {}: {error}", paths::vf_scripts().display()))?;
    let legacy = paths::vf_scripts().join("zz-vapordepot-provider.lua");
    if legacy.exists() {
        let _ = std::fs::remove_file(legacy);
    }
    let target = provider_path();
    std::fs::write(&target, text)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    Ok(target)
}

pub fn remove_provider() -> bool {
    let path = provider_path();
    if path.exists() {
        return std::fs::remove_file(&path).is_ok();
    }
    false
}

pub fn provider_state() -> Value {
    let path = provider_path();
    let installed = path.is_file();
    json!({
        "installed": installed,
        "path": path.display().to_string(),
        "mtime": std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|delta| delta.as_secs()),
    })
}
