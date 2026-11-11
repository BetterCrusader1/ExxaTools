//! Post-apply checks: would this plan actually load in Steam?

use serde_json::{json, Value};

use crate::appmanifest;
use crate::engine;
use crate::state;
use crate::steamctl;
use crate::vfsys;

const ALLOWED_CALLS: &[&str] = &[
    "addappid",
    "setmanifestid",
    "setappticket",
    "seteticket",
    "setstat",
    "setavatar",
    "addtoken",
];

pub fn lint_lua(text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.split("--").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if !ALLOWED_CALLS.iter().any(|call| line.starts_with(call)) {
            problems.push(format!("line {}: unexpected statement", number + 1));
        }
    }
    problems
}

fn item(id: &str, label: &str, status: &str, detail: String) -> Value {
    json!({ "id": id, "label": label, "status": status, "detail": detail })
}

pub fn run(appid: u64) -> Value {
    let st = state::load().unwrap_or_default();
    let status = engine::detect();
    let mut items = Vec::new();

    let installed = status.get("installed").and_then(Value::as_bool).unwrap_or(false);
    let libs = status
        .get("libs")
        .and_then(Value::as_object)
        .map(|map| {
            map.values()
                .filter_map(Value::as_array)
                .flatten()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    items.push(item(
        "vapor_forge",
        "Vapor Forge libraries",
        if installed { "ok" } else { "bad" },
        if libs.is_empty() { "not installed".into() } else { libs.join(", ") },
    ));

    let audit = status.get("ld_audit").and_then(Value::as_str).unwrap_or("");
    items.push(item(
        "ld_audit",
        "LD_AUDIT wired for Steam",
        if audit.is_empty() { "bad" } else { "ok" },
        if audit.is_empty() { "no drop-in, .env.steam, or launcher entry".into() } else { audit.to_string() },
    ));

    let Some(plan) = st.apps.get(&appid.to_string()).cloned() else {
        items.push(item("plan", "Saved plan", "bad", "no plan for this app yet".into()));
        return json!({ "appid": appid, "items": items, "ok": false });
    };

    let script = vfsys::script_path(appid);
    let disabled = vfsys::disabled_path(appid);
    let (state_label, detail) = if script.exists() {
        ("ok", script.display().to_string())
    } else if disabled.exists() {
        ("warn", format!("disabled: {}", disabled.display()))
    } else {
        ("bad", "not installed into Vapor Forge".into())
    };
    items.push(item("script", "Lua plan installed", state_label, detail));

    let text = crate::lua::render_plan(
        appid,
        plan.get("name").and_then(Value::as_str).unwrap_or(""),
        plan.get("branch").and_then(Value::as_str).unwrap_or("public"),
        plan.get("depots").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]),
        &st.depot_keys,
        &plan
            .get("dlc")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
            .unwrap_or_default(),
        "auto",
    );
    let problems = lint_lua(&text);
    items.push(item(
        "lua",
        "Lua plan parses",
        if problems.is_empty() { "ok" } else { "bad" },
        if problems.is_empty() { format!("{} lines", text.lines().count()) } else { problems.iter().take(4).cloned().collect::<Vec<_>>().join("; ") },
    ));

    let depots = plan.get("depots").and_then(Value::as_array).cloned().unwrap_or_default();
    let missing: Vec<String> = depots
        .iter()
        .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
        .filter(|id| !st.depot_keys.contains_key(&id.to_string()))
        .map(|id| id.to_string())
        .collect();
    items.push(item(
        "keys",
        "Depot keys",
        if missing.is_empty() { "ok" } else { "warn" },
        if missing.is_empty() { "all depots keyed".into() } else { format!("missing: {}", missing.join(", ")) },
    ));

    let resolved = depots
        .iter()
        .filter(|depot| depot.get("gid").and_then(Value::as_str).map(|gid| st.codes.contains_key(gid)).unwrap_or(false))
        .count();
    items.push(item(
        "codes",
        "Manifest request codes",
        if resolved == depots.len() { "ok" } else { "warn" },
        format!("{resolved}/{} resolved (Vapor Forge can also fetch them itself)", depots.len()),
    ));

    if let Some(fixes) = plan.get("fixes").and_then(Value::as_object) {
        if fixes.get("online_fix").and_then(Value::as_bool).unwrap_or(false) {
            let current = steamctl::read_launch_options(appid).unwrap_or_default();
            let present = current.split_whitespace().any(|part| part == "-onlinefix");
            items.push(item(
                "launch_options",
                "Launch option -onlinefix",
                if present { "ok" } else { "warn" },
                if current.is_empty() { "not set".into() } else { current },
            ));
        }
    }

    let flags = appmanifest::report(appid, Some(&plan));
    let flag_value = flags.get("state_flags").and_then(Value::as_u64).unwrap_or(0);
    if flags.get("installed").and_then(Value::as_bool).unwrap_or(false) {
        items.push(item(
            "appmanifest",
            "appmanifest flags",
            if flag_value & 4 != 0 { "ok" } else { "warn" },
            format!("StateFlags {flag_value}"),
        ));
    }

    items.push(item(
        "steam",
        "Steam client",
        if steamctl::is_running() { "warn" } else { "ok" },
        if steamctl::is_running() { "running - restart it to load the plan".into() } else { "not running".into() },
    ));

    let ok = items
        .iter()
        .all(|entry| entry.get("status").and_then(Value::as_str) != Some("bad"));
    json!({ "appid": appid, "items": items, "ok": ok })
}

/// Merge a launch option into an existing string without duplicating it.
pub fn merge_launch_option(current: Option<&str>, flag: &str) -> String {
    let text = current.unwrap_or("").trim().to_string();
    if flag.is_empty() {
        return text;
    }
    if text.split_whitespace().any(|part| part == flag) {
        return text;
    }
    format!("{text} {flag}").trim().to_string()
}
