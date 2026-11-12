//! A loopback HTTP server: the JSON API and the web UI, one binary.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::appmanifest;
use crate::engine;
use crate::hubcap;
use crate::lua;
use crate::paths;
use crate::state;
use crate::steam;
use crate::steamctl;
use crate::vfsys;

pub struct Config {
    pub port: u16,
}

static PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

/// The port the API is actually listening on, so injected scripts agree.
pub fn port() -> u16 {
    *PORT.get().unwrap_or(&8765)
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "woff2" => "font/woff2",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn respond_json(stream: &mut TcpStream, status: u16, value: &Value) {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    respond(stream, status, "application/json; charset=utf-8", &body);
}

/// Percent-decoded query value, so a game name with spaces or an ampersand
/// arrives as typed instead of as escape codes.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => { out.push(b' '); index += 1; }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
                match hex.and_then(|text| u8::from_str_radix(text, 16).ok()) {
                    Some(byte) => { out.push(byte); index += 3; }
                    None => { out.push(bytes[index]); index += 1; }
                }
            }
            byte => { out.push(byte); index += 1; }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("").to_string();
            let value = decode(parts.next().unwrap_or(""));
            (key, value)
        })
        .collect()
}

fn get<'a>(query: &'a [(String, String)], key: &str) -> Option<&'a str> {
    query.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
}

fn need_u64(query: &[(String, String)], key: &str) -> Result<u64, (u16, String)> {
    get(query, key)
        .and_then(|value| value.parse().ok())
        .ok_or((400, format!("{key} must be an integer")))
}

fn body_u64(body: &Value, key: &str) -> Result<u64, (u16, String)> {
    body.get(key)
        .and_then(|value| value.as_u64().or_else(|| value.as_str().and_then(|text| text.parse().ok())))
        .ok_or((400, format!("{key} must be an integer")))
}

fn render_record(record: &Value, st: &state::State) -> String {
    lua::render_record(record, &st.depot_keys)
}

fn app_view(appid: u64, branch: &str) -> Result<Value, (u16, String)> {
    let catalog = steam::catalog(appid).map_err(|error| (502, error))?;
    let st = state::load().map_err(|error| (500, error))?;
    let mut depots = Vec::new();
    if let Some(list) = catalog.get("depots").and_then(Value::as_array) {
        for depot in list {
            let depot_id = depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0);
            let manifest = steam::pick_manifest(depot, branch);
            let key = st.depot_keys.get(&depot_id.to_string()).cloned();
            let mut names: Vec<String> = depot
                .get("manifests")
                .and_then(Value::as_object)
                .map(|map| map.keys().cloned().collect())
                .unwrap_or_default();
            names.sort();
            depots.push(json!({
                "depot_id": depot_id,
                "oslist": depot.get("oslist").cloned().unwrap_or_else(|| json!([])),
                "osarch": depot.get("osarch").cloned().unwrap_or_else(|| json!("")),
                "shared_install": depot.get("shared_install").cloned().unwrap_or(json!(false)),
                "dlc_appid": depot.get("dlc_appid").cloned().unwrap_or(Value::Null),
                "gid": manifest.as_ref().and_then(|value| value.get("gid")).cloned().unwrap_or_else(|| json!("")),
                "size": manifest.as_ref().and_then(|value| value.get("size")).cloned().unwrap_or(Value::Null),
                "download": manifest.as_ref().and_then(|value| value.get("download")).cloned().unwrap_or(Value::Null),
                "branches": names,
                "has_key": key.is_some(),
                "key": key,
                "code": manifest.as_ref().and_then(|value| value.get("gid")).and_then(Value::as_str)
                    .and_then(|gid| st.codes.get(gid)).cloned(),
            }));
        }
    }
    let has_dlc = catalog.get("dlc").and_then(Value::as_array).map(|list| !list.is_empty()).unwrap_or(false);
    let dlc_names = if has_dlc { steam::dlc_names(appid, &st.settings.country) } else { Vec::new() };
    Ok(json!({
        "appid": appid,
        "name": catalog.get("name").cloned().unwrap_or(Value::Null),
        "type": catalog.get("type").cloned().unwrap_or(Value::Null),
        "developers": catalog.get("developers").cloned().unwrap_or_else(|| json!("")),
        "branch": branch,
        "branches": catalog.get("branches").cloned().unwrap_or_else(|| json!([])),
        "dlc": catalog.get("dlc").cloned().unwrap_or_else(|| json!([])),
        "dlc_names": dlc_names,
        "depots": depots,
        "plan": st.apps.get(&appid.to_string()).cloned(),
        "installed": vfsys::script_path(appid).exists(),
        "source": st.settings.default_source,
        "sources": crate::sources::catalog(&st.settings),
        "launch_options": steamctl::read_launch_options(appid),
        "catalog": catalog,
    }))
}

