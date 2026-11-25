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
/// A port no common dev server picks: 8765 is a popular default, and a foreign
/// service there made every ExxaTools start think its server was already up.
pub const DEFAULT_PORT: u16 = 47865;

pub fn port() -> u16 {
    *PORT.get().unwrap_or(&DEFAULT_PORT)
}

/// Our `/api/session` answers `{token: <64 hex>, version}`; anything else on the
/// port is another program.
pub fn looks_like_session(value: &Value) -> bool {
    let token = value.get("token").and_then(Value::as_str).unwrap_or("");
    token.len() == 64
        && token.chars().all(|ch| ch.is_ascii_hexdigit())
        && value.get("version").and_then(Value::as_str).is_some()
}

/// True only when ExxaTools itself answers on the port, not merely something.
pub fn is_ours(port: u16) -> bool {
    crate::http::get_json(&format!("http://127.0.0.1:{port}/api/session"))
        .map(|value| looks_like_session(&value))
        .unwrap_or(false)
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
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
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
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
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
                match hex.and_then(|text| u8::from_str_radix(text, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    None => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
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
    query
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

fn need_u64(query: &[(String, String)], key: &str) -> Result<u64, (u16, String)> {
    get(query, key)
        .and_then(|value| value.parse().ok())
        .ok_or((400, format!("{key} must be an integer")))
}

fn body_u64(body: &Value, key: &str) -> Result<u64, (u16, String)> {
    body.get(key)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
        .ok_or((400, format!("{key} must be an integer")))
}

/// Public settings are an allowlist, never a serialization of stored settings.
/// Provider objects may contain URLs, headers and arbitrarily nested secrets.
fn public_source(spec: &Value) -> Value {
    let mut out = Map::new();
    if let Some(label) = spec.get("label").and_then(Value::as_str) {
        out.insert("label".into(), json!(label));
    }
    if let Some(enabled) = spec.get("enabled").and_then(Value::as_bool) {
        out.insert("enabled".into(), json!(enabled));
    }
    if let Some(providers) = spec.get("providers").and_then(Value::as_array) {
        let names: Vec<&str> = providers
            .iter()
            .filter_map(|entry| {
                entry
                    .as_str()
                    .or_else(|| entry.get("name").and_then(Value::as_str))
            })
            .filter(|name| public_provider_name(name))
            .collect();
        out.insert("providers".into(), json!(names));
    }
    Value::Object(out)
}

fn public_provider_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn public_settings(settings: &state::Settings) -> Value {
    let sources: Map<String, Value> = settings
        .sources
        .iter()
        .map(|(name, spec)| (name.clone(), public_source(spec)))
        .collect();
    let plan_sources: Map<String, Value> = settings
        .plan_sources
        .iter()
        .map(|(name, spec)| (name.clone(), public_source(spec)))
        .collect();
    let providers: Vec<&str> = settings
        .manifest_providers
        .iter()
        .map(String::as_str)
        .filter(|name| public_provider_name(name))
        .collect();
    json!({
        "country": settings.country,
        "default_source": settings.default_source,
        "default_os": settings.default_os,
        "theme": settings.theme,
        "auto_update_plans": settings.auto_update_plans,
        "manifest_providers": providers,
        "sources": sources,
        "plan_sources": plan_sources,
        "has_hubcap_key": !settings.hubcap_key.trim().is_empty(),
        "has_ryuu_key": !settings.ryuu_key.trim().is_empty(),
    })
}

/// Source maps are patches: omitted IDs/fields retain their private values.
/// An explicitly empty map still clears the map; null/non-map values retain
/// the existing settings deserialization semantics.
fn merge_source_map(current: &mut Value, posted: &Value) {
    let Some(patch) = posted.as_object() else {
        *current = posted.clone();
        return;
    };
    if patch.is_empty() {
        *current = posted.clone();
        return;
    }
    if !current.is_object() {
        *current = json!({});
    }
    let sources = current.as_object_mut().expect("source map");
    for (id, value) in patch {
        let source = sources.entry(id.clone()).or_insert_with(|| json!({}));
        let Some(fields) = value.as_object() else {
            *source = value.clone();
            continue;
        };
        for (field, value) in fields {
            if field.starts_with("has_") {
                continue;
            }
            // The public provider list is lossy: objects become names and
            // private URL entries disappear. Posting that view is a no-op.
            if field == "providers" && public_source(source).get(field) == Some(value) {
                continue;
            }
            let value = if field == "providers" {
                merge_source_providers(source.get(field), value)
            } else {
                value.clone()
            };
            if !source.is_object() {
                *source = json!({});
            }
            source[field] = value;
        }
    }
}

/// A deliberately changed provider list replaces membership/order, but a
/// retained provider name must not replace its private object with a string.
fn merge_source_providers(current: Option<&Value>, posted: &Value) -> Value {
    let (Some(existing), Some(providers)) = (current.and_then(Value::as_array), posted.as_array())
    else {
        return posted.clone();
    };
    Value::Array(
        providers
            .iter()
            .map(|provider| {
                let name = provider
                    .as_str()
                    .or_else(|| provider.get("name").and_then(Value::as_str));
                let previous = name.and_then(|name| {
                    existing
                        .iter()
                        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
                });
                match (previous, provider.as_object()) {
                    (Some(previous), Some(fields)) => {
                        let mut merged = previous.clone();
                        for (field, value) in fields {
                            if !field.starts_with("has_") {
                                merged[field] = value.clone();
                            }
                        }
                        merged
                    }
                    (Some(previous), None) => previous.clone(),
                    _ => provider.clone(),
                }
            })
            .collect(),
    )
}

fn public_source_catalog(settings: &state::Settings) -> Vec<Value> {
    crate::sources::catalog(settings)
        .iter()
        .map(|source| {
            let mut view = public_source(source);
            if let Some(name) = source.get("name").and_then(Value::as_str) {
                view["name"] = json!(name);
            }
            view
        })
        .collect()
}

/// Keep HTTP validation errors distinct from transaction/persistence failures.
fn update_route<T>(
    change: impl FnOnce(&mut state::State) -> Result<T, (u16, String)>,
) -> Result<T, (u16, String)> {
    let mut route_error = None;
    state::update(|st| {
        change(st).map_err(|error| {
            route_error = Some(error);
            "request rejected".to_string()
        })
    })
    .map_err(|error| route_error.unwrap_or((500, error)))
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
                "code": manifest.as_ref().and_then(|value| value.get("gid")).and_then(Value::as_str)
                    .and_then(|gid| st.codes.get(gid)).cloned(),
            }));
        }
    }
    let has_dlc = catalog
        .get("dlc")
        .and_then(Value::as_array)
        .map(|list| !list.is_empty())
        .unwrap_or(false);
    let dlc_names = if has_dlc {
        steam::dlc_names(appid, &st.settings.country)
    } else {
        Vec::new()
    };
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
        "sources": public_source_catalog(&st.settings),
        "launch_options": steamctl::read_launch_options(appid),
        "catalog": catalog,
    }))
}

