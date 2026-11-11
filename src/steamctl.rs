//! Steam client control: roots, pid, running state, LD_AUDIT, restart.

use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn root() -> Option<PathBuf> {
    if let Ok(steam) = steamlocate::SteamDir::locate() {
        return Some(steam.path().to_path_buf());
    }
    let home = crate::paths::home();
    let candidates = [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".steam/root"),
        home.join("Steam"),
    ];
    for candidate in &candidates {
        if candidate.join("config/loginusers.vdf").is_file() {
            return Some(candidate.clone());
        }
    }
    candidates.into_iter().find(|path| path.join("steam.sh").is_file() || path.join("ubuntu12_64").is_dir())
}

pub fn pid() -> Option<i32> {
    let root = root()?;
    let text = std::fs::read_to_string(root.join("steam.pid")).ok()?;
    let value: i32 = text.trim().parse().ok()?;
    if PathBuf::from(format!("/proc/{value}")).exists() {
        Some(value)
    } else {
        None
    }
}

pub fn is_running() -> bool {
    if pid().is_some() {
        return true;
    }
    Command::new("pgrep")
        .args(["-x", "steam"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn audit_loaded() -> serde_json::Value {
    let target = pid().or_else(|| {
        let output = Command::new("pgrep").args(["-f", "ubuntu12_32/steam "]).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.split_whitespace().next().and_then(|value| value.parse::<i32>().ok())
    });
    let Some(target) = target else {
        return serde_json::json!({ "pid": null, "ld_audit": "", "loaded": false });
    };
    match std::fs::read(format!("/proc/{target}/environ")) {
        Ok(bytes) => {
            let mut value = String::new();
            for chunk in bytes.split(|byte| *byte == 0) {
                if let Some(rest) = chunk.strip_prefix(b"LD_AUDIT=") {
                    value = String::from_utf8_lossy(rest).into_owned();
                    break;
                }
            }
            let loaded = value.contains("libvapor_forge.so");
            serde_json::json!({ "pid": target, "ld_audit": value, "loaded": loaded })
        }
        Err(error) => serde_json::json!({ "pid": target, "ld_audit": "", "loaded": false, "error": error.to_string() }),
    }
}

/// Only the audit shim goes in LD_AUDIT; the Proton helper is injected elsewhere.
pub fn audit_libs() -> String {
    let mut libs: Vec<String> = Vec::new();
    for arch in ["i686", "x86_64"] {
        let path = crate::paths::vf_data().join(arch).join("libvapor_forge.so");
        if path.is_file() {
            libs.push(path.display().to_string());
        }
    }
    libs.join(":")
}

fn steam_binary() -> Option<String> {
    if let Ok(output) = Command::new("which").arg("steam").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    root().map(|path| path.join("steam.sh").display().to_string()).filter(|path| PathBuf::from(path).is_file())
}

pub fn shutdown(timeout: std::time::Duration) -> Result<bool, String> {
    let binary = steam_binary().ok_or("steam executable not found in PATH")?;
    Command::new(binary)
        .arg("-shutdown")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot run steam -shutdown: {error}"))?;
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !is_running() {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(!is_running())
}

pub fn start(extra_args: &[String]) -> Result<serde_json::Value, String> {
    let binary = steam_binary().ok_or("steam executable not found in PATH")?;
    let libs = audit_libs();
    let mut command = Command::new(binary);
    for arg in extra_args {
        command.arg(arg);
    }
    let existing = std::env::var("LD_AUDIT").unwrap_or_default();
    if !libs.is_empty() && !existing.contains("vapor_forge") {
        let merged = if existing.is_empty() { libs.clone() } else { format!("{libs}:{existing}") };
        command.env("LD_AUDIT", merged);
    }
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start Steam: {error}"))?;
    Ok(serde_json::json!({ "started": true, "ld_audit": libs, "args": extra_args }))
}

pub fn restart(extra_args: &[String]) -> Result<serde_json::Value, String> {
    let was_running = is_running();
    if was_running && !shutdown(std::time::Duration::from_secs(45))? {
        return Err("Steam did not shut down in time".into());
    }
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let started = start(extra_args)?;
    Ok(serde_json::json!({ "stopped": was_running, "start": started }))
}