fn record_from_body(body: &Value) -> Result<Value, (u16, String)> {
    let appid = body_u64(body, "appid")?;
    let depots_raw = body.get("depots").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut depots = Vec::new();
    for item in depots_raw {
        let depot_id = item
            .get("depot_id")
            .and_then(|value| value.as_u64().or_else(|| value.as_str().and_then(|text| text.parse().ok())))
            .ok_or((400, "each depot needs a depot_id".to_string()))?;
        let gid = item.get("gid").and_then(Value::as_str).unwrap_or("").to_string();
        if !gid.is_empty() && !gid.chars().all(|ch| ch.is_ascii_digit()) {
            return Err((400, format!("depot {depot_id}: gid must be decimal")));
        }
        let size = item.get("size").and_then(Value::as_u64);
        let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        depots.push(json!({ "depot_id": depot_id, "gid": gid, "size": size, "enabled": enabled }));
    }
    let dlc: Vec<u64> = body
        .get("dlc")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(|value| value.as_u64().or_else(|| value.as_str().and_then(|text| text.parse().ok()))).collect())
        .unwrap_or_default();
    let fixes = body.get("fixes").cloned().filter(Value::is_object).unwrap_or_else(|| {
        json!({ "ticket": "forge", "steam_id": "", "avatar": 0, "purchase_time": 0, "online_fix": false })
    });
    // A catalog snapshot travels with the plan so a refresh still works when the
    // Steam API is unreachable.
    let catalog = body.get("catalog").cloned().filter(Value::is_object);
    let mut record = json!({
        "appid": appid,
        "name": body.get("name").and_then(Value::as_str).unwrap_or("").trim(),
        "branch": body.get("branch").and_then(Value::as_str).unwrap_or("public"),
        "depots": depots,
        "dlc": dlc,
        "source": body.get("source").and_then(Value::as_str).unwrap_or("auto"),
        "fixes": fixes,
        "updated": vfsys::stamp(),
    });
    if let Some(catalog) = catalog {
        record["catalog"] = catalog;
    }
    Ok(record)
}

