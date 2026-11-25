//! Steam appmanifest handling: read what Steam thinks, and stop the loops.
//!
//! Two failures ruin a manifest install on Linux and both live in
//! `steamapps/appmanifest_<appid>.acf`:
//!
//! * "update required" forever, because `StateFlags` is not 4 or
//!   `BytesDownloaded` lags `BytesToDownload`;
//! * a silent re-download after restart, because the client is allowed to
//!   schedule its own updates.
//!
//! Vapor Forge makes the licence and the manifests valid; this module makes
//! Steam believe the install is finished and keeps it that way. Steam rewrites
//! the file while it runs, so every write refuses unless Steam is closed.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::steamctl;
use crate::vdf;

pub const FLAG_BITS: &[(u64, &str)] = &[
    (1, "uninstalled"),
    (2, "update required"),
    (4, "fully installed"),
    (8, "encrypted"),
    (16, "locked"),
    (32, "files missing"),
    (64, "app running"),
    (128, "files corrupt"),
    (256, "update running"),
    (512, "update paused"),
    (1024, "update started"),
    (2048, "uninstalling"),
    (4096, "backup running"),
    (8192, "reconfiguring"),
    (16384, "validating"),
    (32768, "adding files"),
    (65536, "preallocating"),
    (131072, "downloading"),
    (262144, "staging"),
    (524288, "committing"),
    (1048576, "update stopping"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Fix,
    Block,
    Unblock,
}

impl Mode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "fix" => Ok(Self::Fix),
            "block" => Ok(Self::Block),
            "unblock" => Ok(Self::Unblock),
            other => Err(format!("unknown mode {other:?}; use fix, block or unblock")),
        }
    }

    fn settings(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::Fix => vec![
                ("StateFlags", "4"),
                ("AutoUpdateBehavior", "1"),
                ("AllowOtherDownloadsWhileRunning", "2"),
                ("ScheduledAutoUpdate", "0"),
            ],
            Self::Block => vec![("AutoUpdateBehavior", "1"), ("ScheduledAutoUpdate", "0")],
            Self::Unblock => vec![("AutoUpdateBehavior", "0"), ("ScheduledAutoUpdate", "1")],
        }
    }

    fn matches_bytes(&self) -> bool {
        matches!(self, Self::Fix)
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Fix => "fix",
            Self::Block => "block",
            Self::Unblock => "unblock",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Depot {
    pub depot_id: String,
    pub manifest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub path: PathBuf,
    pub library: PathBuf,
    pub appid: u64,
    pub name: Option<String>,
    pub state_flags: u64,
    pub state: Vec<String>,
    pub bytes_to_download: Option<i64>,
    pub bytes_downloaded: Option<i64>,
    pub size_on_disk: Option<i64>,
    pub last_updated: Option<i64>,
    pub buildid: Option<String>,
    pub auto_update: Option<String>,
    pub scheduled_auto_update: Option<String>,
    pub allow_other_downloads: Option<String>,
    pub depots: Vec<Depot>,
    pub mtime: u64,
}

pub fn libraries() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    if let Some(root) = steamctl::root() {
        let direct = root.join("steamapps");
        if direct.is_dir() {
            found.push(direct.clone());
        }
        for name in ["libraryfolders.vdf", "libraryfolders.acf"] {
            let path = direct.join(name);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(tree) = vdf::parse(&text) else {
                continue;
            };
            let mut paths = Vec::new();
            vdf::collect_paths(&tree, &mut paths);
            for base in paths {
                let candidate = PathBuf::from(base).join("steamapps");
                if candidate.is_dir() {
                    found.push(candidate);
                }
            }
        }
    }
    let mut unique: Vec<PathBuf> = Vec::new();
    for path in found {
        let resolved = path.canonicalize().unwrap_or(path);
        if !unique.contains(&resolved) {
            unique.push(resolved);
        }
    }
    unique
}

pub fn manifest_path(appid: u64) -> Option<PathBuf> {
    let name = format!("appmanifest_{appid}.acf");
    libraries()
        .into_iter()
        .map(|library| library.join(&name))
        .find(|path| path.is_file())
}

