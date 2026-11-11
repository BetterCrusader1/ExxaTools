//! Manifest request-code providers.
//!
//! These are the same small shared services the Vapor Forge runtime uses.
//! Requests are serialised per provider with a minimum interval, so one client
//! cannot burst a service the whole community depends on.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::http;

const MIN_INTERVAL: Duration = Duration::from_secs(1);
const COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct Spec {
    pub name: String,
    pub url: String,
    pub format: String,
    pub json_field: Option<String>,
    pub user_agent: Option<String>,
}

fn builtin(name: &str) -> Option<Spec> {
    let (url, format, field, agent) = match name {
        "steamrun" => ("https://manifest.steam.run/api/manifest/{gid}", "json", Some("content"), Some("ExxaTools/1.0")),
        "20770407" => ("https://20770407.xyz/manifest/{depotId}/{gid}", "plain", None, Some("ExxaTools/1.0")),
        "wudrm" => ("http://gmrc.wudrm.com/manifest/{gid}", "plain", None, None),
        "manifestdex" => ("https://manifest.manifestdex.com/{gid}", "plain", None, Some("ManifestDeX/1.0")),
        "opensteamtool" => ("https://manifest.opensteamtool.com/{gid}", "plain", None, Some("OpenSteamTool/1.0")),
        _ => return None,
    };
    Some(Spec {
        name: name.to_string(),
        url: url.to_string(),
        format: format.to_string(),
        json_field: field.map(str::to_string),
        user_agent: agent.map(str::to_string),
    })
}

pub fn default_order() -> Vec<Value> {
    ["steamrun", "20770407", "wudrm", "manifestdex", "opensteamtool"]
        .iter()
        .map(|name| Value::String((*name).to_string()))
        .collect()
}

pub fn resolve(entry: &Value) -> Option<Spec> {
    match entry {
        Value::String(name) => builtin(name),
        Value::Object(map) => {
            let name = map.get("name").and_then(Value::as_str)?.trim().to_string();
            let url = map.get("url").and_then(Value::as_str)?.trim().to_string();
            if name.is_empty() || !url.contains("{gid}") {
                return None;
            }
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return None;
            }
            let format = map.get("format").and_then(Value::as_str).unwrap_or("plain").to_string();
            let json_field = map.get("json_field").and_then(Value::as_str).map(str::to_string);
            if format == "json" && json_field.is_none() {
                return None;
            }
            Some(Spec {
                name,
                url,
                format,
                json_field,
                user_agent: map.get("user_agent").and_then(Value::as_str).map(str::to_string),
            })
        }
        _ => None,
    }
}

fn build_url(spec: &Spec, app_id: u64, depot_id: u64, gid: &str) -> String {
    spec.url
        .replace("{gid}", gid)
        .replace("{depotId}", &depot_id.to_string())
        .replace("{appId}", &app_id.to_string())
}

fn parse_code(body: &str, spec: &Spec) -> Result<u64, String> {
    let mut text = body.trim().to_string();
    if text.is_empty() {
        return Err("empty response".into());
    }
    if text.starts_with('<') {
        return Err("HTML response (challenge or outage)".into());
    }
    if spec.format == "json" || text.starts_with('{') {
        let payload: Value = serde_json::from_str(&text).map_err(|_| "malformed JSON".to_string())?;
        let field = spec.json_field.clone().unwrap_or_else(|| "content".into());
        text = match &payload {
            Value::Object(_) => payload.get(&field).map(value_to_text).unwrap_or_default(),
            other => value_to_text(other),
        };
    }
    if text.is_empty() || !text.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("not a manifest code: {:.60}", text));
    }
    text.parse::<u64>().map_err(|_| "manifest code out of range".to_string())
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

struct Limiter {
    last: Mutex<HashMap<String, Instant>>,
    cool: Mutex<HashMap<String, Instant>>,
}

static LIMITER: std::sync::LazyLock<Limiter> = std::sync::LazyLock::new(|| Limiter {
    last: Mutex::new(HashMap::new()),
    cool: Mutex::new(HashMap::new()),
});

fn wait_for(name: &str) -> Result<(), String> {
    let until = LIMITER.cool.lock().ok().and_then(|map| map.get(name).copied());
    let now = Instant::now();
    if let Some(until) = until {
        if until > now {
            return Err(format!("{name} is cooling down for {}s", (until - now).as_secs()));
        }
    }
    let delay = {
        let mut last = LIMITER.last.lock().map_err(|_| "limiter poisoned".to_string())?;
        let previous = last.get(name).copied();
        last.insert(name.to_string(), now);
        match previous {
            Some(previous) if now.duration_since(previous) < MIN_INTERVAL => MIN_INTERVAL - now.duration_since(previous),
            _ => Duration::ZERO,
        }
    };
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
    Ok(())
}

fn cool_down(name: &str) {
    if let Ok(mut map) = LIMITER.cool.lock() {
        map.insert(name.to_string(), Instant::now() + COOLDOWN);
    }
}

pub fn fetch_code(app_id: u64, depot_id: u64, gid: &str, order: &[Value]) -> Result<Value, String> {
    let mut failures = serde_json::Map::new();
    let specs: Vec<Spec> = order.iter().filter_map(resolve).collect();
    if specs.is_empty() {
        return Err("no usable manifest providers configured".into());
    }
    for spec in specs {
        let attempt = wait_for(&spec.name).and_then(|_| {
            let url = build_url(&spec, app_id, depot_id, gid);
            let body = http::get_text_with(&url, spec.user_agent.as_deref(), Some(Duration::from_secs(15))).map_err(|error| error.to_string())?;
            parse_code(&body, &spec)
        });
        match attempt {
            Ok(code) => {
                return Ok(json!({
                    "code": code,
                    "provider": spec.name,
                    "failures": failures,
                }))
            }
            Err(reason) => {
                failures.insert(spec.name.clone(), Value::String(reason));
                cool_down(&spec.name);
            }
        }
    }
    let detail: Vec<String> = failures
        .iter()
        .map(|(name, reason)| format!("{name}: {}", reason.as_str().unwrap_or("failed")))
        .collect();
    Err(format!("every provider failed: {}", detail.join("; ")))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: &str, field: Option<&str>) -> Spec {
        Spec {
            name: "test".into(),
            url: "https://example.invalid/{gid}".into(),
            format: kind.into(),
            json_field: field.map(str::to_string),
            user_agent: None,
        }
    }

    #[test]
    fn parses_plain_codes() {
        assert_eq!(parse_code(" 12345 \n", &spec("plain", None)).unwrap(), 12345);
    }

    #[test]
    fn parses_json_field() {
        let body = r#"{"content":"987654321"}"#;
        assert_eq!(parse_code(body, &spec("json", Some("content"))).unwrap(), 987654321);
    }

    #[test]
    fn rejects_html_and_garbage() {
        assert!(parse_code("<html>challenge</html>", &spec("plain", None)).is_err());
        assert!(parse_code("not a code", &spec("plain", None)).is_err());
        assert!(parse_code("", &spec("plain", None)).is_err());
    }

    #[test]
    fn builds_urls_from_templates() {
        let built = build_url(&spec("plain", None), 620, 621, "42");
        assert_eq!(built, "https://example.invalid/42");
    }

    #[test]
    fn resolves_custom_entries() {
        let entry = serde_json::json!({ "name": "mine", "url": "https://x.test/{gid}", "format": "plain" });
        assert!(resolve(&entry).is_some());
        let broken = serde_json::json!({ "name": "mine", "url": "https://x.test/none", "format": "plain" });
        assert!(resolve(&broken).is_none());
    }
}
