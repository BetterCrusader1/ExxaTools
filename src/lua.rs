//! Render Vapor Forge Lua scripts: a plan for one app, and the provider that
//! answers manifest-code requests at download time.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::providers;

pub const PROVIDER_FILE: &str = "zz-exxatools-provider.lua";
const PROVIDER_TEMPLATE: &str = include_str!("../assets/provider.lua");

fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_secs())
        .unwrap_or(0);
    format!("{}", now)
}

pub fn render_plan(
    appid: u64,
    name: &str,
    branch: &str,
    depots: &[Value],
    keys: &BTreeMap<String, String>,
    dlc: &[u64],
    source: &str,
) -> String {
    let depot_ids: Vec<String> = depots
        .iter()
        .filter_map(|depot| depot.get("depot_id").and_then(value_to_id))
        .collect();
    let mut lines: Vec<String> = vec![
        "-- ExxaTools plan".into(),
        format!("-- @appid {appid}"),
        format!("-- @name {}", name.trim_end()),
        format!("-- @branch {branch}"),
        format!("-- @source {source}"),
        format!("-- @depots {}", depot_ids.join(",")),
        format!("-- generated {}", stamp()),
        String::new(),
        format!("addappid({appid})"),
    ];

    let keyed: Vec<&Value> = depots
        .iter()
        .filter(|depot| {
            depot
                .get("depot_id")
                .and_then(value_to_id)
                .and_then(|id| keys.get(&id))
                .is_some()
        })
        .collect();
    if !keyed.is_empty() {
        lines.push(String::new());
        lines.push("-- depot decryption keys".into());
        for depot in keyed {
            if let Some(depot_id) = depot.get("depot_id").and_then(value_to_id) {
                if let Some(key) = keys.get(&depot_id) {
                    lines.push(format!("addappid({depot_id}, 1, {})", quote(key)));
                }
            }
        }
    }

    let pinned: Vec<&Value> = depots
        .iter()
        .filter(|depot| depot.get("gid").and_then(value_to_gid).is_some())
        .collect();
    if !pinned.is_empty() {
        lines.push(String::new());
        lines.push("-- manifest pinning".into());
        for depot in pinned {
            let Some(depot_id) = depot.get("depot_id").and_then(value_to_id) else { continue };
            let Some(gid) = depot.get("gid").and_then(value_to_gid) else { continue };
            match depot.get("size").and_then(Value::as_u64) {
                Some(size) if size > 0 => lines.push(format!("setmanifestid({depot_id}, {}, {size})", quote(&gid))),
                _ => lines.push(format!("setmanifestid({depot_id}, {})", quote(&gid))),
            }
        }
    }

    if !dlc.is_empty() {
        lines.push(String::new());
        lines.push("-- DLC".into());
        for dlc_id in dlc {
            lines.push(format!("addappid({dlc_id})"));
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Depot and app ids arrive as numbers from appinfo and as strings from state.
pub fn value_to_id(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.trim().to_string()),
        _ => None,
    }
}

pub fn value_to_gid(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

pub fn render_provider(order: &[Value]) -> Result<String, String> {
    let mut rows: Vec<String> = Vec::new();
    for entry in order {
        let Some(spec) = providers::resolve(entry) else { continue };
        let mut parts = vec![
            format!("name = {}", quote(&spec.name)),
            format!("url = {}", quote(&spec.url)),
            format!("kind = {}", quote(&spec.format)),
        ];
        if let Some(field) = &spec.json_field {
            parts.push(format!("field = {}", quote(field)));
        }
        if let Some(agent) = &spec.user_agent {
            parts.push(format!("agent = {}", quote(agent)));
        }
        rows.push(format!("  {{ {} }},", parts.join(", ")));
    }
    if rows.is_empty() {
        return Err("no usable manifest provider configured".into());
    }
    let block = format!("{{\n{}\n}}", rows.join("\n"));
    Ok(PROVIDER_TEMPLATE.replace("__STAMP__", &stamp()).replace("__SOURCES__", &block))
}