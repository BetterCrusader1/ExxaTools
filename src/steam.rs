//! Steam catalog access: store search, store metadata, appinfo depots, discover.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::http;

const STORE_SEARCH: &str = "https://store.steampowered.com/api/storesearch/?term={term}&l=english&cc={cc}";
const STORE_DETAILS: &str = "https://store.steampowered.com/api/appdetails?appids={appid}&l=english&cc={cc}";
const APPINFO: &str = "https://api.steamcmd.net/v1/info/{appid}";
const FEATURED: &str = "https://store.steampowered.com/api/featuredcategories?l=english&cc={cc}";
const STEAMSPY: &str = "https://steamspy.com/api.php?request=top100in2weeks";

const NOISE: &[&str] = &[
    "soundtrack",
    " demo",
    "bundle",
    "steam deck",
    "steam frame",
    "steam controller",
    " dlc",
    "season pass",
    "artbook",
    "wallpaper",
    "server",
    "beta",
];

fn price_of(item: &Value) -> Option<String> {
    let price = item.get("price").and_then(Value::as_object)?;
    let final_price = price.get("final").and_then(Value::as_i64).unwrap_or(0);
    if final_price == 0 {
        return Some("free".into());
    }
    let currency = price.get("currency").and_then(Value::as_str).unwrap_or("");
    Some(format!("{:.2} {}", final_price as f64 / 100.0, currency).trim().to_string())
}

pub fn search(term: &str, cc: &str) -> Result<Vec<Value>, String> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(Vec::new());
    }
    if term.chars().all(|ch| ch.is_ascii_digit()) {
        let appid: u64 = term.parse().map_err(|_| "bad app id".to_string())?;
        if let Some(info) = store_details(appid, cc) {
            return Ok(vec![json!({
                "appid": appid,
                "name": info.get("name").cloned().unwrap_or_else(|| json!(term)),
                "image": info.get("header_image").cloned().unwrap_or_else(|| json!("")),
                "price": price_of(&info),
            })]);
        }
        if let Ok(info) = app_info(appid) {
            let name = info.pointer("/common/name").and_then(Value::as_str).unwrap_or(term);
            return Ok(vec![json!({ "appid": appid, "name": name, "image": "", "price": null })]);
        }
    }
    // Steam's store search wants spaces as plus signs; %20 comes back empty.
    let encoded = http::quote(term).replace("%20", "+");
    let url = STORE_SEARCH.replace("{term}", &encoded).replace("{cc}", cc);
    let payload = http::get_json(&url).map_err(|error| error.to_string())?;
    let mut items = Vec::new();
    if let Some(entries) = payload.get("items").and_then(Value::as_array) {
        for item in entries {
            if item.get("type").and_then(Value::as_str) != Some("app") {
                continue;
            }
            let Some(appid) = item.get("id").and_then(Value::as_u64) else { continue };
            items.push(json!({
                "appid": appid,
                "name": item.get("name").cloned().unwrap_or_else(|| json!("")),
                "image": item.get("tiny_image").cloned().unwrap_or_else(|| json!("")),
                "price": price_of(item),
            }));
        }
    }
    Ok(items)
}

pub fn store_details(appid: u64, cc: &str) -> Option<Value> {
    let url = STORE_DETAILS.replace("{appid}", &appid.to_string()).replace("{cc}", cc);
    let payload = http::get_json(&url).ok()?;
    let entry = payload.get(appid.to_string())?;
    if entry.get("success").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    entry.get("data").cloned()
}

pub fn app_info(appid: u64) -> Result<Value, String> {
    let url = APPINFO.replace("{appid}", &appid.to_string());
    let payload = http::get_json(&url).map_err(|error| error.to_string())?;
    if payload.get("status").and_then(Value::as_str) != Some("success") {
        return Err(format!("appinfo for {appid} is unavailable"));
    }
    payload
        .pointer(&format!("/data/{appid}"))
        .cloned()
        .ok_or_else(|| format!("appinfo for {appid} has no entry"))
}

