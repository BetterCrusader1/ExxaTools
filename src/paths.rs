//! Every path the tool touches, resolved once.

use std::env;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/".into()))
}

pub fn xdg(name: &str, fallback: &str) -> PathBuf {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => home().join(fallback),
    }
}

pub fn config_home() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}

pub fn cache_home() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache")
}

/// The checkout the binary lives in: walk up until `web/index.html` shows up.
pub fn project_dir() -> PathBuf {
    if let Ok(value) = env::var("EXXATOOLS_PROJECT") {
        return PathBuf::from(value);
    }
    if let Ok(exe) = env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..4 {
            let Some(current) = dir else { break };
            if current.join("web/index.html").is_file() {
                return current;
            }
            dir = current.parent().map(Path::to_path_buf);
        }
    }
    for installed in ["/usr/share/exxatools", "/usr/local/share/exxatools"] {
        let candidate = PathBuf::from(installed);
        if candidate.join("web/index.html").is_file() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn installed_share() -> PathBuf {
    PathBuf::from("/usr/share/exxatools")
}

pub fn web_dir() -> PathBuf {
    let local = project_dir().join("web");
    if local.is_dir() {
        return local;
    }
    installed_share().join("web")
}


pub fn state_dir() -> PathBuf {
    match env::var("EXXATOOLS_HOME") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => config_home().join("exxatools"),
    }
}

pub fn state_path() -> PathBuf {
    state_dir().join("state.json")
}

pub fn vf_config_dir() -> PathBuf {
    match env::var("EXXATOOLS_VF_HOME") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => config_home().join("vapor-forge"),
    }
}

pub fn vf_config() -> PathBuf {
    vf_config_dir().join("config.toml")
}

pub fn vf_scripts() -> PathBuf {
    vf_config_dir().join("scripts")
}

pub fn vf_data() -> PathBuf {
    data_home().join("vapor-forge")
}

pub fn engine_cache() -> PathBuf {
    cache_home().join("exxatools/engine")
}

pub fn engine_meta() -> PathBuf {
    vf_data().join("exxatools-engine.json")
}

pub fn steam_log() -> PathBuf {
    data_home().join("Steam/logs/console-linux.txt")
}

/// systemd drop-ins the Vapor Forge installer writes for the Steam unit.
pub fn systemd_dropins() -> Vec<PathBuf> {
    let base = config_home().join("systemd/user");
    let names = [
        "app-steam@autostart.service.d/vapor-forge.conf",
        "app-steam.service.d/vapor-forge.conf",
    ];
    names.iter().map(|name| base.join(name)).collect()
}