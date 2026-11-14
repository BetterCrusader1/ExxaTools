//! Vapor Forge engine lifecycle: detect, install, update, uninstall, validate.
//!
//! Vapor Forge is the piece that actually hooks the Steam client: it makes
//! Steam accept a licence it does not own, serve pinned manifests, and decrypt
//! depots. Without it a plan is a file nobody reads, so this module owns the
//! whole "does the engine run" question.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::http;
use crate::paths;
use crate::steamctl;

pub const REPO: &str = "Hintay/vapor-forge";
pub const ARCHES: &[&str] = &["i686", "x86_64"];
pub const LIBS: &[&str] = &["libvapor_forge.so", "libvapor_forge_proton_inject.so"];
pub const LOG_TAG: &str = "[vapor-forge]";
pub const NOISE_TAG: &str = "feat: sharing unlocked app_id=";

fn linux_asset(release: &Value) -> Option<Value> {
    let assets = release.get("assets")?.as_array()?;
    let mut best: Option<Value> = None;
    for asset in assets {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or("");
        if !name.ends_with(".tar.gz") || !name.contains("linux") {
            continue;
        }
        if name.contains("x86_64") || name.contains("amd64") {
            return Some(asset.clone());
        }
        if best.is_none() {
            best = Some(asset.clone());
        }
    }
    best
}

