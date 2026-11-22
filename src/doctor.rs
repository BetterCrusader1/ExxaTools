//! Doctor: one list that answers "why is it not working".

use serde_json::{json, Value};

use crate::appmanifest;
use crate::engine;
use crate::paths;
use crate::state;
use crate::steamctl;
use crate::steamui;
use crate::vfsys;

fn item(id: &str, label: &str, status: &str, detail: String, hint: &str, action: &str) -> Value {
    json!({
        "id": id,
        "label": label,
        "status": status,
        "detail": detail,
        "hint": hint,
        "action": if action.is_empty() { Value::Null } else { json!(action) },
    })
}

fn free_bytes(path: &std::path::Path) -> Option<u64> {
    let output = std::process::Command::new("df")
        .args(["-B1", "--output=avail"])
        .arg(path)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().nth(1)?.trim().parse().ok()
}

fn human(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut index = 0;
    while value >= 1024.0 && index + 1 < units.len() {
        value /= 1024.0;
        index += 1;
    }
    format!("{value:.1} {}", units[index])
}

/// Severity and wording for the engine check.
///
/// `loaded` must mean the shim is mapped inside the process that owns
/// `steamclient.so`; `shim_mapped` only means some Steam process mapped it.
/// Neither reading alone is enough to call the engine live.
struct EngineCheck {
    status: &'static str,
    detail: String,
    hint: &'static str,
    action: &'static str,
}

fn engine_check(loaded: bool, shim_mapped: bool, running: bool) -> EngineCheck {
    if loaded {
        return EngineCheck {
            status: "ok",
            detail: "the shim is mapped in the Steam client process".into(),
            hint: "Plans do not apply until the client itself loads the engine",
            action: "",
        };
    }
    EngineCheck {
        status: "bad",
        detail: if !running {
            "Steam is not running".into()
        } else if shim_mapped {
            "the shim is mapped, but not in the process that owns steamclient.so".into()
        } else {
            "Steam runs in a container that does not load the shim".into()
        },
        hint: "Plans do not apply until the client itself loads the engine",
        action: "engine_container",
    }
}