fn get_route(path: &str, query: &[(String, String)]) -> Result<Value, (u16, String)> {
    let st = state::load().map_err(|error| (500, error))?;
    match path {
        "/api/session" => Ok(json!({ "token": token(), "version": env!("CARGO_PKG_VERSION") })),
        "/api/diagnostics" => Ok(crate::diagnostics::build()),
        "/api/status" => Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "steam": {
                "root": steamctl::root().map(|value| value.display().to_string()),
                "running": steamctl::is_running(),
            },
            "engine": engine::detect(),
            "apps": st.apps.values().map(|record| {
                let mut view = record.clone();
                if let Some(summary) = state::app_summary(&st, record).as_object() {
                    for (key, value) in summary {
                        view[key] = value.clone();
                    }
                }
                view
            }).collect::<Vec<_>>(),
            "settings": st.settings,
            "state_path": state::state_path().display().to_string(),
            "keys": st.depot_keys.len(),
        })),
        "/api/engine" => Ok(engine::detect()),
        "/api/engine/releases" => Ok(json!({ "releases": engine::releases(5).map_err(|error| (502, error))? })),
        "/api/log" => {
            let lines = get(query, "lines").and_then(|value| value.parse().ok()).unwrap_or(200);
            Ok(engine::log(lines))
        }
        "/api/discover" => {
            let limit = get(query, "limit").and_then(|value| value.parse().ok()).unwrap_or(18);
            let cc = get(query, "cc").unwrap_or(&st.settings.country).to_string();
            Ok(json!({ "items": steam::discover(&cc, limit) }))
        }
        "/api/search" => {
            let term = get(query, "q").unwrap_or("");
            let items = steam::search(term, &st.settings.country).map_err(|error| (502, error))?;
            Ok(json!({ "items": items }))
        }
        "/api/app" => {
            let appid = need_u64(query, "appid")?;
            let branch = get(query, "branch").unwrap_or("public").to_string();
            app_view(appid, &branch)
        }
        "/api/appmanifest" => {
            let appid = need_u64(query, "appid")?;
            let plan = st.apps.get(&appid.to_string()).cloned();
            Ok(appmanifest::report(appid, plan.as_ref()))
        }
        "/api/key" => {
            let depot_id = need_u64(query, "depot_id")?;
            Ok(json!({ "depot_id": depot_id, "key": st.depot_keys.get(&depot_id.to_string()) }))
        }
        "/api/plan" => {
            let appid = need_u64(query, "appid")?;
            st.apps.get(&appid.to_string()).cloned().map(Ok).unwrap_or(Err((404, "no saved plan".into())))
        }
        "/api/script" => {
            let appid = need_u64(query, "appid")?;
            let record = st.apps.get(&appid.to_string()).ok_or((404, "no saved plan".to_string()))?;
            Ok(json!({
                "appid": appid,
                "script": render_record(record, &st),
                "installed": vfsys::script_path(appid).exists(),
            }))
        }
        "/api/settings" => Ok(json!(st.settings)),
        "/api/sources" => Ok(json!({
            "sources": crate::sources::catalog(&st.settings),
            "default": st.settings.default_source,
        })),
        "/api/dlc" => {
            let appid = need_u64(query, "appid")?;
            Ok(json!({ "items": steam::dlc_names(appid, &st.settings.country) }))
        }
        "/api/autoselect" => {
            let appid = need_u64(query, "appid")?;
            let branch = get(query, "branch").unwrap_or("public").to_string();
            let os = get(query, "os").unwrap_or(&st.settings.default_os).to_string();
            let catalog = steam::catalog(appid).map_err(|error| (502, error))?;
            let mut out = Vec::new();
            if let Some(list) = catalog.get("depots").and_then(Value::as_array) {
                for depot in list {
                    let depot_id = depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0);
                    let oslist: Vec<String> = depot.get("oslist").and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                    let shared = depot.get("shared_install").and_then(Value::as_bool).unwrap_or(false);
                    let manifest = steam::pick_manifest(depot, &branch);
                    let (selected, reason) = if manifest.is_none() {
                        (false, format!("no manifest in {branch}"))
                    } else if !oslist.is_empty() && !oslist.iter().any(|value| value == &os) {
                        (false, format!("only for {}", oslist.join(", ")))
                    } else if shared {
                        (true, "shared depot".to_string())
                    } else {
                        (true, if oslist.is_empty() { format!("platform-neutral, {branch}") } else { format!("{os} build, {branch}") })
                    };
                    out.push(json!({ "depot_id": depot_id, "selected": selected, "reason": reason }));
                }
            }
            Ok(json!({ "items": out, "os": os, "branch": branch }))
        }
        "/api/library" => {
            let items: Vec<Value> = st
                .apps
                .values()
                .map(|record| {
                    let mut view = state::app_summary(&st, record);
                    let appid = view.get("appid").and_then(Value::as_u64).unwrap_or(0);
                    view["enabled"] = view.get("installed").cloned().unwrap_or(json!(false));
                    view["launch_options"] = json!(steamctl::read_launch_options(appid));
                    view["cloud"] = steamctl::cloud_state(appid);
                    view
                })
                .collect();
            Ok(json!({
                "items": items,
                "keys": st.depot_keys.len(),
                "steam": { "running": steamctl::is_running(), "root": steamctl::root().map(|value| value.display().to_string()) },
                "engine": engine::detect(),
                "settings": st.settings,
            }))
        }
        "/api/cloud" => {
            let appid = need_u64(query, "appid")?;
            Ok(json!({ "appid": appid, "cloud": steamctl::cloud_state(appid) }))
        }
        "/api/doctor" => Ok(crate::doctor::run()),
        "/api/selftest" => Ok(crate::selftest::run()),
        "/api/diagnose" => Ok(crate::diagnose::from_files()),
        "/api/storage" => Ok(crate::storage::report()),
        "/api/pins" => {
            let appid = need_u64(query, "appid")?;
            let branch = get(query, "branch").unwrap_or("public").to_string();
            let mut st = state::load().map_err(|error| (500, error))?;
            let result = crate::pins::compare(&mut st, appid, &branch).map_err(|error| (502, error))?;
            if result.get("changed").and_then(Value::as_bool).unwrap_or(false) {
                state::save(&st).map_err(|error| (500, error))?;
            }
            Ok(result)
        }
        "/api/watch" => Ok(crate::watch::sweep(false)),
        "/api/verify" => {
            let appid = need_u64(query, "appid")?;
            Ok(crate::verify::run(appid))
        }
        "/api/launchoptions" => {
            let appid = need_u64(query, "appid")?;
            Ok(json!({
                "found": steamctl::localconfig_path().is_some(),
                "path": steamctl::localconfig_path().map(|path| path.display().to_string()),
                "options": steamctl::read_launch_options(appid),
            }))
        }
        "/api/provider" => {
            let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
            let text = lua::render_provider(&order).map_err(|error| (500, error))?;
            Ok(json!({ "text": text, "state": vfsys::provider_state() }))
        }
        "/api/steamui" => Ok(crate::steamui::status()),
        "/api/steam" => Ok(json!({
            "root": steamctl::root().map(|value| value.display().to_string()),
            "running": steamctl::is_running(),
        })),
        "/api/hubcap" => {
            let key = st.settings.hubcap_key.clone();
            let mut info = json!({
                "configured": !key.is_empty(),
                "hint": if key.len() > 8 { format!("{}...", &key[..8]) } else { String::new() },
                "providers": st.settings.manifest_providers,
            });
            if !key.is_empty() {
                match hubcap::validate(&key) {
                    Ok(stats) => info["stats"] = stats,
                    Err(error) => info["error"] = json!(error.to_string()),
                }
            }
            Ok(info)
        }
        _ => Err((404, format!("unknown endpoint GET {path}"))),
    }
}