/// Normalised description of an app: name, branches, depots, dlc.
pub fn catalog(appid: u64) -> Result<Value, String> {
    // A test and an offline machine must take the same path: no silent network.
    if std::env::var("EXXATOOLS_OFFLINE").is_ok_and(|value| !value.trim().is_empty()) {
        return Err("offline mode: the Steam API is not called".into());
    }
    let info = app_info(appid)?;
    let name = info
        .pointer("/common/name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_string();
    let app_type = info.pointer("/common/type").and_then(Value::as_str).unwrap_or("unknown").to_string();
    let developers = info.pointer("/extended/developer").and_then(Value::as_str).unwrap_or("").to_string();
    let dlc = dlc_ids(&info);
    let mut branches = parse_branches(info.get("branches"));
    let depots = parse_depots(info.get("depots"));
    if branches.is_empty() {
        branches = branches_from_depots(&depots);
    }
    Ok(json!({
        "appid": appid,
        "name": if name.is_empty() { appid.to_string() } else { name },
        "type": app_type,
        "developers": developers,
        "dlc": dlc,
        "branches": branches,
        "depots": depots,
    }))
}

fn parse_branches(raw: Option<&Value>) -> Vec<Value> {
    let Some(map) = raw.and_then(Value::as_object) else { return Vec::new() };
    let mut out: Vec<Value> = Vec::new();
    for (name, value) in map {
        if value.get("buildid").is_none() {
            continue;
        }
        out.push(json!({
            "name": name,
            "buildid": value.get("buildid").map(value_text).unwrap_or_default(),
            "description": value.get("description").and_then(Value::as_str).unwrap_or(""),
            "timeupdated": value.get("timeupdated").and_then(Value::as_i64).unwrap_or(0),
        }));
    }
    out.sort_by(|a, b| {
        let name_a = a.get("name").and_then(Value::as_str).unwrap_or("");
        let name_b = b.get("name").and_then(Value::as_str).unwrap_or("");
        let time_a = a.get("timeupdated").and_then(Value::as_i64).unwrap_or(0);
        let time_b = b.get("timeupdated").and_then(Value::as_i64).unwrap_or(0);
        (name_a != "public", -time_a).cmp(&(name_b != "public", -time_b))
    });
    out
}

fn branches_from_depots(depots: &[Value]) -> Vec<Value> {
    let mut names: Vec<String> = Vec::new();
    for depot in depots {
        if let Some(manifests) = depot.get("manifests").and_then(Value::as_object) {
            for name in manifests.keys() {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
    }
    names.sort_by(|a, b| (a != "public", a).cmp(&(b != "public", b)));
    names
        .into_iter()
        .map(|name| json!({ "name": name, "buildid": "", "description": "", "timeupdated": 0 }))
        .collect()
}

fn dlc_ids(info: &Value) -> Vec<u64> {
    let raw = info.pointer("/extended/listofdlc").map(value_text).unwrap_or_default();
    raw.split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|chunk| !chunk.is_empty() && chunk.chars().all(|ch| ch.is_ascii_digit()))
        .filter_map(|chunk| chunk.parse().ok())
        .collect()
}

fn parse_depots(raw: Option<&Value>) -> Vec<Value> {
    let Some(map) = raw.and_then(Value::as_object) else { return Vec::new() };
    let mut out = Vec::new();
    for (key, value) in map {
        if !key.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let Some(manifests_raw) = value.get("manifests").and_then(Value::as_object) else { continue };
        let mut manifests = serde_json::Map::new();
        for (branch, entry) in manifests_raw {
            let Some(gid) = entry.get("gid").and_then(value_to_gid) else { continue };
            if gid.is_empty() {
                continue;
            }
            manifests.insert(
                branch.clone(),
                json!({
                    "gid": gid,
                    "size": entry.get("size").and_then(Value::as_u64),
                    "download": entry.get("download").and_then(Value::as_u64),
                }),
            );
        }
        if manifests.is_empty() {
            continue;
        }
        let config = value.get("config").cloned().unwrap_or(Value::Null);
        let oslist: Vec<String> = config
            .get("oslist")
            .and_then(Value::as_str)
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect();
        out.push(json!({
            "depot_id": key.parse::<u64>().unwrap_or(0),
            "oslist": oslist,
            "osarch": config.get("osarch").and_then(Value::as_str).unwrap_or(""),
            "shared_install": value.get("sharedinstall").and_then(Value::as_bool).unwrap_or(false),
            "dlc_appid": value.get("dlcappid").and_then(Value::as_u64),
            "depot_from_app": value.get("depotfromapp").and_then(Value::as_u64),
            "manifests": manifests,
        }));
    }
    out.sort_by_key(|depot| depot.get("depot_id").and_then(Value::as_u64).unwrap_or(0));
    out
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

fn value_to_gid(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub fn pick_manifest(depot: &Value, branch: &str) -> Option<Value> {
    let manifests = depot.get("manifests")?.as_object()?;
    manifests.get(branch).or_else(|| manifests.get("public")).cloned()
}

static DISCOVER_CACHE: Mutex<Option<(Instant, Vec<Value>)>> = Mutex::new(None);

fn rng(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn shuffle(items: &mut [Value]) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.subsec_nanos() as u64 | (delta.as_secs() << 20))
        .unwrap_or(0x9e3779b97f4a7c15);
    let mut seed = nanos | 1;
    for index in (1..items.len()).rev() {
        let pick = (rng(&mut seed) % (index as u64 + 1)) as usize;
        items.swap(index, pick);
    }
}

fn discover_pool(cc: &str) -> Vec<Value> {
    let mut pool: Vec<Value> = Vec::new();
    let mut seen: Vec<u64> = Vec::new();
    let push = |appid: u64, name: &str, pool: &mut Vec<Value>, seen: &mut Vec<u64>| {
        if name.is_empty() || seen.contains(&appid) {
            return;
        }
        let lower = name.to_lowercase();
        if NOISE.iter().any(|noise| lower.contains(noise)) {
            return;
        }
        seen.push(appid);
        pool.push(json!({ "appid": appid, "name": name }));
    };

    if let Ok(spy) = http::get_json_with(STEAMSPY, None) {
        if let Some(map) = spy.as_object() {
            for item in map.values() {
                if let (Some(appid), Some(name)) = (item.get("appid").and_then(Value::as_u64), item.get("name").and_then(Value::as_str)) {
                    push(appid, name, &mut pool, &mut seen);
                }
            }
        }
    }
    let featured_url = FEATURED.replace("{cc}", cc);
    if let Ok(featured) = http::get_json(&featured_url) {
        for section in ["top_sellers", "new_releases", "specials"] {
            let Some(items) = featured.pointer(&format!("/{section}/items")).and_then(Value::as_array) else { continue };
            for item in items {
                if let (Some(appid), Some(name)) = (item.get("id").and_then(Value::as_u64), item.get("name").and_then(Value::as_str)) {
                    push(appid, name, &mut pool, &mut seen);
                }
            }
        }
    }
    pool
}

pub fn discover(cc: &str, limit: usize) -> Vec<Value> {
    let cached = DISCOVER_CACHE.lock().ok().and_then(|guard| guard.clone());
    let mut items = match cached {
        Some((at, items)) if at.elapsed() < Duration::from_secs(900) && !items.is_empty() => items,
        _ => {
            let items = discover_pool(cc);
            if let Ok(mut guard) = DISCOVER_CACHE.lock() {
                *guard = Some((Instant::now(), items.clone()));
            }
            items
        }
    };
    shuffle(&mut items);
    items.truncate(limit.max(1));
    items
}
static DLC_CACHE: Mutex<Option<(Instant, Vec<Value>)>> = Mutex::new(None);

/// Names for an app's DLC list. The store only returns ids on the app itself,
/// so each id costs one small request; the result is cached for fifteen minutes.
pub fn dlc_names(appid: u64, cc: &str) -> Vec<Value> {
    if let Ok(guard) = DLC_CACHE.lock() {
        if let Some((at, items)) = guard.as_ref() {
            if at.elapsed() < Duration::from_secs(900) && !items.is_empty() {
                return items.clone();
            }
        }
    }
    let Some(details) = store_details(appid, cc) else { return Vec::new() };
    let ids: Vec<u64> = details
        .get("dlc")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_u64).take(12).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for id in ids {
        let name = store_details(id, cc)
            .and_then(|info| info.get("name").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| id.to_string());
        out.push(json!({ "appid": id, "name": name }));
    }
    if let Ok(mut guard) = DLC_CACHE.lock() {
        *guard = Some((Instant::now(), out.clone()));
    }
    out
}
