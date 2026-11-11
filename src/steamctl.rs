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
pub fn active_account() -> Option<(String, String)> {
    let root = root()?;
    let text = std::fs::read_to_string(root.join("config/loginusers.vdf")).ok()?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    let tree = crate::vdf::parse(&text).ok()?;
    let users = crate::vdf::child(&tree, "users").or_else(|| crate::vdf::child(&tree, "Users"))?;
    let map = users.as_object()?;
    let mut fallback: Option<String> = None;
    for (steam_id, entry) in map {
        if fallback.is_none() {
            fallback = Some(steam_id.clone());
        }
        if crate::vdf::scalar(entry, "MostRecent").as_deref() == Some("1") {
            return Some((account_id(steam_id), steam_id.clone()));
        }
    }
    fallback.map(|steam_id| (account_id(&steam_id), steam_id))
}

fn account_id(steam_id: &str) -> String {
    steam_id
        .parse::<u64>()
        .map(|value| value.saturating_sub(76561197960265728).to_string())
        .unwrap_or_else(|_| steam_id.to_string())
}

pub fn localconfig_path() -> Option<PathBuf> {
    let root = root()?;
    if let Some((account, _)) = active_account() {
        let path = root.join("userdata").join(account).join("config/localconfig.vdf");
        if path.is_file() {
            return Some(path);
        }
    }
    // Fall back to any account that has a localconfig, newest directory first.
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(root.join("userdata"))
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("config/localconfig.vdf"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort_by_key(|path| std::fs::metadata(path).and_then(|meta| meta.modified()).ok());
    candidates.pop()
}

pub fn read_launch_options(appid: u64) -> Option<String> {
    let path = localconfig_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let tree = crate::vdf::parse(&text).ok()?;
    let apps = crate::vdf::find_key(&tree, "apps")?;
    let app = crate::vdf::child(apps, &appid.to_string())?;
    crate::vdf::scalar(app, "LaunchOptions")
}

/// Brace span of the `"<appid>" { ... }` block inside the apps table.
fn app_block(text: &str, appid: u64) -> Option<(usize, usize)> {
    let needle = format!("\"{appid}\"");
    let mut from = 0;
    while let Some(found) = text[from..].find(&needle) {
        let start = from + found;
        let after = start + needle.len();
        if let Some(open_rel) = text[after..].find('{') {
            let open = after + open_rel;
            let mut depth = 0usize;
            for (offset, ch) in text[open..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some((open, open + offset));
                        }
                    }
                    _ => {}
                }
            }
        }
        from = after;
    }
    None
}

/// Pure text edit, so the rule can be tested without a Steam install.
pub fn apply_launch_option(text: &str, appid: u64, options: &str) -> Result<(String, Option<String>), String> {
    let previous = {
        let tree = crate::vdf::parse(text).map_err(|error| error.to_string())?;
        crate::vdf::find_key(&tree, "apps")
            .and_then(|apps| crate::vdf::child(apps, &appid.to_string()))
            .and_then(|app| crate::vdf::scalar(app, "LaunchOptions"))
    };
    let Some((open, close)) = app_block(text, appid) else {
        return Err(format!("{appid} is not in this config yet: launch the game once, then set options"));
    };
    if let Some(index) = text[open..close].find("\"LaunchOptions\"") {
        let after_key = open + index + "\"LaunchOptions\"".len();
        let Some(quote_start) = text[after_key..close].find('"') else {
            return Err("malformed LaunchOptions entry".into());
        };
        let value_start = after_key + quote_start + 1;
        let Some(quote_end) = text[value_start..close].find('"') else {
            return Err("malformed LaunchOptions value".into());
        };
        let value_end = value_start + quote_end;
        return Ok((format!("{}{}{}", &text[..value_start], options, &text[value_end..]), previous));
    }
    let line_start = text[..close].rfind('\n').map(|index| index + 1).unwrap_or(0);
    let tabs: String = text[line_start..close].chars().take_while(|ch| *ch == '\t' || *ch == ' ').collect();
    let mut out = String::with_capacity(text.len() + options.len() + 24);
    out.push_str(&text[..close]);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{tabs}\t\"LaunchOptions\"\t\t\"{options}\"\n"));
    out.push_str(&text[close..]);
    Ok((out, previous))
}

pub fn set_launch_options(appid: u64, options: &str) -> Result<serde_json::Value, String> {
    let path = localconfig_path().ok_or("no Steam localconfig.vdf found for the active account")?;
    if is_running() {
        return Err("close Steam before changing launch options".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let (updated, previous) = apply_launch_option(&text, appid, options)?;
    let backup = path.with_file_name("localconfig.vdf.exxatools.bak");
    let _ = std::fs::write(&backup, &text);
    let temp = path.with_file_name("localconfig.vdf.exxatools.tmp");
    std::fs::write(&temp, &updated).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "path": path.display().to_string(),
        "previous": previous,
        "options": options,
        "backup": backup.display().to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "\"UserLocalConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n\t\t\t\t\t\"440\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1700000000\"\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n";

    #[test]
    fn inserts_when_missing() {
        let (updated, previous) = apply_launch_option(CONFIG, 440, "-onlinefix").unwrap();
        assert_eq!(previous, None);
        assert!(updated.contains("\"LaunchOptions\"\t\t\"-onlinefix\""));
        let tree = crate::vdf::parse(&updated).unwrap();
        let options = crate::vdf::find_key(&tree, "apps").and_then(|apps| crate::vdf::child(apps, "440")).and_then(|app| crate::vdf::scalar(app, "LaunchOptions"));
        assert_eq!(options.as_deref(), Some("-onlinefix"));
    }

    #[test]
    fn replaces_existing() {
        let text = CONFIG.replace("\"LastPlayed\"\t\t\"1700000000\"", "\"LaunchOptions\"\t\t\"-old\"");
        let (updated, previous) = apply_launch_option(&text, 440, "-new").unwrap();
        assert_eq!(previous.as_deref(), Some("-old"));
        assert!(updated.contains("\"-new\""));
        assert!(!updated.contains("-old"));
    }

    #[test]
    fn refuses_unknown_app() {
        assert!(apply_launch_option(CONFIG, 620, "-onlinefix").is_err());
    }

    #[test]
    fn merges_without_duplicates() {
        assert_eq!(crate::verify::merge_launch_option(Some("-a"), "-onlinefix"), "-a -onlinefix");
        assert_eq!(crate::verify::merge_launch_option(Some("-onlinefix"), "-onlinefix"), "-onlinefix");
        assert_eq!(crate::verify::merge_launch_option(None, "-x"), "-x");
    }
}