pub fn read(appid: u64) -> Result<Option<Manifest>, String> {
    let Some(path) = manifest_path(appid) else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let tree =
        vdf::parse(&text).map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    // keyvalues-parser hands back the root object's contents, so AppState is
    // already unwrapped; older callers may still pass a wrapped tree.
    let state = match vdf::child(&tree, "AppState") {
        Some(node) => node.clone(),
        None => tree,
    };
    let scalar = |key: &str| vdf::scalar(&state, key);
    let flags: u64 = scalar("StateFlags")
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    let mut labels = Vec::new();
    for (bit, label) in FLAG_BITS {
        if flags & bit != 0 {
            labels.push((*label).to_string());
        }
    }
    let depots = vdf::child(&state, "InstalledDepots")
        .and_then(|node| node.as_object().cloned())
        .map(|map| {
            let mut list: Vec<Depot> = map
                .into_iter()
                .map(|(key, value)| Depot {
                    depot_id: key,
                    manifest: vdf::scalar(&value, "manifest"),
                })
                .collect();
            list.sort_by(|a, b| a.depot_id.cmp(&b.depot_id));
            list
        })
        .unwrap_or_default();
    let numeric = |key: &str| vdf::scalar_i64(&state, key);
    let mtime = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|delta| delta.as_secs())
        .unwrap_or(0);
    Ok(Some(Manifest {
        library: path.parent().map(Path::to_path_buf).unwrap_or_default(),
        path,
        appid: scalar("appid")
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(appid),
        name: scalar("name"),
        state_flags: flags,
        state: labels,
        bytes_to_download: numeric("BytesToDownload"),
        bytes_downloaded: numeric("BytesDownloaded"),
        size_on_disk: numeric("SizeOnDisk"),
        last_updated: numeric("LastUpdated"),
        buildid: scalar("buildid"),
        auto_update: scalar("AutoUpdateBehavior"),
        scheduled_auto_update: scalar("ScheduledAutoUpdate"),
        allow_other_downloads: scalar("AllowOtherDownloadsWhileRunning"),
        depots,
        mtime,
    }))
}

pub fn running_apps() -> Vec<Value> {
    let mut out = Vec::new();
    for library in libraries() {
        let Ok(entries) = std::fs::read_dir(&library) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(rest) = name.strip_prefix("appmanifest_") else {
                continue;
            };
            let Some(appid) = rest
                .strip_suffix(".acf")
                .and_then(|text| text.parse::<u64>().ok())
            else {
                continue;
            };
            let Ok(Some(data)) = read(appid) else {
                continue;
            };
            if data.state_flags & 64 == 0 {
                continue;
            }
            out.push(json!({ "appid": data.appid, "name": data.name }));
        }
    }
    out
}

pub fn report(appid: u64, plan: Option<&Value>) -> Value {
    let running = steamctl::is_running();
    let libs: Vec<String> = libraries()
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let Ok(Some(data)) = read(appid) else {
        return json!({
            "appid": appid,
            "installed": false,
            "steam_running": running,
            "libraries": libs,
            "problems": ["no appmanifest yet: let Steam download the app once, then fix it here"],
            "depots": [],
            "planned_depots": [],
        });
    };

    let mut problems: Vec<String> = Vec::new();
    if data.state_flags != 0 && data.state_flags & 4 == 0 {
        problems.push(format!(
            "StateFlags {} is not fully installed ({})",
            data.state_flags,
            data.state.join(", ")
        ));
    }
    if let (Some(total), Some(done)) = (data.bytes_to_download, data.bytes_downloaded) {
        if done < total {
            problems.push(format!("{} bytes still marked pending", total - done));
        }
    }
    if plan.is_some() {
        match data.auto_update.as_deref() {
            None | Some("") | Some("0") => problems.push(
                "automatic updates enabled: Steam may re-download on its own schedule".into(),
            ),
            _ => {}
        }
        if let Some(value) = data.scheduled_auto_update.as_deref() {
            if value != "0" && !value.is_empty() {
                problems.push("scheduled auto update enabled".into());
            }
        }
    }

    let mut planned = Vec::new();
    if let Some(plan) = plan {
        if let Some(depots) = plan.get("depots").and_then(Value::as_array) {
            for depot in depots {
                let depot_id = depot.get("depot_id").cloned().unwrap_or(Value::Null);
                let key = depot_id
                    .as_u64()
                    .map(|id| id.to_string())
                    .or_else(|| depot_id.as_str().map(str::to_string));
                let current = key
                    .as_ref()
                    .and_then(|key| data.depots.iter().find(|item| &item.depot_id == key));
                let planned_gid = depot.get("gid").cloned().unwrap_or(Value::Null);
                let steam_gid = current.and_then(|item| item.manifest.clone());
                let pinned =
                    matches!((&steam_gid, planned_gid.as_str()), (Some(a), Some(b)) if a == b);
                planned.push(json!({
                    "depot_id": depot_id,
                    "planned_gid": planned_gid,
                    "steam_gid": steam_gid,
                    "in_appmanifest": current.is_some(),
                    "pinned": pinned,
                }));
            }
        }
    }

    json!({
        "appid": data.appid,
        "installed": true,
        "name": data.name,
        "path": data.path.display().to_string(),
        "library": data.library.display().to_string(),
        "steam_running": running,
        "state_flags": data.state_flags,
        "state": data.state,
        "bytes_to_download": data.bytes_to_download,
        "bytes_downloaded": data.bytes_downloaded,
        "size_on_disk": data.size_on_disk,
        "last_updated": data.last_updated,
        "auto_update": data.auto_update,
        "scheduled_auto_update": data.scheduled_auto_update,
        "allow_other_downloads": data.allow_other_downloads,
        "buildid": data.buildid,
        "mtime": data.mtime,
        "depots": data.depots.iter().map(|depot| json!({
            "depot_id": depot.depot_id,
            "manifest": depot.manifest,
        })).collect::<Vec<_>>(),
        "planned_depots": planned,
        "problems": problems,
        "libraries": libs,
    })
}

