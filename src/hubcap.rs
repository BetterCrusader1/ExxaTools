//! Hubcap Manifest: depot keys and pinned manifests, one app at a time.
//!
//! Hubcap hands out a free API key (`smm_...`) and returns a zip per app with a
//! `.lua` containing the depot list, the depot keys, and the manifest ids of a
//! snapshot build. Keys are the half Steam never gives you.

use std::io::{Cursor, Read};

use serde_json::{json, Value};

use crate::http;
use crate::state::State;

const BASE: &str = "https://hubcapmanifest.com/api/v1";
const AGENT: &str = "ExxaTools/1.0 (Linux)";

#[derive(Debug)]
pub struct HubcapError(pub String);

impl std::fmt::Display for HubcapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn request(path: &str, key: &str, params: &[(String, String)]) -> Result<Vec<u8>, HubcapError> {
    if key.trim().is_empty() {
        return Err(HubcapError("no Hubcap API key set".into()));
    }
    let mut url = format!("{BASE}{path}");
    if !params.is_empty() {
        let encoded: Vec<String> = params
            .iter()
            .map(|(name, value)| format!("{}={}", http::quote(name), http::quote(value)))
            .collect();
        url.push('?');
        url.push_str(&encoded.join("&"));
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .user_agent(AGENT)
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = match agent
        .get(&url)
        .header("Authorization", &format!("Bearer {}", key.trim()))
        .header("Accept", "application/json, application/zip, text/plain, */*")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(code)) => {
            return Err(HubcapError(match code {
                401 => "Hubcap rejected the API key".to_string(),
                429 => "Hubcap daily limit reached".to_string(),
                other => format!("Hubcap returned HTTP {other}"),
            }));
        }
        Err(error) => return Err(HubcapError(format!("cannot reach Hubcap: {error}"))),
    };
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|error| HubcapError(format!("cannot read Hubcap response: {error}")))?;
    Ok(body)
}

fn request_json(path: &str, key: &str, params: &[(String, String)]) -> Result<Value, HubcapError> {
    let raw = request(path, key, params)?;
    serde_json::from_slice(&raw).map_err(|_| HubcapError("Hubcap returned a non-JSON payload".into()))
}

pub fn valid_key(value: &str) -> Option<String> {
    let text = value.trim();
    if !text.starts_with("smm") || text.len() < 16 || text.chars().any(char::is_whitespace) {
        return None;
    }
    Some(text.to_string())
}

pub fn validate(key: &str) -> Result<Value, HubcapError> {
    let payload = request_json("/user/stats", key, &[])?;
    Ok(json!({
        "ok": payload.get("can_make_requests").and_then(Value::as_bool).unwrap_or(false),
        "usage": payload.get("daily_usage"),
        "limit": payload.get("daily_limit"),
        "can_request": payload.get("can_make_requests"),
    }))
}

pub fn search(key: &str, term: &str, limit: usize) -> Result<Vec<Value>, HubcapError> {
    let params = vec![("q".to_string(), term.to_string()), ("limit".to_string(), limit.clamp(1, 100).to_string())];
    let payload = request_json("/search", key, &params)?;
    let items = payload.get("results").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for item in items {
        let appid = item.get("game_id").or_else(|| item.get("appid")).and_then(|value| {
            value.as_u64().or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        });
        let Some(appid) = appid else { continue };
        out.push(json!({
            "appid": appid,
            "name": item.get("game_name").or_else(|| item.get("name")).cloned().unwrap_or_else(|| json!("")),
            "updated": item.get("uploaded_date").or_else(|| item.get("last_updated")).cloned().unwrap_or(Value::Null),
            "size": item.get("manifest_size").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(out)
}

pub struct Fetched {
    pub lua: String,
    pub manifests: Vec<String>,
    pub source: String,
}

pub fn fetch(key: &str, appid: u64) -> Result<Fetched, HubcapError> {
    let raw = request(&format!("/manifest/{appid}"), key, &[])?;
    if raw.starts_with(b"PK") {
        return from_zip(&raw, appid);
    }
    let text = String::from_utf8_lossy(&raw).into_owned();
    let stripped = text.trim_start();
    if stripped.starts_with('<') {
        return Err(HubcapError(format!("app {appid} is not in the Hubcap database")));
    }
    if stripped.starts_with('{') {
        return Err(HubcapError(format!("Hubcap answered with JSON for {appid}")));
    }
    Ok(Fetched { lua: text, manifests: Vec::new(), source: "lua".into() })
}

fn from_zip(raw: &[u8], appid: u64) -> Result<Fetched, HubcapError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(raw)).map_err(|_| HubcapError("Hubcap archive is not a valid zip".into()))?;
    let mut lua: Option<String> = None;
    let mut manifests = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| HubcapError(error.to_string()))?;
        let name = entry.name().to_string();
        let lower = name.to_lowercase();
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer).map_err(|error| HubcapError(error.to_string()))?;
        if lower.ends_with(".lua") && lua.is_none() {
            lua = Some(String::from_utf8_lossy(&buffer).into_owned());
        } else if lower.ends_with(".manifest") {
            manifests.push(name.rsplit('/').next().unwrap_or(&name).to_string());
        }
    }
    let lua = lua.ok_or_else(|| HubcapError(format!("Hubcap archive for {appid} has no .lua inside")))?;
    Ok(Fetched { lua, manifests, source: "zip".into() })
}