pub fn releases(limit: usize) -> Result<Vec<Value>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page={}", limit.clamp(1, 20));
    let payload = http::get_json(&url).map_err(|error| format!("cannot reach the GitHub release API: {error}"))?;
    let mut out = Vec::new();
    for release in payload.as_array().cloned().unwrap_or_default() {
        let Some(asset) = linux_asset(&release) else { continue };
        out.push(json!({
            "tag": release.get("tag_name").cloned().unwrap_or(Value::Null),
            "name": release.get("name").cloned().unwrap_or(Value::Null),
            "published": release.get("published_at").cloned().unwrap_or(Value::Null),
            "notes_url": release.get("html_url").cloned().unwrap_or(Value::Null),
            "asset": asset.get("name").cloned().unwrap_or(Value::Null),
            "size": asset.get("size").cloned().unwrap_or(Value::Null),
            "url": asset.get("browser_download_url").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(out)
}

pub fn latest() -> Result<Value, String> {
    releases(1)?.into_iter().next().ok_or_else(|| "no linux release published for Vapor Forge".to_string())
}

pub fn meta() -> Value {
    let path = paths::engine_meta();
    if !path.is_file() {
        let legacy = path.with_file_name("vapordepot-engine.json");
        if legacy.is_file() {
            let _ = std::fs::rename(&legacy, &path);
        }
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

pub fn libs() -> Value {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut out = serde_json::Map::new();
    for arch in ARCHES {
        let mut entries = Vec::new();
        for name in LIBS {
            let path = paths::vf_data().join(arch).join(name);
            let Ok(stat) = std::fs::metadata(&path) else { continue };
            let mtime = stat
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|delta| delta.as_secs())
                .unwrap_or(0);
            entries.push(json!({
                "name": name,
                "path": path.display().to_string(),
                "size": stat.len(),
                "mtime": mtime,
                "current": now.saturating_sub(mtime) < 900,
            }));
        }
        out.insert((*arch).to_string(), Value::Array(entries));
    }
    Value::Object(out)
}

pub fn luac() -> Option<String> {
    for name in ["luac", "luac5.4", "luac5.3"] {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

pub fn detect() -> Value {
    let libs_value = libs();
    let running = steamctl::is_running();
    let audit = if running { steamctl::audit_loaded() } else { json!({ "loaded": false, "pid": null, "ld_audit": "" }) };
    let info = meta();
    let has = |arch: &str| {
        libs_value
            .get(arch)
            .and_then(Value::as_array)
            .map(|list| !list.is_empty())
            .unwrap_or(false)
    };
    let mut missing = Vec::new();
    for arch in ARCHES {
        for name in LIBS {
            if !paths::vf_data().join(arch).join(name).is_file() {
                missing.push(format!("{arch}/{name}"));
            }
        }
    }
    // `loaded` means the shim is mapped inside the process that owns
    // steamclient.so, not merely present in LD_AUDIT: a container runtime
    // keeps the variable but never loads the library.
    let loaded = audit.get("loaded").and_then(Value::as_bool).unwrap_or(false);
    json!({
        // A directory with one stray file is not an installation: every library
        // for both architectures has to be present.
        "installed": missing.is_empty(),
        "partial": has("x86_64") != has("i686"),
        "missing": missing,
        "version": info.get("tag").cloned().unwrap_or(Value::Null),
        "sha256": info.get("sha256").cloned().unwrap_or(Value::Null),
        "installed_at": info.get("installed_at").cloned().unwrap_or(Value::Null),
        "source": info.get("url").cloned().unwrap_or(Value::Null),
        "libs": libs_value,
        "ld_audit": steamctl::audit_libs(),
        "config_exists": paths::vf_config().is_file(),
        "config_path": paths::vf_config().display().to_string(),
        "scripts_dir": paths::vf_scripts().display().to_string(),
        "data_dir": paths::vf_data().display().to_string(),
        "steam_running": running,
        "steam_pid": audit.get("pid").cloned().unwrap_or(Value::Null),
        "steam_engine_loaded": loaded,
        "steam_engine_mapped": audit.get("mapped").and_then(Value::as_bool).unwrap_or(false),
        "steam_engine_active": audit.get("active").and_then(Value::as_bool).unwrap_or(false),
        "steam_ld_audit": audit.get("ld_audit").cloned().unwrap_or_else(|| json!("")),
        "needs_restart": running && has("x86_64") && !loaded,
        "lua": { "luac": luac(), "syntax_check": luac().is_some() },
    })
}

fn extract(archive: &Path, tag: &str) -> Result<PathBuf, String> {
    let target = paths::engine_cache().join("releases").join(tag);
    if target.join("install.sh").is_file() {
        return Ok(target);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let staging = paths::engine_cache().join(format!("staging-{}", std::process::id()));
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let file = std::fs::File::open(archive).map_err(|error| format!("cannot open {}: {error}", archive.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    tar.set_preserve_permissions(false);
    tar.set_overwrite(true);
    let entries = tar.entries().map_err(|error| format!("cannot read archive: {error}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| format!("cannot read archive: {error}"))?;
        let path = entry.path().map_err(|error| error.to_string())?.to_path_buf();
        if path.is_absolute() || path.components().any(|part| matches!(part, std::path::Component::ParentDir)) {
            return Err(format!("refusing unsafe archive member: {}", path.display()));
        }
        entry.unpack_in(&staging).map_err(|error| format!("cannot extract: {error}"))?;
    }
    let mut roots: Vec<PathBuf> = std::fs::read_dir(&staging)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    let source = if roots.len() == 1 { roots.remove(0) } else { staging.clone() };
    if target.exists() {
        std::fs::remove_dir_all(&target).ok();
    }
    std::fs::rename(&source, &target).map_err(|error| format!("cannot move release into place: {error}"))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(target)
}

fn run_installer(directory: &Path, action: &str) -> Result<Vec<String>, String> {
    let script = directory.join("install.sh");
    if !script.is_file() {
        return Err(format!("{} is missing from the release archive", script.display()));
    }
    let output = Command::new("bash")
        .arg(&script)
        .arg(action)
        .current_dir(directory)
        .output()
        .map_err(|error| format!("cannot run installer: {error}"))?;
    let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    lines.extend(String::from_utf8_lossy(&output.stderr).lines().map(str::to_string));
    if !output.status.success() {
        return Err(format!("installer exited {}\n{}", output.status.code().unwrap_or(-1), lines.join("\n")));
    }
    Ok(lines)
}

pub fn install(tag: Option<&str>) -> Result<Value, String> {
    let release = match tag {
        None => latest()?,
        Some(tag) => releases(20)?
            .into_iter()
            .find(|release| release.get("tag").and_then(Value::as_str) == Some(tag))
            .ok_or_else(|| format!("release {tag} has no linux tarball"))?,
    };
    let tag = release.get("tag").and_then(Value::as_str).unwrap_or("unknown").to_string();
    let url = release.get("url").and_then(Value::as_str).unwrap_or("").to_string();
    let asset = release.get("asset").and_then(Value::as_str).unwrap_or("engine.tar.gz").to_string();
    if url.is_empty() {
        return Err("release has no download URL".into());
    }
    let archive = paths::engine_cache().join("downloads").join(&asset);
    let recorded = meta();
    let reuse = archive.is_file()
        && recorded.get("tag").and_then(Value::as_str) == Some(tag.as_str())
        && recorded.get("sha256").and_then(Value::as_str).map(|expected| {
            http::sha256_file(&archive).map(|actual| actual == expected).unwrap_or(false)
        }).unwrap_or(false);
    let (sha256, size) = if reuse {
        (http::sha256_file(&archive).map_err(|error| error.to_string())?, std::fs::metadata(&archive).map(|m| m.len()).unwrap_or(0))
    } else {
        if archive.exists() {
            let _ = std::fs::remove_file(&archive);
        }
        http::download(&url, &archive).map_err(|error| format!("download failed: {error}"))?
    };
    let directory = extract(&archive, &tag)?;
    let installer = run_installer(&directory, "install")?;
    let present = libs();
    let ok = present
        .get("x86_64")
        .and_then(Value::as_array)
        .map(|list| !list.is_empty())
        .unwrap_or(false);
    if !ok {
        return Err("installer finished but the x86_64 library is not in place".into());
    }
    let meta_dir = paths::engine_meta();
    if let Some(parent) = meta_dir.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let payload = json!({
        "tag": tag,
        "url": url,
        "asset": asset,
        "sha256": sha256,
        "size": size,
        "installed_at": crate::lua::stamp(),
        "release_dir": directory.display().to_string(),
    });
    std::fs::write(&meta_dir, format!("{}\n", serde_json::to_string_pretty(&payload).unwrap_or_default()))
        .map_err(|error| format!("cannot write {}: {error}", meta_dir.display()))?;
    Ok(json!({
        "installed": true,
        "version": tag,
        "sha256": sha256,
        "installer": installer,
        "status": detect(),
    }))
}

fn manual_uninstall() -> Vec<String> {
    let mut done = Vec::new();
    for arch in ARCHES {
        for name in LIBS {
            let path = paths::vf_data().join(arch).join(name);
            if path.exists() && std::fs::remove_file(&path).is_ok() {
                done.push(format!("removed {}", path.display()));
            }
        }
    }
    for dropin in crate::paths::systemd_dropins() {
        if dropin.exists() && std::fs::remove_file(&dropin).is_ok() {
            done.push(format!("removed {}", dropin.display()));
        }
    }
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).stdout(Stdio::null()).status();
    done
}

pub fn uninstall() -> Result<Value, String> {
    let info = meta();
    let directory = PathBuf::from(info.get("release_dir").and_then(Value::as_str).unwrap_or(""));
    let output = if directory.join("install.sh").is_file() {
        run_installer(&directory, "uninstall")?
    } else {
        manual_uninstall()
    };
    let _ = std::fs::remove_file(paths::engine_meta());
    Ok(json!({ "installed": false, "removed": true, "output": output, "status": detect() }))
}

pub fn log(lines: usize) -> Value {
    let path = paths::steam_log();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return json!({ "path": path.display().to_string(), "lines": [], "error": "Steam console log not found" });
    };
    let tail: Vec<&str> = text.lines().rev().take(6000).collect();
    let mut picked = Vec::new();
    let mut skipped = 0;
    for line in tail.into_iter().rev() {
        if !line.contains(LOG_TAG) {
            continue;
        }
        if line.contains(NOISE_TAG) {
            skipped += 1;
            continue;
        }
        picked.push(line.to_string());
    }
    let total = picked.len();
    let keep = lines.clamp(1, 1000);
    let start = total.saturating_sub(keep);
    json!({
        "path": path.display().to_string(),
        "lines": picked[start..].to_vec(),
        "total": total,
        "skipped": skipped,
    })
}

pub fn check_script(text: &str, path_hint: &str) -> Value {
    let tool = luac();
    let Some(tool) = tool else {
        return json!({ "path": path_hint, "ok": false, "tool": null, "output": "luac not installed", "lint": [] });
    };
    let dir = paths::cache_home().join("lua-check");
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join(format!("{}.lua", std::process::id()));
    if std::fs::write(&target, text).is_err() {
        return json!({ "path": path_hint, "ok": false, "tool": tool, "output": "cannot write temp file", "lint": [] });
    }
    let output = Command::new(&tool).arg("-p").arg(&target).output();
    let _ = std::fs::remove_file(&target);
    match output {
        Ok(result) => {
            let mut text = String::from_utf8_lossy(&result.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&result.stderr));
            json!({
                "path": path_hint,
                "ok": result.status.success(),
                "tool": tool,
                "output": text.trim(),
                "lint": [],
            })
        }
        Err(error) => json!({ "path": path_hint, "ok": false, "tool": tool, "output": error.to_string(), "lint": [] }),
    }
}