fn post_route(path: &str, body: &Value, query: &[(String, String)]) -> Result<Value, (u16, String)> {
    let _ = query;
    match path {
        "/api/engine/install" => {
            let tag = body.get("tag").and_then(Value::as_str).filter(|value| !value.trim().is_empty());
            engine::install(tag).map_err(|error| (500, error))
        }
        "/api/engine/uninstall" => engine::uninstall().map_err(|error| (500, error)),
        "/api/depot" => {
            let appid = body_u64(body, "appid")?;
            let depot_id = body_u64(body, "depot_id")?;
            let mut st = state::load().map_err(|error| (500, error))?;
            let record = st.apps.get_mut(&appid.to_string()).ok_or((404, "no saved plan".to_string()))?;
            let list = record.get_mut("depots").and_then(Value::as_array_mut).ok_or((409, "plan has no depots".to_string()))?;
            let mut found = false;
            for depot in list.iter_mut() {
                if depot.get("depot_id").and_then(Value::as_u64) != Some(depot_id) { continue }
                if let Some(enabled) = body.get("enabled").and_then(Value::as_bool) {
                    depot["enabled"] = json!(enabled);
                }
                if let Some(gid) = body.get("gid").and_then(Value::as_str) {
                    depot["gid"] = json!(gid.trim());
                }
                found = true;
            }
            if !found {
                return Err((404, format!("depot {depot_id} is not in this plan")));
            }
            record["updated"] = json!(crate::vfsys::stamp());
            state::save(&st).map_err(|error| (500, error))?;
            let installed = crate::plans::write_script(appid).unwrap_or(false);
            Ok(json!({ "appid": appid, "depot_id": depot_id, "installed": installed }))
        }
        "/api/plan/fetch" => {
            let appid = body_u64(body, "appid")?;
            let name = body.get("name").and_then(Value::as_str);
            crate::plans::fetch_plan(appid, name).map_err(|error| (502, error))
        }
        "/api/plan" => {
            let record = record_from_body(body)?;
            let appid = record.get("appid").and_then(Value::as_u64).unwrap_or(0);
            let mut st = state::load().map_err(|error| (500, error))?;
            st.apps.insert(appid.to_string(), record.clone());
            state::save(&st).map_err(|error| (500, error))?;
            Ok(record)
        }
        "/api/install" => {
            let appid = body_u64(body, "appid")?;
            let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let st = state::load().map_err(|error| (500, error))?;
            let record = st.apps.get(&appid.to_string()).ok_or((409, "save a plan first".to_string()))?;
            let text = render_record(record, &st);
            let target = vfsys::write_script(appid, &text, enabled).map_err(|error| (500, error))?;
            let missing: Vec<u64> = record
                .get("depots")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
                        .filter(|id| !st.depot_keys.contains_key(&id.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Ok(json!({ "path": target.display().to_string(), "script": text, "missing_keys": missing }))
        }
        "/api/toggle" => {
            let appid = body_u64(body, "appid")?;
            let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            match vfsys::toggle_script(appid, enabled) {
                Some(target) => Ok(json!({
                    "path": target.display().to_string(),
                    "enabled": target.to_string_lossy().ends_with(".lua"),
                })),
                None => Err((404, "script not installed".into())),
            }
        }
        "/api/key" => {
            let depot_id = body_u64(body, "depot_id")?;
            let raw = body.get("key").and_then(Value::as_str).unwrap_or("");
            let key = state::valid_key(raw).ok_or((400, "a depot key is 64 hex characters".to_string()))?;
            let mut st = state::load().map_err(|error| (500, error))?;
            state::set_key(&mut st, depot_id, Some(&key));
            state::save(&st).map_err(|error| (500, error))?;
            Ok(json!({ "depot_id": depot_id, "key": key }))
        }
        "/api/script/check" => {
            let text = body.get("text").and_then(Value::as_str);
            match text {
                Some(text) if !text.trim().is_empty() => Ok(engine::check_script(text, "inline")),
                _ => {
                    let appid = body_u64(body, "appid")?;
                    let st = state::load().map_err(|error| (500, error))?;
                    let record = st.apps.get(&appid.to_string()).ok_or((409, "save a plan first".to_string()))?;
                    Ok(engine::check_script(&render_record(record, &st), &format!("{appid}.lua")))
                }
            }
        }
        "/api/provider" => {
            let st = state::load().map_err(|error| (500, error))?;
            let order: Vec<Value> = match body.get("providers").and_then(Value::as_array) {
                Some(list) if !list.is_empty() => list.clone(),
                _ => st.settings.manifest_providers.iter().map(|name| json!(name)).collect(),
            };
            let text = lua::render_provider(&order).map_err(|error| (500, error))?;
            let check = engine::check_script(&text, lua::PROVIDER_FILE);
            if !check.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                return Err((500, format!("provider script failed to parse: {}", check.get("output").and_then(Value::as_str).unwrap_or(""))));
            }
            let target = vfsys::write_provider(&text).map_err(|error| (500, error))?;
            Ok(json!({
                "path": target.display().to_string(),
                "state": vfsys::provider_state(),
                "check": check,
                "reload": "Vapor Forge hot-reloads its script directory; no restart needed",
            }))
        }
        "/api/appmanifest" => {
            let appid = body_u64(body, "appid")?;
            let mode = appmanifest::Mode::parse(body.get("mode").and_then(Value::as_str).unwrap_or("fix"))
                .map_err(|error| (400, error))?;
            let force = body.get("force").and_then(Value::as_bool).unwrap_or(false);
            let restart = body.get("restart").and_then(Value::as_bool).unwrap_or(false);
            if restart && steamctl::is_running() {
                let busy = appmanifest::running_apps();
                if !busy.is_empty() {
                    let names: Vec<String> = busy
                        .iter()
                        .map(|app| app.get("name").and_then(Value::as_str).unwrap_or("?").to_string())
                        .collect();
                    return Err((409, format!("a game is running ({}): close it before restarting Steam", names.join(", "))));
                }
                steamctl::shutdown(std::time::Duration::from_secs(45)).map_err(|error| (500, error))?;
                let mut result = appmanifest::patch(appid, &mode, false).map_err(|error| (500, error))?;
                steamctl::start(&[]).map_err(|error| (500, error))?;
                result["restarted"] = json!(true);
                return Ok(result);
            }
            appmanifest::patch(appid, &mode, !force).map_err(|error| (500, error))
        }
        "/api/fix" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or("");
            let appid = body.get("appid").and_then(Value::as_u64).unwrap_or(0);
            let st = state::load().map_err(|error| (500, error))?;
            match action {
                "manifest" => {
                    if appid == 0 { return Err((400, "appid is required".into())); }
                    let result = appmanifest::patch(appid, &appmanifest::Mode::Fix, true)
                        .map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": "manifest repaired", "result": result }))
                }
                "keys" => {
                    let mut written = 0;
                    for record in st.apps.values() {
                        let id = record.get("appid").and_then(Value::as_u64).unwrap_or(0);
                        if appid != 0 && id != appid { continue; }
                        let text = render_record(record, &st);
                        if vfsys::script_path(id).exists() && vfsys::write_script(id, &text, true).is_ok() {
                            written += 1;
                        }
                    }
                    let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
                    lua::render_provider(&order)
                        .and_then(|text| vfsys::write_provider(&text).map(|_| ()))
                        .map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": format!("{written} script(s) refreshed with the keys on disk") }))
                }
                "provider" => {
                    let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
                    let text = lua::render_provider(&order).map_err(|error| (500, error))?;
                    vfsys::write_provider(&text).map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": "provider script written" }))
                }
                "cloud" => {
                    if appid == 0 { return Err((400, "appid is required".into())); }
                    let result = steamctl::clear_cloud_state(appid).map_err(|error| (409, error))?;
                    Ok(json!({ "ok": true, "message": "cloud state reset, Steam will sync this game from scratch", "result": result }))
                }
                "launch" => {
                    if appid == 0 { return Err((400, "appid is required".into())); }
                    steamctl::set_launch_options(appid, "").map_err(|error| (409, error))?;
                    Ok(json!({ "ok": true, "message": "launch options cleared" }))
                }
                "plan" => {
                    let result = crate::plans::refresh_manifests().map_err(|error| (500, error))?;
                    let updated = result.get("updated").and_then(Value::as_u64).unwrap_or(0);
                    Ok(json!({
                        "ok": true,
                        "message": format!("{updated} plan(s) rebuilt from the current catalog"),
                        "result": result,
                    }))
                }
                "disk" => {
                    let report = crate::storage::report();
                    let lowest = report.get("lowest").cloned().unwrap_or(Value::Null);
                    let message = match lowest.get("free_human").and_then(Value::as_str) {
                        Some(free) => format!("Lowest free space: {free} ({}). Steam needs room for the download plus the unpacked build.", lowest.get("path").and_then(Value::as_str).unwrap_or("")),
                        None => "Could not read free space for the Steam libraries".to_string(),
                    };
                    Ok(json!({ "ok": true, "message": message, "result": report }))
                }
                "remove" => {
                    if appid == 0 { return Err((400, "appid is required".into())); }
                    vfsys::remove_script(appid);
                    let mut fresh = state::load().map_err(|error| (500, error))?;
                    fresh.apps.remove(&appid.to_string());
                    state::save(&fresh).map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": "removed from the library" }))
                }
                other => Err((400, format!("unknown fix {other}"))),
            }
        }
        "/api/steamui/panel" => {
            let open = body.get("open").and_then(Value::as_bool).unwrap_or(true);
            let toggle = body.get("toggle").and_then(Value::as_bool).unwrap_or(false);
            let x = body.get("x").and_then(Value::as_i64).unwrap_or(12);
            let y = body.get("y").and_then(Value::as_i64).unwrap_or(44);
            let screen_x = body.get("screen_x").and_then(Value::as_i64).unwrap_or(0);
            let screen_y = body.get("screen_y").and_then(Value::as_i64).unwrap_or(0);
            crate::steamui::panel(open, toggle, x, y, screen_x, screen_y).map_err(|error| (500, error))
        }
        "/api/steamui" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or("status");
            match action {
                "inject" => crate::steamui::inject().map_err(|error| (500, error)),
                "remove" => Ok(crate::steamui::remove()),
                "enable" => crate::steamui::enable_debug().map_err(|error| (500, error)),
                other => Err((400, format!("action must be inject, remove, or enable, not {other}"))),
            }
        }
        "/api/app/launch" => Ok(crate::desktop::launch_app()),
        "/api/desktop" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or("install");
            match action {
                "install" => crate::desktop::install_desktop_entry().map_err(|error| (500, error)),
                "uninstall" => Ok(crate::desktop::uninstall_desktop_entry()),
                other => Err((400, format!("action must be install or uninstall, not {other}"))),
            }
        }
        "/api/launchoptions" => {
            let appid = body_u64(body, "appid")?;
            let options = body.get("options").and_then(Value::as_str).unwrap_or("").to_string();
            steamctl::set_launch_options(appid, &options).map_err(|error| (400, error))
        }
        "/api/codes" => {
            let appid = body_u64(body, "appid")?;
            let depots = body.get("depots").and_then(Value::as_array).cloned().unwrap_or_default();
            let st = state::load().map_err(|error| (500, error))?;
            let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
            let mut out = Vec::new();
            let mut errors = Vec::new();
            for depot in depots {
                let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else { continue };
                let Some(gid) = depot.get("gid").and_then(Value::as_str).filter(|value| !value.is_empty()) else { continue };
                match crate::providers::fetch_code(appid, depot_id, gid, &order) {
                    Ok(result) => out.push(result),
                    Err(error) => errors.push(json!({ "depot_id": depot_id, "gid": gid, "error": error })),
                }
            }
            Ok(json!({ "codes": out, "errors": errors }))
        }
        "/api/pin" => {
            let appid = body_u64(body, "appid")?;
            let depot_id = body_u64(body, "depot_id")?;
            let gid = body.get("gid").and_then(Value::as_str).filter(|value| !value.is_empty());
            let mut st = state::load().map_err(|error| (500, error))?;
            let result = crate::pins::set_pin(&mut st, appid, depot_id, gid).map_err(|error| (400, error))?;
            state::save(&st).map_err(|error| (500, error))?;
            Ok(result)
        }
        "/api/watch" => {
            let apply = body.get("apply").and_then(Value::as_bool).unwrap_or(false);
            Ok(crate::watch::sweep(apply))
        }
        "/api/steam/restart" => steamctl::restart(&[]).map_err(|error| (500, error)),
        "/api/steam/shutdown" => {
            let stopped = steamctl::shutdown(std::time::Duration::from_secs(45)).map_err(|error| (500, error))?;
            Ok(json!({ "stopped": stopped }))
        }
        "/api/hubcap" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or("save");
            let mut st = state::load().map_err(|error| (500, error))?;
            match action {
                "save" => {
                    let raw = body.get("key").and_then(Value::as_str).unwrap_or("").trim().to_string();
                    if raw.is_empty() {
                        st.settings.hubcap_key.clear();
                        state::save(&st).map_err(|error| (500, error))?;
                        return Ok(json!({ "configured": false }));
                    }
                    let valid = hubcap::valid_key(&raw)
                        .ok_or((400, "a Hubcap key starts with smm and has no spaces".to_string()))?;
                    let stats = hubcap::validate(&valid).map_err(|error| (400, error.to_string()))?;
                    st.settings.hubcap_key = valid;
                    state::save(&st).map_err(|error| (500, error))?;
                    Ok(json!({ "configured": true, "stats": stats }))
                }
                "search" => {
                    if st.settings.hubcap_key.is_empty() {
                        return Err((409, "set a Hubcap API key first".into()));
                    }
                    let term = body.get("query").and_then(Value::as_str).unwrap_or("").trim().to_string();
                    if term.is_empty() {
                        return Err((400, "query is required".into()));
                    }
                    let items = hubcap::search(&st.settings.hubcap_key, &term, 25).map_err(|error| (502, error.to_string()))?;
                    Ok(json!({ "items": items }))
                }
                "fetch" => {
                    if st.settings.hubcap_key.is_empty() {
                        return Err((409, "set a Hubcap API key first".into()));
                    }
                    let appid = body_u64(body, "appid")?;
                    let fetched = hubcap::fetch(&st.settings.hubcap_key, appid).map_err(|error| (502, error.to_string()))?;
                    let parsed = hubcap::parse_lua(&fetched.lua, Some(appid));
                    let stamp = vfsys::stamp();
                    let summary = hubcap::apply_to_state(
                        &mut st,
                        &parsed,
                        &stamp,
                        body.get("name").and_then(Value::as_str),
                        body.get("pin").and_then(Value::as_bool).unwrap_or(false),
                        body.get("create").and_then(Value::as_bool).unwrap_or(false),
                    );
                    state::save(&st).map_err(|error| (500, error))?;
                    Ok(json!({ "summary": summary, "manifests": fetched.manifests, "source": fetched.source }))
                }
                other => Err((400, format!("action must be save, search, or fetch, not {other}"))),
            }
        }
        "/api/settings" => {
            let mut st = state::load().map_err(|error| (500, error))?;
            if let Some(object) = body.as_object() {
                let current = serde_json::to_value(&st.settings).unwrap_or_else(|_| json!({}));
                let mut merged: Map<String, Value> = current.as_object().cloned().unwrap_or_default();
                for (key, value) in object {
                    merged.insert(key.clone(), value.clone());
                }
                st.settings = serde_json::from_value(Value::Object(merged)).map_err(|error| (400, error.to_string()))?;
            }
            state::save(&st).map_err(|error| (500, error))?;
            Ok(json!(st.settings))
        }
        "/api/open" => {
            let target = body.get("target").and_then(Value::as_str).unwrap_or("");
            if target.is_empty() {
                return Err((400, "target is required".into()));
            }
            std::process::Command::new("xdg-open")
                .arg(target)
                .spawn()
                .map_err(|error| (500, format!("cannot open: {error}")))?;
            Ok(json!({ "opened": target }))
        }
        "/api/import" => {
            let text = body.get("text").and_then(Value::as_str).unwrap_or("");
            if text.trim().is_empty() {
                return Err((400, "text is required".into()));
            }
            let mut st = state::load().map_err(|error| (500, error))?;
            let parsed = hubcap::parse_lua(text, body.get("appid").and_then(Value::as_u64));
            if parsed.appid.is_none() {
                return Err((400, "that lua has no app id".into()));
            }
            let stamp = vfsys::stamp();
            let summary = hubcap::apply_to_state(
                &mut st,
                &parsed,
                &stamp,
                body.get("name").and_then(Value::as_str),
                body.get("pin").and_then(Value::as_bool).unwrap_or(false),
                body.get("create").and_then(Value::as_bool).unwrap_or(false),
            );
            state::save(&st).map_err(|error| (500, error))?;
            Ok(json!({ "summary": summary }))
        }
        "/api/code" => {
            let appid = body_u64(body, "appid")?;
            let depot_id = body_u64(body, "depot_id")?;
            let gid = body.get("gid").and_then(Value::as_str).unwrap_or("").to_string();
            if gid.is_empty() || !gid.chars().all(|ch| ch.is_ascii_digit()) {
                return Err((400, "gid must be decimal".into()));
            }
            let st = state::load().map_err(|error| (500, error))?;
            if let Some(cached) = st.codes.get(&gid) {
                return Ok(json!({ "gid": gid, "depot_id": depot_id, "code": cached, "provider": "cache" }));
            }
            let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
            let result = crate::providers::fetch_code(appid, depot_id, &gid, &order).map_err(|error| (502, error))?;
            if let Some(code) = result.get("code").and_then(Value::as_u64) {
                let mut st = state::load().map_err(|error| (500, error))?;
                st.codes.insert(gid, code);
                let _ = state::save(&st);
            }
            Ok(result)
        }
        _ => Err((404, format!("unknown endpoint POST {path}"))),
    }
}