/// Replace `"key" "value"` in place, or append it before the closing brace.
fn set_key(text: &str, key: &str, value: &str) -> (String, bool) {
    let needle = format!("\"{key}\"");
    if let Some(found) = text.find(&needle) {
        let after = found + needle.len();
        let rest = &text[after..];
        let mut chars = rest.char_indices().peekable();
        while let Some((_, ch)) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        if let Some((start, '"')) = chars.next() {
            if let Some(end_rel) = rest[start + 1..].find('"') {
                let value_start = after + start + 1;
                let value_end = value_start + end_rel;
                let mut out = String::with_capacity(text.len() + value.len());
                out.push_str(&text[..value_start]);
                out.push_str(value);
                out.push_str(&text[value_end..]);
                return (out, true);
            }
        }
    }
    let Some(close) = text.rfind('}') else {
        return (text.to_string(), false);
    };
    let line_start = text[..close]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let indent: String = text[line_start..close]
        .chars()
        .take_while(|ch| *ch == '\t' || *ch == ' ')
        .collect();
    let indent = if indent.is_empty() {
        "\t".to_string()
    } else {
        indent
    };
    let mut out = String::with_capacity(text.len() + key.len() + value.len() + 16);
    out.push_str(&text[..line_start]);
    out.push_str(&format!("{indent}\"{key}\"\t\t\"{value}\"\n"));
    out.push_str(&text[line_start..]);
    (out, false)
}

pub fn patch_with_restart(appid: u64, mode: &Mode) -> Result<Value, String> {
    if manifest_path(appid).is_none() {
        return Err("no appmanifest for that app yet".into());
    }
    if steamctl::is_running() {
        let busy = running_apps();
        if !busy.is_empty() {
            let names: Vec<String> = busy
                .iter()
                .map(|app| {
                    app.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string()
                })
                .collect();
            return Err(format!(
                "a game is running ({}): close it before restarting Steam",
                names.join(", ")
            ));
        }
        steamctl::shutdown(std::time::Duration::from_secs(45))?;
        match patch(appid, mode, false) {
            Ok(mut result) => {
                steamctl::start(&[])?;
                result["restarted"] = json!(true);
                Ok(result)
            }
            Err(error) => {
                // A failed patch must not leave the person with no client.
                let _ = steamctl::start(&[]);
                Err(error)
            }
        }
    } else {
        patch(appid, mode, false)
    }
}

