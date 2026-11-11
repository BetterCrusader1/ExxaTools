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

pub fn run() -> Value {
    let status = engine::detect();
    let st = state::load().unwrap_or_default();
    let installed = status.get("installed").and_then(Value::as_bool).unwrap_or(false);
    let loaded = status.get("steam_engine_loaded").and_then(Value::as_bool).unwrap_or(false);
    let audit = status.get("ld_audit").and_then(Value::as_str).unwrap_or("").to_string();
    let running = steamctl::is_running();
    let mut items = Vec::new();

    items.push(item(
        "engine",
        "Vapor Forge installed",
        if installed { "ok" } else { "bad" },
        status.get("version").and_then(Value::as_str).unwrap_or("not installed").to_string(),
        "Installs both architecture libraries and wires LD_AUDIT",
        if installed { "" } else { "engine_install" },
    ));

    items.push(item(
        "engine_loaded",
        "Engine loaded inside Steam",
        if loaded { "ok" } else if running { "warn" } else { "warn" },
        if loaded { "LD_AUDIT picked it up".into() } else if running { "Steam is running without the shim".into() } else { "Steam is not running".into() },
        "Restart Steam so the client loads the engine",
        if loaded { "" } else { "restart_steam" },
    ));

    items.push(item(
        "ld_audit",
        "LD_AUDIT wired",
        if audit.is_empty() { "bad" } else { "ok" },
        if audit.is_empty() { "no library path resolved".into() } else { audit },
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
    let provider_installed = provider.get("installed").and_then(Value::as_bool).unwrap_or(false);
    items.push(item(
        "provider",
        "Manifest provider installed",
        if provider_installed { "ok" } else { "warn" },
        provider.get("path").and_then(Value::as_str).unwrap_or("").to_string(),
        "Without it each depot needs a code copied into the plan",
        if provider_installed { "" } else { "provider_install" },
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
        for depot in record.get("depots").and_then(Value::as_array).cloned().unwrap_or_default() {
            let Some(id) = depot.get("depot_id").and_then(Value::as_u64) else { continue };
            if !st.depot_keys.contains_key(&id.to_string()) {
                unkeyed.push(format!("{appid}/{id}"));
            }
        }
    }
    items.push(item(
        "keys",
        "Depot keys",
        if unkeyed.is_empty() { "ok" } else { "warn" },
        if unkeyed.is_empty() { "every planned depot is keyed".into() } else { format!("missing: {}", unkeyed.join(", ")) },
        "Encrypted depots do not download without a key",
        "",
    ));

    let sweep = crate::watch::sweep(false);
    let pending = sweep.get("pending").and_then(Value::as_array).cloned().unwrap_or_default();
    items.push(item(
        "appmanifest",
        "appmanifest flags",
        if pending.is_empty() { "ok" } else { "warn" },
        if pending.is_empty() { "all installs look finished".into() } else { format!("{} manifest(s) still marked unfinished", pending.len()) },
        "The stop-loop fix runs once Steam is closed",
        if pending.is_empty() { "" } else { "watch" },
    ));

    let libraries = appmanifest::libraries();
    let disk = libraries.first().and_then(|path| free_bytes(path)).map(human).unwrap_or_else(|| "unknown".into());
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
        if running { "running".into() } else { "not running".into() },
        "Plans load on the next start",
        "",
    ));

    let debug = steamui::debug_ready();
    items.push(item(
        "debug_port",
        "Steam debugging port",
        if debug { "ok" } else { "warn" },
        if debug { format!("127.0.0.1:{}", steamui::DEBUG_PORT) } else { "closed".into() },
        "Needed for the menu next to Help",
        if debug { "" } else { "steamui_enable" },
    ));

    let hubcap = !st.settings.hubcap_key.is_empty();
    items.push(item(
        "hubcap",
        "Hubcap key",
        if hubcap { "ok" } else { "warn" },
        if hubcap { "configured".into() } else { "not set".into() },
        "Optional: it fills depot keys and pinned manifests for you",
        "",
    ));

    let ok = items.iter().all(|entry| entry.get("status").and_then(Value::as_str) != Some("bad"));
    let warnings = items.iter().filter(|entry| entry.get("status").and_then(Value::as_str) == Some("warn")).count();
    json!({ "ok": ok, "warnings": warnings, "items": items })
}