fn delete_route(path: &str, query: &[(String, String)]) -> Result<Value, (u16, String)> {
    let mut st = state::load().map_err(|error| (500, error))?;
    match path {
        "/api/key" => {
            let depot_id = need_u64(query, "depot_id")?;
            state::set_key(&mut st, depot_id, None);
            state::save(&st).map_err(|error| (500, error))?;
            Ok(json!({ "depot_id": depot_id, "key": null }))
        }
        "/api/plan" => {
            let appid = need_u64(query, "appid")?;
            st.apps.remove(&appid.to_string());
            state::save(&st).map_err(|error| (500, error))?;
            vfsys::remove_script(appid);
            Ok(json!({ "removed": appid }))
        }
        "/api/script" => {
            let appid = need_u64(query, "appid")?;
            Ok(json!({ "removed": vfsys::remove_script(appid) }))
        }
        "/api/provider" => Ok(json!({ "removed": vfsys::remove_provider(), "state": vfsys::provider_state() })),
        _ => Err((404, format!("unknown endpoint DELETE {path}"))),
    }
}

/// Resolves a request path against the web root and refuses anything that ends up
/// outside it: `..`, an absolute path, or a symlink planted in the tree.
static TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn token() -> &'static str {
    TOKEN.get().map(String::as_str).unwrap_or("")
}