fn record_from_body(body: &Value) -> Result<Value, (u16, String)> {
    let appid = body_u64(body, "appid")?;
    let depots_raw = body
        .get("depots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut depots = Vec::new();
    for item in depots_raw {
        let depot_id = item
            .get("depot_id")
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
            })
            .ok_or((400, "each depot needs a depot_id".to_string()))?;
        let gid = item
            .get("gid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
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
        .map(|list| {
            list.iter()
                .filter_map(|value| {
                    value
                        .as_u64()
                        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
                })
                .collect()
        })
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
        "/api/plugins" => Ok(crate::plugins::list_json()),
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
            "settings": public_settings(&st.settings),
            "state_path": state::state_path().display().to_string(),
            "keys": st.depot_keys.len(),
        })),
        "/api/engine" => Ok(engine::detect()),
        "/api/engine/releases" => {
            Ok(json!({ "releases": engine::releases(5).map_err(|error| (502, error))? }))
        }
        "/api/log" => {
            let lines = get(query, "lines")
                .and_then(|value| value.parse().ok())
                .unwrap_or(200);
            Ok(engine::log(lines))
        }
        "/api/discover" => {
            let limit = get(query, "limit")
                .and_then(|value| value.parse().ok())
                .unwrap_or(18);
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
            Ok(
                json!({ "depot_id": depot_id, "has_key": st.depot_keys.contains_key(&depot_id.to_string()) }),
            )
        }
        "/api/plan" => {
            let appid = need_u64(query, "appid")?;
            st.apps
                .get(&appid.to_string())
                .cloned()
                .map(Ok)
                .unwrap_or(Err((404, "no saved plan".into())))
        }
        "/api/script" => {
            let appid = need_u64(query, "appid")?;
            let record = st
                .apps
                .get(&appid.to_string())
                .ok_or((404, "no saved plan".to_string()))?;
            Ok(json!({
                "appid": appid,
                "script": render_record(record, &st),
                "installed": vfsys::script_path(appid).exists(),
            }))
        }
        "/api/settings" => Ok(public_settings(&st.settings)),
        "/api/sources" => Ok(json!({
            "sources": public_source_catalog(&st.settings),
            "default": st.settings.default_source,
        })),
        "/api/dlc" => {
            let appid = need_u64(query, "appid")?;
            Ok(json!({ "items": steam::dlc_names(appid, &st.settings.country) }))
        }
        "/api/autoselect" => {
            let appid = need_u64(query, "appid")?;
            let branch = get(query, "branch").unwrap_or("public").to_string();
            let os = get(query, "os")
                .unwrap_or(&st.settings.default_os)
                .to_string();
            let catalog = steam::catalog(appid).map_err(|error| (502, error))?;
            let mut out = Vec::new();
            if let Some(list) = catalog.get("depots").and_then(Value::as_array) {
                for depot in list {
                    let depot_id = depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0);
                    let oslist: Vec<String> = depot
                        .get("oslist")
                        .and_then(Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default();
                    let shared = depot
                        .get("shared_install")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let osarch = depot.get("osarch").and_then(Value::as_str).unwrap_or("");
                    let manifest = steam::pick_manifest(depot, &branch);
                    let (selected, reason) = if manifest.is_none() {
                        (false, format!("no manifest in {branch}"))
                    } else if !oslist.is_empty() && !oslist.iter().any(|value| value == &os) {
                        (false, format!("only for {}", oslist.join(", ")))
                    } else if osarch == "32" {
                        (false, "32-bit only".to_string())
                    } else if shared {
                        (true, "shared depot".to_string())
                    } else {
                        (
                            true,
                            if oslist.is_empty() {
                                format!("platform-neutral, {branch}")
                            } else if osarch == "64" {
                                format!("{os} 64-bit build, {branch}")
                            } else {
                                format!("{os} build, {branch}")
                            },
                        )
                    };
                    out.push(
                        json!({ "depot_id": depot_id, "selected": selected, "reason": reason }),
                    );
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
                "settings": public_settings(&st.settings),
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
            // compare performs network IO. Collect only this request's observed
            // gids outside the lock, then merge them into the latest history.
            let mut observed = st.clone();
            observed.build_history.clear();
            let mut result = crate::pins::compare(&mut observed, appid, &branch)
                .map_err(|error| (502, error))?;
            let (changed, history) = state::update(|fresh| {
                let mut changed = false;
                if let Some(depots) = observed.build_history.get(&appid.to_string()) {
                    for (depot, entries) in depots {
                        let Ok(depot_id) = depot.parse::<u64>() else {
                            continue;
                        };
                        for entry in entries.iter().rev() {
                            let Some(gid) = entry.get("gid").and_then(Value::as_str) else {
                                continue;
                            };
                            changed |= state::record_gid(
                                fresh,
                                appid,
                                depot_id,
                                gid,
                                entry.get("size").and_then(Value::as_u64),
                                &branch,
                            );
                        }
                    }
                }
                Ok((
                    changed,
                    fresh
                        .build_history
                        .get(&appid.to_string())
                        .cloned()
                        .unwrap_or_default(),
                ))
            })
            .map_err(|error| (500, error))?;
            result["changed"] = json!(changed);
            result["history"] = json!(history);
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
            let order: Vec<Value> = st
                .settings
                .manifest_providers
                .iter()
                .map(|name| json!(name))
                .collect();
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
                "hint": "",
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

fn post_route(
    path: &str,
    body: &Value,
    query: &[(String, String)],
) -> Result<Value, (u16, String)> {
    let _ = query;
    match path {
        "/api/plugins/toggle" => {
            let id = body
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let enabled = body
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            crate::plugins::set_enabled(&id, enabled).map_err(|error| (400, error))
        }
        "/api/plugins/remove" => {
            let id = body
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            crate::plugins::remove(&id).map_err(|error| (400, error))
        }
        "/api/plugins/inject" => {
            let _ = crate::steamui::inject();
            Ok(crate::plugins::list_json())
        }
        "/api/engine/install" => {
            let tag = body
                .get("tag")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            engine::install(tag).map_err(|error| (500, error))
        }
        "/api/engine/uninstall" => engine::uninstall().map_err(|error| (500, error)),
        "/api/depot" => {
            let appid = body_u64(body, "appid")?;
            let depot_id = body_u64(body, "depot_id")?;
            update_route(|st| {
                let record = st
                    .apps
                    .get_mut(&appid.to_string())
                    .ok_or((404, "no saved plan".to_string()))?;
                let list = record
                    .get_mut("depots")
                    .and_then(Value::as_array_mut)
                    .ok_or((409, "plan has no depots".to_string()))?;
                let mut found = false;
                for depot in list.iter_mut() {
                    if depot.get("depot_id").and_then(Value::as_u64) != Some(depot_id) {
                        continue;
                    }
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
                Ok(())
            })?;
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
            state::update(|st| {
                st.apps.insert(appid.to_string(), record.clone());
                Ok(())
            })
            .map_err(|error| (500, error))?;
            Ok(record)
        }
        "/api/install" => {
            let appid = body_u64(body, "appid")?;
            let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let st = state::load().map_err(|error| (500, error))?;
            let record = st
                .apps
                .get(&appid.to_string())
                .ok_or((409, "save a plan first".to_string()))?;
            let text = render_record(record, &st);
            let target =
                vfsys::write_script(appid, &text, enabled).map_err(|error| (500, error))?;
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
            Ok(
                json!({ "path": target.display().to_string(), "script": text, "missing_keys": missing }),
            )
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
            let key = state::valid_key(raw)
                .ok_or((400, "a depot key is 64 hex characters".to_string()))?;
            state::update(|st| {
                state::set_key(st, depot_id, Some(&key));
                Ok(())
            })
            .map_err(|error| (500, error))?;
            Ok(json!({ "depot_id": depot_id, "has_key": true }))
        }
        "/api/script/check" => {
            let text = body.get("text").and_then(Value::as_str);
            match text {
                Some(text) if !text.trim().is_empty() => Ok(engine::check_script(text, "inline")),
                _ => {
                    let appid = body_u64(body, "appid")?;
                    let st = state::load().map_err(|error| (500, error))?;
                    let record = st
                        .apps
                        .get(&appid.to_string())
                        .ok_or((409, "save a plan first".to_string()))?;
                    Ok(engine::check_script(
                        &render_record(record, &st),
                        &format!("{appid}.lua"),
                    ))
                }
            }
        }
        "/api/provider" => {
            let st = state::load().map_err(|error| (500, error))?;
            let order: Vec<Value> = match body.get("providers").and_then(Value::as_array) {
                Some(list) if !list.is_empty() => list.clone(),
                _ => st
                    .settings
                    .manifest_providers
                    .iter()
                    .map(|name| json!(name))
                    .collect(),
            };
            let text = lua::render_provider(&order).map_err(|error| (500, error))?;
            let check = engine::check_script(&text, lua::PROVIDER_FILE);
            if !check.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                return Err((
                    500,
                    format!(
                        "provider script failed to parse: {}",
                        check.get("output").and_then(Value::as_str).unwrap_or("")
                    ),
                ));
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
            let mode =
                appmanifest::Mode::parse(body.get("mode").and_then(Value::as_str).unwrap_or("fix"))
                    .map_err(|error| (400, error))?;
            let force = body.get("force").and_then(Value::as_bool).unwrap_or(false);
            let restart = body
                .get("restart")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if restart {
                return appmanifest::patch_with_restart(appid, &mode).map_err(|error| {
                    let status = if error.contains("a game is running") {
                        409
                    } else {
                        500
                    };
                    (status, error)
                });
            }
            appmanifest::patch(appid, &mode, !force).map_err(|error| (500, error))
        }
        "/api/fix" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or("");
            let appid = body.get("appid").and_then(Value::as_u64).unwrap_or(0);
            let st = state::load().map_err(|error| (500, error))?;
            match action {
                "manifest" => {
                    if appid == 0 {
                        return Err((400, "appid is required".into()));
                    }
                    let result = appmanifest::patch(appid, &appmanifest::Mode::Fix, true)
                        .map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": "manifest repaired", "result": result }))
                }
                "keys" => {
                    let mut written = 0;
                    for record in st.apps.values() {
                        let id = record.get("appid").and_then(Value::as_u64).unwrap_or(0);
                        if appid != 0 && id != appid {
                            continue;
                        }
                        let text = render_record(record, &st);
                        if vfsys::script_path(id).exists()
                            && vfsys::write_script(id, &text, true).is_ok()
                        {
                            written += 1;
                        }
                    }
                    let order: Vec<Value> = st
                        .settings
                        .manifest_providers
                        .iter()
                        .map(|name| json!(name))
                        .collect();
                    lua::render_provider(&order)
                        .and_then(|text| vfsys::write_provider(&text).map(|_| ()))
                        .map_err(|error| (500, error))?;
                    Ok(
                        json!({ "ok": true, "message": format!("{written} script(s) refreshed with the keys on disk") }),
                    )
                }
                "provider" => {
                    let order: Vec<Value> = st
                        .settings
                        .manifest_providers
                        .iter()
                        .map(|name| json!(name))
                        .collect();
                    let text = lua::render_provider(&order).map_err(|error| (500, error))?;
                    vfsys::write_provider(&text).map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": "provider script written" }))
                }
                "cloud" => {
                    if appid == 0 {
                        return Err((400, "appid is required".into()));
                    }
                    let result =
                        steamctl::clear_cloud_state(appid).map_err(|error| (409, error))?;
                    Ok(
                        json!({ "ok": true, "message": "cloud state reset, Steam will sync this game from scratch", "result": result }),
                    )
                }
                "launch" => {
                    if appid == 0 {
                        return Err((400, "appid is required".into()));
                    }
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
                    if appid == 0 {
                        return Err((400, "appid is required".into()));
                    }
                    vfsys::remove_script(appid);
                    state::update(|fresh| {
                        fresh.apps.remove(&appid.to_string());
                        Ok(())
                    })
                    .map_err(|error| (500, error))?;
                    Ok(json!({ "ok": true, "message": "removed from the library" }))
                }
                other => Err((400, format!("unknown fix {other}"))),
            }
        }
        "/api/steamui" => {
            let action = body
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("status");
            match action {
                "inject" => crate::steamui::inject().map_err(|error| (500, error)),
                "remove" => Ok(crate::steamui::remove()),
                "enable" => crate::steamui::enable_debug().map_err(|error| (500, error)),
                other => Err((
                    400,
                    format!("action must be inject, remove, or enable, not {other}"),
                )),
            }
        }
        "/api/app/launch" => Ok(match crate::desktop::open_window("") {
            Ok(()) => json!({ "opened": true, "where": "native" }),
            Err(error) => json!({ "opened": false, "error": error }),
        }),
        "/api/desktop" => {
            let action = body
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("install");
            match action {
                "install" => crate::desktop::install_desktop_entry().map_err(|error| (500, error)),
                "uninstall" => Ok(crate::desktop::uninstall_desktop_entry()),
                other => Err((
                    400,
                    format!("action must be install or uninstall, not {other}"),
                )),
            }
        }
        "/api/launchoptions" => {
            let appid = body_u64(body, "appid")?;
            let options = body
                .get("options")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            steamctl::set_launch_options(appid, &options).map_err(|error| (400, error))
        }
        "/api/codes" => {
            let appid = body_u64(body, "appid")?;
            let depots = body
                .get("depots")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let st = state::load().map_err(|error| (500, error))?;
            let order: Vec<Value> = st
                .settings
                .manifest_providers
                .iter()
                .map(|name| json!(name))
                .collect();
            let mut out = Vec::new();
            let mut errors = Vec::new();
            for depot in depots {
                let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else {
                    continue;
                };
                let Some(gid) = depot
                    .get("gid")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                match crate::providers::fetch_code(appid, depot_id, gid, &order) {
                    Ok(result) => out.push(result),
                    Err(error) => {
                        errors.push(json!({ "depot_id": depot_id, "gid": gid, "error": error }))
                    }
                }
            }
            Ok(json!({ "codes": out, "errors": errors }))
        }
        "/api/pin" => {
            let appid = body_u64(body, "appid")?;
            let depot_id = body_u64(body, "depot_id")?;
            let gid = body
                .get("gid")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty());
            update_route(|st| {
                crate::pins::set_pin(st, appid, depot_id, gid).map_err(|error| (400, error))
            })
        }
        "/api/watch" => {
            let apply = body.get("apply").and_then(Value::as_bool).unwrap_or(false);
            Ok(crate::watch::sweep(apply))
        }
        "/api/steam/restart" => steamctl::restart(&[]).map_err(|error| (500, error)),
        "/api/steam/shutdown" => {
            let stopped = steamctl::shutdown(std::time::Duration::from_secs(45))
                .map_err(|error| (500, error))?;
            Ok(json!({ "stopped": stopped }))
        }
        "/api/hubcap" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or("save");
            let st = state::load().map_err(|error| (500, error))?;
            match action {
                "save" => {
                    let raw = body
                        .get("key")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if raw.is_empty() {
                        state::update(|fresh| {
                            fresh.settings.hubcap_key.clear();
                            Ok(())
                        })
                        .map_err(|error| (500, error))?;
                        return Ok(json!({ "configured": false }));
                    }
                    let valid = hubcap::valid_key(&raw).ok_or((
                        400,
                        "a Hubcap key starts with smm and has no spaces".to_string(),
                    ))?;
                    let stats =
                        hubcap::validate(&valid).map_err(|error| (400, error.to_string()))?;
                    state::update(|fresh| {
                        fresh.settings.hubcap_key = valid;
                        Ok(())
                    })
                    .map_err(|error| (500, error))?;
                    Ok(json!({ "configured": true, "stats": stats }))
                }
                "search" => {
                    let term = body
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if term.is_empty() {
                        return Err((400, "query is required".into()));
                    }
                    if st.settings.hubcap_key.is_empty() {
                        let items = crate::steam::search(&term, &st.settings.country)
                            .map_err(|error| (502, error))?;
                        return Ok(json!({ "items": items, "source": "steam" }));
                    }
                    let items = hubcap::search(&st.settings.hubcap_key, &term, 25)
                        .map_err(|error| (502, error.to_string()))?;
                    Ok(json!({ "items": items }))
                }
                "fetch" => {
                    if st.settings.hubcap_key.is_empty() {
                        return Err((409, "set a Hubcap API key first".into()));
                    }
                    let appid = body_u64(body, "appid")?;
                    let fetched = hubcap::fetch(&st.settings.hubcap_key, appid)
                        .map_err(|error| (502, error.to_string()))?;
                    let parsed = hubcap::parse_lua(&fetched.lua, Some(appid));
                    let stamp = vfsys::stamp();
                    let summary = state::update(|fresh| {
                        Ok(hubcap::apply_to_state(
                            fresh,
                            &parsed,
                            &stamp,
                            body.get("name").and_then(Value::as_str),
                            body.get("pin").and_then(Value::as_bool).unwrap_or(false),
                            body.get("create").and_then(Value::as_bool).unwrap_or(false),
                        ))
                    })
                    .map_err(|error| (500, error))?;
                    Ok(
                        json!({ "summary": summary, "manifests": fetched.manifests, "source": fetched.source }),
                    )
                }
                other => Err((
                    400,
                    format!("action must be save, search, or fetch, not {other}"),
                )),
            }
        }
        "/api/settings" => {
            let result = update_route(|st| {
                if let Some(object) = body.as_object() {
                    let current =
                        serde_json::to_value(&st.settings).map_err(|error| (500, error.to_string()))?;
                    let mut merged: Map<String, Value> =
                        current.as_object().cloned().unwrap_or_default();
                    for (key, value) in object {
                        if key.starts_with("has_") {
                            continue;
                        }
                        if key == "sources" || key == "plan_sources" {
                            merge_source_map(
                                merged.entry(key.clone()).or_insert_with(|| json!({})),
                                value,
                            );
                        } else {
                            merged.insert(key.clone(), value.clone());
                        }
                    }
                    st.settings = serde_json::from_value(Value::Object(merged))
                        .map_err(|_| (400, "invalid settings".to_string()))?;
                }
                Ok(public_settings(&st.settings))
            })?;
            // Keep the Vapor Forge provider chain in step with the saved order;
            // otherwise a settings change never reaches the running client.
            if let Ok(st) = state::load() {
                let order = crate::lua::provider_order(&st);
                if let Err(error) = crate::lua::republish_provider(&order) {
                    eprintln!("exxa: manifest provider chain not written: {error}");
                }
            }
            Ok(result)
        }
        "/api/open" => {
            let target = body.get("target").and_then(Value::as_str).unwrap_or("");
            if target.is_empty() {
                return Err((400, "target is required".into()));
            }
            crate::desktop::spawn_reaped(std::process::Command::new("xdg-open").arg(target))
                .map_err(|error| (500, format!("cannot open: {error}")))?;
            Ok(json!({ "opened": target }))
        }
        "/api/import" => {
            let text = body.get("text").and_then(Value::as_str).unwrap_or("");
            if text.trim().is_empty() {
                return Err((400, "text is required".into()));
            }
            let parsed = hubcap::parse_lua(text, body.get("appid").and_then(Value::as_u64));
            if parsed.appid.is_none() {
                return Err((400, "that lua has no app id".into()));
            }
            let stamp = vfsys::stamp();
            let summary = state::update(|st| {
                Ok(hubcap::apply_to_state(
                    st,
                    &parsed,
                    &stamp,
                    body.get("name").and_then(Value::as_str),
                    body.get("pin").and_then(Value::as_bool).unwrap_or(false),
                    body.get("create").and_then(Value::as_bool).unwrap_or(false),
                ))
            })
            .map_err(|error| (500, error))?;
            Ok(json!({ "summary": summary }))
        }
        "/api/code" => {
            let appid = body_u64(body, "appid")?;
            let depot_id = body_u64(body, "depot_id")?;
            let gid = body
                .get("gid")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if gid.is_empty() || !gid.chars().all(|ch| ch.is_ascii_digit()) {
                return Err((400, "gid must be decimal".into()));
            }
            let st = state::load().map_err(|error| (500, error))?;
            if let Some(cached) = st.codes.get(&gid) {
                return Ok(
                    json!({ "gid": gid, "depot_id": depot_id, "code": cached, "provider": "cache" }),
                );
            }
            let order: Vec<Value> = st
                .settings
                .manifest_providers
                .iter()
                .map(|name| json!(name))
                .collect();
            let result = crate::providers::fetch_code(appid, depot_id, &gid, &order)
                .map_err(|error| (502, error))?;
            if let Some(code) = result.get("code").and_then(Value::as_u64) {
                state::update(|st| {
                    st.codes.entry(gid).or_insert(code);
                    Ok(())
                })
                .map_err(|error| (500, error))?;
            }
            Ok(result)
        }
        _ => Err((404, format!("unknown endpoint POST {path}"))),
    }
}

fn delete_route(path: &str, query: &[(String, String)]) -> Result<Value, (u16, String)> {
    match path {
        "/api/key" => {
            let depot_id = need_u64(query, "depot_id")?;
            state::update(|st| {
                state::set_key(st, depot_id, None);
                Ok(())
            })
            .map_err(|error| (500, error))?;
            Ok(json!({ "depot_id": depot_id, "has_key": false }))
        }
        "/api/plan" => {
            let appid = need_u64(query, "appid")?;
            state::update(|st| {
                st.apps.remove(&appid.to_string());
                Ok(())
            })
            .map_err(|error| (500, error))?;
            vfsys::remove_script(appid);
            Ok(json!({ "removed": appid }))
        }
        "/api/script" => {
            let appid = need_u64(query, "appid")?;
            Ok(json!({ "removed": vfsys::remove_script(appid) }))
        }
        "/api/provider" => {
            Ok(json!({ "removed": vfsys::remove_provider(), "state": vfsys::provider_state() }))
        }
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
pub fn authorize(
    method: &str,
    origin: Option<&str>,
    supplied: Option<&str>,
    expected: &str,
    port: u16,
) -> Result<(), (u16, String)> {
    if method == "GET" || method == "OPTIONS" {
        return Ok(());
    }
    if let Some(origin) = origin {
        let allowed = [
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
        ];
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
    let candidate = if cleaned.is_empty() {
        root.join("index.html")
    } else {
        root.join(cleaned)
    };
    let resolved = candidate.canonicalize().ok()?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return None;
    }
    Some(resolved)
}

fn handle(mut stream: TcpStream) {
    let Ok(clone) = stream.try_clone() else {
        return;
    };
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
    let mut content_type: Option<String> = None;
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
                if let Some(value) = lower.strip_prefix("content-type:") {
                    content_type = Some(value.trim().to_string());
                }
            }
            Err(_) => break,
        }
    }
    const MAX_BODY: usize = 1 << 20;
    if length > MAX_BODY {
        respond_json(
            &mut stream,
            413,
            &json!({ "error": format!("body is larger than {MAX_BODY} bytes") }),
        );
        return;
    }
    let mut body = vec![0u8; length];
    if length > 0 && reader.read_exact(&mut body).is_err() {
        respond_json(&mut stream, 400, &json!({ "error": "cannot read body" }));
        return;
    }
    let body_json: Value = if body.is_empty() {
        json!({})
    } else {
        if let Some(kind) = content_type {
            if !kind.starts_with("application/json") {
                respond_json(
                    &mut stream,
                    415,
                    &json!({ "error": "body must be application/json" }),
                );
                return;
            }
        }
        match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => {
                respond_json(&mut stream, 400, &json!({ "error": "body must be JSON" }));
                return;
            }
        }
    };

    if let Err((status, message)) = authorize(
        &method,
        origin.as_deref(),
        supplied.as_deref(),
        token(),
        PORT.get().copied().unwrap_or(DEFAULT_PORT),
    ) {
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
    // The provider chain is a generated file; refresh it at startup so an
    // edited settings list (or an older file) never keeps a dead order alive.
    if let Ok(st) = crate::state::load() {
        let order = crate::lua::provider_order(&st);
        match crate::lua::republish_provider(&order) {
            Ok(path) => eprintln!(
                "exxa: manifest provider chain: {}",
                path.display()
            ),
            Err(error) => eprintln!("exxa: manifest provider chain not written: {error}"),
        }
    }
    if let Ok(value) =
        crate::http::get_json(&format!("http://127.0.0.1:{}/api/session", config.port))
    {
        if value.get("token").and_then(Value::as_str).is_some() {
            eprintln!(
                "exxa is already serving on http://127.0.0.1:{}/",
                config.port
            );
            return Ok(());
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", config.port))
        .map_err(|error| format!("cannot bind 127.0.0.1:{}: {error}", config.port))?;
    eprintln!(
        "exxa {} on http://127.0.0.1:{}/",
        env!("CARGO_PKG_VERSION"),
        config.port
    );
    std::thread::spawn(crate::steamui::watch_loop);
    crate::steamui::start_pump();
    crate::live::start_server_reexec(config.port);
    crate::live::start_steam_follower();
    crate::live::start_builder();
    crate::plans::start_auto_update();
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

#[cfg(test)]
mod identity_tests {
    use super::looks_like_session;
    use serde_json::json;

    #[test]
    fn our_session_is_recognised() {
        let token = "a".repeat(64);
        assert!(looks_like_session(
            &json!({"token": token, "version": "0.3.0"})
        ));
    }

    #[test]
    fn a_foreign_server_on_the_port_is_not_ours() {
        assert!(!looks_like_session(&json!({"detail": "Not Found"})));
        assert!(!looks_like_session(
            &json!({"token": "short", "version": "1"})
        ));
        assert!(!looks_like_session(
            &json!({"token": "g".repeat(64), "version": "1"})
        ));
    }
}