pub struct Parsed {
    pub appid: Option<u64>,
    pub dlc: Vec<u64>,
    pub depots: Vec<Value>,
}

fn call_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in line.chars() {
        match quote {
            Some(active) if ch == active => {
                out.push(current.trim().to_string());
                current.clear();
                quote = None;
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == ',' => {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                current.clear();
            }
            None if ch.is_whitespace() => {
                if !current.trim().is_empty() && !current.ends_with('(') {
                    out.push(current.trim().to_string());
                    current.clear();
                }
            }
            None => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out.retain(|value| !value.is_empty() && !value.starts_with('('));
    out
}

fn digits(value: &str) -> Option<u64> {
    let text = value.trim().trim_matches(')');
    if !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}

pub fn parse_lua(text: &str, appid: Option<u64>) -> Parsed {
    let mut apps: Vec<u64> = Vec::new();
    let mut depots: std::collections::BTreeMap<u64, Value> = std::collections::BTreeMap::new();
    let mut pins: std::collections::BTreeMap<u64, (String, Option<u64>)> = std::collections::BTreeMap::new();
    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with("--") {
            continue;
        }
        let lower = stripped.to_lowercase();
        let args = if lower.starts_with("addappid(") {
            call_args(stripped.trim_start_matches("addappid(").trim_end_matches(')'))
        } else if lower.starts_with("setmanifestid(") {
            call_args(stripped.trim_start_matches("setmanifestid(").trim_end_matches(')'))
        } else {
            continue;
        };
        if lower.starts_with("addappid(") {
            let Some(target) = args.first().and_then(|value| digits(value)) else { continue };
            let key = args.get(2).map(|value| value.trim().to_lowercase()).unwrap_or_default();
            let is_key = key.len() == 64 && key.chars().all(|ch| ch.is_ascii_hexdigit());
            if is_key {
                depots.entry(target).or_insert_with(|| json!({ "depot_id": target, "key": key, "gid": null, "size": null }));
            } else {
                apps.push(target);
            }
            continue;
        }
        let (Some(depot_id), Some(gid)) = (args.first().and_then(|value| digits(value)), args.get(1).and_then(|value| digits(value))) else {
            continue;
        };
        let size = args.get(2).and_then(|value| digits(value));
        pins.insert(depot_id, (gid.to_string(), size));
    }
    for (depot_id, (gid, size)) in pins {
        let entry = depots.entry(depot_id).or_insert_with(|| json!({ "depot_id": depot_id, "key": "", "gid": null, "size": null }));
        entry["gid"] = json!(gid);
        if size.is_some() {
            entry["size"] = json!(size);
        }
    }
    let root = appid.or_else(|| apps.first().copied());
    let dlc: Vec<u64> = apps.iter().copied().filter(|value| Some(*value) != root).collect();
    Parsed {
        appid: root,
        dlc,
        depots: depots.into_values().collect(),
    }
}

/// Keys always land; pins only on request, because a Hubcap snapshot is one
/// build and not necessarily the branch the user selected.
pub fn apply_to_state(state: &mut State, parsed: &Parsed, stamp: &str, name: Option<&str>, pin: bool, create: bool) -> Value {
    let Some(appid) = parsed.appid else {
        return json!({ "error": "that lua has no app id" });
    };
    let mut imported = 0;
    for depot in &parsed.depots {
        let Some(depot_id) = depot.get("depot_id").and_then(Value::as_u64) else { continue };
        let key = depot.get("key").and_then(Value::as_str).unwrap_or("");
        if key.len() == 64 {
            state.depot_keys.insert(depot_id.to_string(), key.to_string());
            imported += 1;
        }
    }

    let mut created = false;
    let exists = state.apps.contains_key(&appid.to_string());
    if !exists && create {
        let depots: Vec<Value> = parsed
            .depots
            .iter()
            .map(|depot| {
                json!({
                    "depot_id": depot.get("depot_id").cloned().unwrap_or(Value::Null),
                    "gid": depot.get("gid").cloned().filter(|value| !value.is_null()).unwrap_or_else(|| json!("")),
                    "size": depot.get("size").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        let record = json!({
            "appid": appid,
            "name": name.unwrap_or(&appid.to_string()),
            "branch": "public",
            "depots": depots,
            "dlc": parsed.dlc,
            "source": "hubcap",
            "fixes": { "ticket": "forge", "steam_id": "", "avatar": 0, "purchase_time": 0, "online_fix": false },
            "updated": stamp,
        });
        state.apps.insert(appid.to_string(), record);
        created = true;
    }

    let mut pinned = 0;
    if !created && pin {
        if let Some(record) = state.apps.get_mut(&appid.to_string()) {
            let mut planned: std::collections::BTreeMap<u64, Value> = std::collections::BTreeMap::new();
            if let Some(list) = record.get("depots").and_then(Value::as_array) {
                for entry in list {
                    if let Some(id) = entry.get("depot_id").and_then(Value::as_u64) {
                        planned.insert(id, entry.clone());
                    }
                }
            }
            for depot in &parsed.depots {
                let Some(id) = depot.get("depot_id").and_then(Value::as_u64) else { continue };
                let Some(gid) = depot.get("gid").and_then(Value::as_str).filter(|value| !value.is_empty()) else { continue };
                if let Some(entry) = planned.get_mut(&id) {
                    entry["gid"] = json!(gid);
                    if let Some(size) = depot.get("size").filter(|value| !value.is_null()) {
                        entry["size"] = size.clone();
                    }
                    pinned += 1;
                }
            }
            if let Some(list) = record.get("depots").and_then(Value::as_array).map(|list| {
                list.iter()
                    .map(|entry| planned.get(&entry.get("depot_id").and_then(Value::as_u64).unwrap_or(0)).cloned().unwrap_or_else(|| entry.clone()))
                    .collect::<Vec<_>>()
            }) {
                record["depots"] = Value::Array(list);
            }
            record["updated"] = json!(stamp);
        }
    }

    let missing: Vec<u64> = parsed
        .depots
        .iter()
        .filter(|depot| depot.get("key").and_then(Value::as_str).map(str::is_empty).unwrap_or(true))
        .filter_map(|depot| depot.get("depot_id").and_then(Value::as_u64))
        .collect();

    json!({
        "appid": appid,
        "keys": imported,
        "depots": parsed.depots.len(),
        "pinned": pinned,
        "created_plan": created,
        "missing_keys": missing,
        "dlc": parsed.dlc,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    const LUA: &str = r#"
addappid(620)
addappid(621, 1, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
setmanifestid(621, "9044256803759256265", 1200)
addappid(650)
"#;

    #[test]
    fn parses_depots_keys_and_pins() {
        let parsed = parse_lua(LUA, Some(620));
        assert_eq!(parsed.appid, Some(620));
        assert_eq!(parsed.dlc, vec![650]);
        let depot = parsed
            .depots
            .iter()
            .find(|item| item.get("depot_id").and_then(Value::as_u64) == Some(621))
            .unwrap();
        assert_eq!(depot.get("gid").and_then(Value::as_str), Some("9044256803759256265"));
        assert_eq!(depot.get("size").and_then(Value::as_u64), Some(1200));
        assert_eq!(depot.get("key").and_then(Value::as_str).map(str::len), Some(64));
    }

    #[test]
    fn keys_land_without_pinning() {
        let parsed = parse_lua(LUA, Some(620));
        let mut st = crate::state::State::default();
        let summary = apply_to_state(&mut st, &parsed, "1700000000", Some("Portal 2"), false, false);
        assert_eq!(summary.get("keys").and_then(Value::as_u64), Some(1));
        assert_eq!(st.depot_keys.get("621").map(String::len), Some(64));
        assert!(st.apps.is_empty(), "no plan unless asked");
    }

    #[test]
    fn create_writes_a_plan() {
        let parsed = parse_lua(LUA, Some(620));
        let mut st = crate::state::State::default();
        let summary = apply_to_state(&mut st, &parsed, "1700000000", Some("Portal 2"), false, true);
        assert_eq!(summary.get("created_plan").and_then(Value::as_bool), Some(true));
        let record = st.apps.get("620").unwrap();
        assert_eq!(record.get("name").and_then(Value::as_str), Some("Portal 2"));
        assert_eq!(record.get("depots").and_then(Value::as_array).map(Vec::len), Some(1));
    }

    #[test]
    fn validates_keys() {
        assert!(valid_key("smm_1234567890abcdef").is_some());
        assert!(valid_key("nope_1234567890abc").is_none());
        assert!(valid_key("smm has space here").is_none());
    }
}