/// Writes need the token from `/api/session`; a browser that is not this app
/// cannot present it, and a foreign Origin is refused outright.
pub fn authorize(method: &str, origin: Option<&str>, supplied: Option<&str>, expected: &str, port: u16) -> Result<(), (u16, String)> {
    if method == "GET" || method == "OPTIONS" {
        return Ok(());
    }
    if let Some(origin) = origin {
        let allowed = [format!("http://127.0.0.1:{port}"), format!("http://localhost:{port}")];
        if !allowed.iter().any(|value| value == origin) {
            return Err((403, "origin not allowed".into()));
        }
    }
    if expected.is_empty() {
        return Err((500, "the server has no token".into()));
    }
    match supplied {
        Some(value) if value == expected => Ok(()),
        Some(_) => Err((403, "wrong token".into())),
        None => Err((403, "missing token".into())),
    }
}

pub fn static_file(relative: &str) -> Option<PathBuf> {
    let root = paths::web_dir().canonicalize().ok()?;
    let cleaned = relative.trim_start_matches('/');
    if cleaned.contains('\0') {
        return None;
    }
    let candidate = if cleaned.is_empty() { root.join("index.html") } else { root.join(cleaned) };
    let resolved = candidate.canonicalize().ok()?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return None;
    }
    Some(resolved)
}