pub fn run() -> Value {
    let status = engine::detect();
    let st = state::load().unwrap_or_default();
    let installed = status
        .get("installed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let loaded = status
        .get("steam_engine_loaded")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let audit = status
        .get("ld_audit")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let running = steamctl::is_running();
    let mut items = Vec::new();

    items.push(item(
        "engine",
        "Vapor Forge installed",
        if installed { "ok" } else { "bad" },
        status
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("not installed")
            .to_string(),
        "Installs both architecture libraries and wires LD_AUDIT",
        if installed { "" } else { "engine_install" },
    ));

    // Steam's runtime launches the client inside pressure-vessel. LD_AUDIT stays
    // in the wrapper but the library is not mapped into the process that owns
    // steamclient.so, so the licence and manifest hooks never install. Report
    // that state instead of a green "loaded" that only proves the variable.
    let shim_mapped = status
        .get("steam_engine_mapped")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let engine = engine_check(loaded, shim_mapped, running);
    items.push(item(
        "engine_loaded",
        "Engine loaded inside Steam",
        engine.status,
        engine.detail,
        engine.hint,
        engine.action,
    ));

    let ui = steamui::status();
    let autostart = ui
        .get("autostart")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let wrapper = ui
        .get("launcher_wrapper")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    items.push(item(
        "lifecycle",
        "Starts with Steam",
        if autostart && wrapper { "ok" } else { "warn" },
        match (autostart, wrapper) {
            (true, true) => "autostart and the Steam launcher wrapper are in place".into(),
            (false, true) => "Steam starts ExxaTools, but nothing starts it at login".into(),
            (true, false) => {
                "autostart exists, but the Steam icon still starts Steam directly".into()
            }
            (false, false) => {
                "neither autostart nor the Steam launcher wrapper is installed".into()
            }
        },
        "Run `exxa autostart install` and `exxa steamui install`",
        "",
    ));

    items.push(item(
        "ld_audit",
        "Engine handed to Steam",
        if audit.is_empty() { "bad" } else { "ok" },
        if audit.is_empty() {
            "no library path resolved".into()
        } else {
            audit
        },
        "The engine must be in the launcher environment, not just on disk",
        "",
    ));

    let scripts_writable = {
        std::fs::create_dir_all(paths::vf_scripts()).is_ok()
            && std::fs::write(paths::vf_scripts().join(".doctor"), b"x").is_ok()
            && std::fs::remove_file(paths::vf_scripts().join(".doctor")).is_ok()
    };
    items.push(item(
        "scripts",
        "Script directory writable",
        if scripts_writable { "ok" } else { "bad" },
        paths::vf_scripts().display().to_string(),
        "Vapor Forge reads plans from here; fix permissions if it is read-only",
        "",
    ));

    let provider = vfsys::provider_state();
    let provider_installed = provider
        .get("installed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    items.push(item(
        "provider",
        "Manifest provider installed",
        if provider_installed { "ok" } else { "warn" },
        provider
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "Without it each depot needs a code copied into the plan",
        if provider_installed {
            ""
        } else {
            "provider_install"
        },
    ));

    let plans: Vec<String> = std::fs::read_dir(paths::vf_scripts())
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
                .filter(|name| name.starts_with("exxatools-") && name.ends_with(".lua"))
                .collect()
        })
        .unwrap_or_default();
    items.push(item(
        "plans",
        "Lua plans installed",
        if plans.is_empty() { "warn" } else { "ok" },
        format!("{} plan file(s)", plans.len()),
        "Save a plan and press Install to Vapor Forge",
        "",
    ));

    let mut unkeyed: Vec<String> = Vec::new();
    for (appid, record) in &st.apps {
        for depot in record
            .get("depots")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let Some(id) = depot.get("depot_id").and_then(Value::as_u64) else {
                continue;
            };
            if !st.depot_keys.contains_key(&id.to_string()) {
                unkeyed.push(format!("{appid}/{id}"));
            }
        }
    }
    items.push(item(
        "keys",
        "Depot keys",
        if unkeyed.is_empty() { "ok" } else { "warn" },
        if unkeyed.is_empty() {
            "every planned depot is keyed".into()
        } else {
            format!("missing: {}", unkeyed.join(", "))
        },
        "Encrypted depots do not download without a key",
        "",
    ));

    let sweep = crate::watch::sweep(false);
    let pending = sweep
        .get("pending")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    items.push(item(
        "appmanifest",
        "Steam update flags",
        if pending.is_empty() { "ok" } else { "warn" },
        if pending.is_empty() {
            "all installs look finished".into()
        } else {
            format!("{} manifest(s) still marked unfinished", pending.len())
        },
        "The stop-loop fix runs once Steam is closed",
        if pending.is_empty() { "" } else { "watch" },
    ));

    let libraries = appmanifest::libraries();
    let disk = libraries
        .first()
        .and_then(|path| free_bytes(path))
        .map(human)
        .unwrap_or_else(|| "unknown".into());
    items.push(item(
        "disk",
        "Free space on the library",
        "ok",
        disk,
        "Preallocating stalls when the library runs out of room",
        "",
    ));

    items.push(item(
        "steam",
        "Steam client",
        if running { "ok" } else { "warn" },
        if running {
            "running".into()
        } else {
            "not running".into()
        },
        "Plans load on the next start",
        "",
    ));

    let debug = steamui::debug_ready();
    items.push(item(
        "debug_port",
        "Steam debugging port",
        if debug { "ok" } else { "warn" },
        if debug {
            format!("127.0.0.1:{}", steamui::DEBUG_PORT)
        } else {
            "closed".into()
        },
        "Needed for the menu next to Help",
        if debug { "" } else { "steamui_enable" },
    ));

    let hubcap = !st.settings.hubcap_key.is_empty();
    items.push(item(
        "hubcap",
        "Hubcap key",
        if hubcap { "ok" } else { "warn" },
        if hubcap {
            "configured".into()
        } else {
            "not set".into()
        },
        "Optional: it fills depot keys and pinned manifests for you",
        "",
    ));

    let ok = items
        .iter()
        .all(|entry| entry.get("status").and_then(Value::as_str) != Some("bad"));
    let warnings = items
        .iter()
        .filter(|entry| entry.get("status").and_then(Value::as_str) == Some("warn"))
        .count();
    json!({ "ok": ok, "warnings": warnings, "items": items })
}
#[cfg(test)]
mod tests {
    use super::engine_check;

    #[test]
    fn engine_is_ok_only_when_mapped_in_the_client() {
        assert_eq!(engine_check(true, true, true).status, "ok");
        // The variable shows up in the wrapper even when the client container
        // never loads the library; that must not read as healthy.
        let wrapper_only = engine_check(false, true, true);
        assert_eq!(wrapper_only.status, "bad");
        assert_eq!(wrapper_only.action, "engine_container");
        assert!(wrapper_only.detail.contains("steamclient.so"));
        let nowhere = engine_check(false, false, true);
        assert_eq!(nowhere.status, "bad");
    }
}