pub fn patch(appid: u64, mode: &Mode, steam_check: bool) -> Result<Value, String> {
    let Some(path) = manifest_path(appid) else {
        return Err("no appmanifest for that app yet".into());
    };
    let running = steamctl::is_running();
    if steam_check && running {
        return Err(
            "Steam is running and will overwrite the file: close Steam, patch, then start it"
                .into(),
        );
    }
    let original = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let before = read(appid)?;
    // An install that never wrote a byte is not a broken finished install:
    // marking it StateFlags=4 would hide the download forever. Drop the
    // manifest (keeping a backup) so Steam offers a fresh install instead.
    let empty_install = before.as_ref().is_some_and(|data| {
        data.size_on_disk.unwrap_or(0) <= 0 && data.depots.is_empty()
    });
    if matches!(mode, Mode::Fix) && empty_install {
        let backup = path.with_extension("acf.exxatools.bak");
        if !backup.exists() {
            std::fs::copy(&path, &backup)
                .map_err(|error| format!("cannot back up {}: {error}", path.display()))?;
        }
        std::fs::remove_file(&path)
            .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
        return Ok(json!({
            "path": path.display().to_string(),
            "backup": backup.display().to_string(),
            "mode": mode.as_str(),
            "action": "removed-empty-manifest",
            "detail": "nothing was installed yet; Steam now offers a fresh download",
        }));
    }
    let mut text = original.clone();
    let mut changes = Vec::new();
    for (key, value) in mode.settings() {
        let (updated, replaced) = set_key(&text, key, value);
        text = updated;
        changes.push(json!({ "key": key, "value": value, "updated": replaced }));
    }
    if mode.matches_bytes() {
        if let Some(total) = before.as_ref().and_then(|data| data.bytes_to_download) {
            let (updated, replaced) = set_key(&text, "BytesDownloaded", &total.to_string());
            text = updated;
            changes.push(json!({ "key": "BytesDownloaded", "value": total.to_string(), "updated": replaced }));
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|delta| delta.as_secs())
            .unwrap_or(0);
        let (updated, _) = set_key(&text, "LastUpdated", &now.to_string());
        text = updated;
    }

    let backup = path.with_extension("acf.exxatools.bak");
    if !backup.exists() {
        std::fs::copy(&path, &backup)
            .map_err(|error| format!("cannot back up {}: {error}", path.display()))?;
    }
    let temp = path.with_extension("acf.tmp");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp)
            .map_err(|error| format!("cannot create {}: {error}", temp.display()))?;
        file.write_all(text.as_bytes())
            .map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
        // Rename is atomic, but only after the bytes reach the disk; otherwise a
        // power loss can leave an empty appmanifest where Steam expects a game.
        file.sync_all()
            .map_err(|error| format!("cannot flush {}: {error}", temp.display()))?;
    }
    std::fs::rename(&temp, &path)
        .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;

    match vdf::parse(&text) {
        Ok(tree) if vdf::child(&tree, "appid").is_some() => {}
        Ok(_) => {
            let _ = std::fs::copy(&backup, &path);
            return Err("patched file lost its appid, restored the backup".into());
        }
        Err(error) => {
            let _ = std::fs::copy(&backup, &path);
            return Err(format!(
                "patched file failed validation, restored the backup: {error}"
            ));
        }
    }

    Ok(json!({
        "appid": appid,
        "path": path.display().to_string(),
        "mode": mode.as_str(),
        "backup": backup.display().to_string(),
        "changes": changes,
        "before": {
            "state_flags": before.as_ref().map(|data| data.state_flags),
            "auto_update": before.as_ref().and_then(|data| data.auto_update.clone()),
            "scheduled_auto_update": before.as_ref().and_then(|data| data.scheduled_auto_update.clone()),
        },
        "after": report(appid, None),
        "steam_running": running,
        "warning": if running { Value::Null } else { Value::String("start Steam again so the patched appmanifest is the one it loads".into()) },
    }))
}
#[cfg(test)]
mod tests {
    use super::*;

    const ACF: &str = "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n\t\"StateFlags\"\t\t\"1026\"\n\t\"BytesToDownload\"\t\t\"100\"\n\t\"BytesDownloaded\"\t\t\"40\"\n\t\"AutoUpdateBehavior\"\t\t\"0\"\n\t\"InstalledDepots\"\n\t{\n\t\t\"621\"\n\t\t{\n\t\t\t\"manifest\"\t\t\"111\"\n\t\t}\n\t}\n}\n";

    #[test]
    fn replaces_existing_values() {
        let (text, replaced) = set_key(ACF, "StateFlags", "4");
        assert!(replaced);
        assert!(text.contains("\"StateFlags\"\t\t\"4\""));
        assert!(!text.contains("\"1026\""));
    }

    #[test]
    fn inserts_missing_keys() {
        let (text, replaced) = set_key(ACF, "ScheduledAutoUpdate", "0");
        assert!(!replaced);
        assert!(text.contains("\"ScheduledAutoUpdate\"\t\t\"0\""));
        assert!(crate::vdf::parse(&text).is_ok());
    }

    #[test]
    fn reads_flags_and_depots() {
        let tree = crate::vdf::parse(ACF).unwrap();
        let state = &tree;
        let flags: u64 = crate::vdf::scalar(state, "StateFlags")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(flags & 2, 2);
        assert_eq!(flags & 1024, 1024);
        let depots = crate::vdf::child(state, "InstalledDepots").unwrap();
        assert!(crate::vdf::child(depots, "621").is_some());
    }
}
