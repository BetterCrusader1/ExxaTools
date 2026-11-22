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

fn update_reason(data: &appmanifest::Manifest, managed: bool) -> Option<String> {
    if data.state_flags & 4 == 0 {
        return Some(format!("StateFlags {}", data.state_flags));
    }
    if let (Some(total), Some(done)) = (data.bytes_to_download, data.bytes_downloaded) {
        if done < total {
            return Some(format!("{} bytes pending", total - done));
        }
    }
    // Steam's own Proton/runtime components should keep automatic updates. The
    // stop-loop policy only applies to games the user added to ExxaTools.
    if !managed {
        return None;
    }
    match data.auto_update.as_deref() {
        None | Some("") | Some("0") => Some("automatic updates enabled".into()),
        _ => None,
    }
}

fn needs_fix(appid: u64) -> Option<String> {
    let data = appmanifest::read(appid).ok().flatten()?;
    let managed = crate::state::load()
        .map(|state| state.apps.contains_key(&appid.to_string()))
        .unwrap_or(false);
    // Steam's own runtimes and redistributables update themselves; flagging
    // them left a warning in System that only a Steam exit could clear.
    if !managed {
        return None;
    }
    update_reason(&data, managed)
}

fn appid_from(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    name.strip_prefix("appmanifest_")?
        .strip_suffix(".acf")?
        .parse()
        .ok()
}

/// One sweep over every known appmanifest.
pub fn sweep(apply: bool) -> Value {
    let running = steamctl::is_running();
    let mut pending = Vec::new();
    let mut fixed = Vec::new();
    let mut failed = Vec::new();
    for library in appmanifest::libraries() {
        let Ok(entries) = std::fs::read_dir(&library) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(appid) = appid_from(&path) else {
                continue;
            };
            let Some(reason) = needs_fix(appid) else {
                continue;
            };
            if apply && !running {
                match appmanifest::patch(appid, &appmanifest::Mode::Fix, false) {
                    Ok(result) => fixed
                        .push(json!({ "appid": appid, "changes": result.get("changes").cloned() })),
                    Err(error) => failed.push(json!({ "appid": appid, "error": error })),
                }
            } else {
                pending.push(
                    json!({ "appid": appid, "path": path.display().to_string(), "reason": reason }),
                );
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

    // Steam writes an appmanifest dozens of times per download. Patch only after
    // the writes go quiet, or a half-written file can be read and rewritten.
    let debounce = Duration::from_millis(1500);
    let idle = Duration::from_secs(60);
    let mut pending: std::collections::HashMap<u64, std::time::Instant> =
        std::collections::HashMap::new();
    let mut last_sweep = std::time::Instant::now();
    loop {
        let wait = pending
            .values()
            .map(|seen| debounce.saturating_sub(seen.elapsed()))
            .min()
            .unwrap_or(idle)
            .max(Duration::from_millis(50));
        match receiver.recv_timeout(wait) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if let Some(appid) = appid_from(&path) {
                        pending.insert(appid, std::time::Instant::now());
                    }
                }
            }
            Ok(Err(error)) => eprintln!("watch error: {error}"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let ready: Vec<u64> = pending
                    .iter()
                    .filter(|(_, seen)| seen.elapsed() >= debounce)
                    .map(|(appid, _)| *appid)
                    .collect();
                for appid in ready {
                    pending.remove(&appid);
                    let Some(reason) = needs_fix(appid) else {
                        continue;
                    };
                    if steamctl::is_running() {
                        eprintln!("{appid} needs a fix ({reason}); waiting for Steam to exit");
                        continue;
                    }
                    match appmanifest::patch(appid, &appmanifest::Mode::Fix, false) {
                        Ok(_) => println!(
                            "{}",
                            json!({ "appid": appid, "fixed": true, "reason": reason })
                        ),
                        Err(error) => println!(
                            "{}",
                            json!({ "appid": appid, "fixed": false, "error": error })
                        ),
                    }
                }
                if pending.is_empty() && last_sweep.elapsed() >= idle {
                    last_sweep = std::time::Instant::now();
                    let report = sweep(true);
                    if report
                        .get("fixed")
                        .and_then(Value::as_array)
                        .map(|list| !list.is_empty())
                        .unwrap_or(false)
                    {
                        println!("{report}");
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn manifest(state_flags: u64, auto_update: Option<&str>) -> appmanifest::Manifest {
        appmanifest::Manifest {
            path: PathBuf::from("appmanifest_4183110.acf"),
            library: PathBuf::from("steamapps"),
            appid: 4183110,
            name: Some("Steam Linux Runtime".into()),
            state_flags,
            state: Vec::new(),
            bytes_to_download: None,
            bytes_downloaded: None,
            size_on_disk: None,
            last_updated: None,
            buildid: None,
            auto_update: auto_update.map(str::to_string),
            scheduled_auto_update: None,
            allow_other_downloads: None,
            depots: Vec::new(),
            mtime: 0,
        }
    }

    #[test]
    fn automatic_updates_only_flag_managed_games() {
        let data = manifest(4, Some("0"));
        assert!(update_reason(&data, false).is_none());
        assert_eq!(
            update_reason(&data, true).as_deref(),
            Some("automatic updates enabled")
        );

        let unfinished = manifest(2, Some("0"));
        assert_eq!(
            update_reason(&unfinished, false).as_deref(),
            Some("StateFlags 2")
        );
    }
}
