//! Watches steamapps and closes the update loop once a download lands.
//!
//! Steam rewrites `appmanifest_<appid>.acf` while it runs, so nothing is
//! written until the client is gone: the watcher marks what needs a fix and
//! applies it as soon as Steam exits.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::appmanifest;
use crate::steamctl;

fn needs_fix(appid: u64) -> Option<String> {
    let data = appmanifest::read(appid).ok().flatten()?;
    if data.state_flags & 4 == 0 {
        return Some(format!("StateFlags {}", data.state_flags));
    }
    if let (Some(total), Some(done)) = (data.bytes_to_download, data.bytes_downloaded) {
        if done < total {
            return Some(format!("{} bytes pending", total - done));
        }
    }
    match data.auto_update.as_deref() {
        None | Some("") | Some("0") => Some("automatic updates enabled".into()),
        _ => None,
    }
}

fn appid_from(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    name.strip_prefix("appmanifest_")?.strip_suffix(".acf")?.parse().ok()
}

/// One sweep over every known appmanifest.
pub fn sweep(apply: bool) -> Value {
    let running = steamctl::is_running();
    let mut pending = Vec::new();
    let mut fixed = Vec::new();
    let mut failed = Vec::new();
    for library in appmanifest::libraries() {
        let Ok(entries) = std::fs::read_dir(&library) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(appid) = appid_from(&path) else { continue };
            let Some(reason) = needs_fix(appid) else { continue };
            if apply && !running {
                match appmanifest::patch(appid, &appmanifest::Mode::Fix, false) {
                    Ok(result) => fixed.push(json!({ "appid": appid, "changes": result.get("changes").cloned() })),
                    Err(error) => failed.push(json!({ "appid": appid, "error": error })),
                }
            } else {
                pending.push(json!({ "appid": appid, "path": path.display().to_string(), "reason": reason }));
            }
        }
    }
    json!({
        "steam_running": running,
        "applied": apply && !running,
        "pending": pending,
        "fixed": fixed,
        "failed": failed,
    })
}

/// Blocking watch: a fix is attempted whenever Steam is not running.
pub fn run() -> Result<(), String> {
    use notify::{RecursiveMode, Watcher};

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .map_err(|error| format!("cannot start the watcher: {error}"))?;

    let libraries = appmanifest::libraries();
    if libraries.is_empty() {
        return Err("no Steam library found to watch".into());
    }
    for library in &libraries {
        watcher
            .watch(library, RecursiveMode::NonRecursive)
            .map_err(|error| format!("cannot watch {}: {error}", library.display()))?;
        eprintln!("watching {}", library.display());
    }

    loop {
        match receiver.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    let Some(appid) = appid_from(&path) else { continue };
                    let Some(reason) = needs_fix(appid) else { continue };
                    if steamctl::is_running() {
                        eprintln!("{appid} needs a fix ({reason}); waiting for Steam to exit");
                        continue;
                    }
                    match appmanifest::patch(appid, &appmanifest::Mode::Fix, false) {
                        Ok(_) => println!("{}", json!({ "appid": appid, "fixed": true, "reason": reason })),
                        Err(error) => println!("{}", json!({ "appid": appid, "fixed": false, "error": error })),
                    }
                }
            }
            Ok(Err(error)) => eprintln!("watch error: {error}"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let idle = sweep(true);
                if idle.get("fixed").and_then(Value::as_array).map(|list| !list.is_empty()).unwrap_or(false) {
                    println!("{idle}");
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}