fn handle(mut stream: TcpStream) {
    let Ok(clone) = stream.try_clone() else { return };
    let mut reader = BufReader::new(clone);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();

    let mut length = 0usize;
    let mut origin: Option<String> = None;
    let mut supplied: Option<String> = None;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {
                let lower = header.to_lowercase();
                if let Some(value) = lower.strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                if let Some(value) = lower.strip_prefix("origin:") {
                    origin = Some(value.trim().to_string());
                }
                if let Some(value) = lower.strip_prefix("x-exxa-token:") {
                    supplied = Some(value.trim().to_string());
                }
            }
            Err(_) => break,
        }
    }
    let mut body = vec![0u8; length.min(1 << 20)];
    if length > 0 && reader.read_exact(&mut body).is_err() {
        respond_json(&mut stream, 400, &json!({ "error": "cannot read body" }));
        return;
    }
    let body_json: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => {
                respond_json(&mut stream, 400, &json!({ "error": "body must be JSON" }));
                return;
            }
        }
    };

    if let Err((status, message)) = authorize(&method, origin.as_deref(), supplied.as_deref(), token(), PORT.get().copied().unwrap_or(8765)) {
        respond_json(&mut stream, status, &json!({ "error": message }));
        return;
    }

    let (raw_path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query_pairs(query)),
        None => (target.clone(), Vec::new()),
    };

    let result = match method.as_str() {
        "GET" => {
            if raw_path.starts_with("/api/") {
                get_route(&raw_path, &query)
            } else {
                match static_file(&raw_path) {
                    Some(file) => match std::fs::read(&file) {
                        Ok(body) => {
                            respond(&mut stream, 200, mime_for(&file), &body);
                            return;
                        }
                        Err(error) => Err((500, error.to_string())),
                    },
                    None => Err((404, "not found".into())),
                }
            }
        }
        "POST" => post_route(&raw_path, &body_json, &query),
        "DELETE" => delete_route(&raw_path, &query),
        "OPTIONS" => {
            respond(&mut stream, 204, "text/plain", b"");
            return;
        }
        _ => Err((405, "method not allowed".into())),
    };

    match result {
        Ok(value) => respond_json(&mut stream, 200, &value),
        Err((status, message)) => respond_json(&mut stream, status, &json!({ "error": message })),
    }
}

pub fn serve(config: Config) -> Result<(), String> {
    let _ = PORT.set(config.port);
    let _ = TOKEN.set(crate::state::ensure_token()?);
    let listener = TcpListener::bind(("127.0.0.1", config.port))
        .map_err(|error| format!("cannot bind 127.0.0.1:{}: {error}", config.port))?;
    eprintln!("exxa {} on http://127.0.0.1:{}/", env!("CARGO_PKG_VERSION"), config.port);
    std::thread::spawn(crate::steamui::watch_loop);
    let listener = Arc::new(listener);
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                std::thread::spawn(move || handle(stream));
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_query_values() {
        assert_eq!(decode("portal%202"), "portal 2");
        assert_eq!(decode("dota+2"), "dota 2");
        assert_eq!(decode("tom%27s%20game"), "tom's game");
        assert_eq!(decode("plain"), "plain");
        assert_eq!(decode("bad%zz"), "bad%zz");
    }
}
