//! Plan sources: `.lua` files that carry depot keys and manifest pins.
//!
//! Manifest providers hand out request codes, which is a different thing: a code
//! only works when Steam already believes the depot is owned. The keys come from
//! these sources, and every one of them is a community service that can refuse,
//! rate-limit or vanish, so each failure is reported with the provider's name.

use serde_json::{json, Value};

use crate::state::{self, Settings};

pub struct PlanSource {
    pub name: String,
    pub kind: Kind,
}

pub enum Kind {
    Hubcap(String),
    Ryuu(String),
    Custom { url: String, header: Option<(String, String)> },
}

/// Built-in sources in the order they are tried. A source without a key is
/// skipped instead of failing the whole request.
pub fn sources(settings: &Settings) -> Vec<PlanSource> {
    let mut out = Vec::new();
    if !settings.hubcap_key.trim().is_empty() {
        out.push(PlanSource { name: "hubcap".into(), kind: Kind::Hubcap(settings.hubcap_key.clone()) });
    }
    if !settings.ryuu_key.trim().is_empty() {
        out.push(PlanSource { name: "ryuu".into(), kind: Kind::Ryuu(settings.ryuu_key.clone()) });
    }
    for (name, spec) in &settings.plan_sources {
        let Some(object) = spec.as_object() else { continue };
        let Some(url) = object.get("url").and_then(Value::as_str).filter(|value| value.contains("{appid}")) else { continue };
        let header = object
            .get("header")
            .and_then(Value::as_str)
            .and_then(|pair| pair.split_once(':'))
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()));
        out.push(PlanSource {
            name: name.clone(),
            kind: Kind::Custom { url: url.to_string(), header },
        });
    }
    out
}

impl PlanSource {
    fn fetch(&self, appid: u64) -> Result<String, String> {
        match &self.kind {
            Kind::Hubcap(key) => crate::hubcap::fetch(key, appid).map(|fetched| fetched.lua).map_err(|error| error.to_string()),
            Kind::Ryuu(key) => {
                let url = format!("https://generator.ryuu.lol/api/download/{appid}?file_type=lua");
                let body = crate::http::get_with_headers(&url, &[("X-Auth-Key", key.as_str())]).map_err(|error| error.to_string())?;
                let text = String::from_utf8_lossy(&body).into_owned();
                if text.trim_start().starts_with('<') {
                    return Err("ryuu answered with an HTML page (login or challenge)".into());
                }
                if text.trim().is_empty() {
                    return Err("ryuu returned an empty file".into());
                }
                Ok(text)
            }
            Kind::Custom { url, header } => {
                let target = url.replace("{appid}", &appid.to_string());
                let headers: Vec<(&str, &str)> = header
                    .as_ref()
                    .map(|(key, value)| vec![(key.as_str(), value.as_str())])
                    .unwrap_or_default();
                let body = crate::http::get_with_headers(&target, &headers).map_err(|error| error.to_string())?;
                Ok(String::from_utf8_lossy(&body).into_owned())
            }
        }
    }
}

/// Fetches a plan, stores its keys and writes the script. Returns the provider
/// that answered plus what it contributed.
pub fn fetch_plan(appid: u64, name_hint: Option<&str>) -> Result<Value, String> {
    let settings = state::load().map_err(|error| error)?;
    let list = sources(&settings.settings);
    if list.is_empty() {
        return Err("no plan source configured: add a Hubcap or Ryuu key in Settings".into());
    }
    let mut failures: Vec<Value> = Vec::new();
    for source in list {
        match source.fetch(appid) {
            Ok(lua) => {
                let parsed = crate::hubcap::parse_lua(&lua, Some(appid));
                if parsed.appid.is_none() {
                    failures.push(json!({ "provider": source.name, "error": "the file has no app id" }));
                    continue;
                }
                let resolved = name_hint.map(str::to_string).or_else(|| {
                    crate::steam::catalog(appid)
                        .ok()
                        .and_then(|catalog| catalog.get("name").and_then(Value::as_str).map(str::to_string))
                });
                let mut fresh = state::load().map_err(|error| error)?;
                let summary = crate::hubcap::apply_to_state(&mut fresh, &parsed, &crate::vfsys::stamp(), resolved.as_deref(), false, true);
                state::save(&fresh).map_err(|error| error)?;
                let installed = write_script(appid).unwrap_or(false);
                return Ok(json!({
                    "provider": source.name,
                    "summary": summary,
                    "installed": installed,
                    "failures": failures,
                }));
            }
            Err(error) => failures.push(json!({ "provider": source.name, "error": error })),
        }
    }
    Err(format!(
        "no plan source answered for {appid}: {}",
        failures.iter().filter_map(|entry| entry.get("error").and_then(Value::as_str)).collect::<Vec<_>>().join("; ")
    ))
}

/// Rewrites the installed script from the saved plan, so keys that arrived after
/// the plan was created are picked up too.
pub fn write_script(appid: u64) -> Result<bool, String> {
    let st = state::load().map_err(|error| error)?;
    let Some(record) = st.apps.get(&appid.to_string()) else { return Ok(false) };
    let name = record.get("name").and_then(Value::as_str).unwrap_or("");
    let branch = record.get("branch").and_then(Value::as_str).unwrap_or("public");
    let depots = record.get("depots").and_then(Value::as_array).cloned().unwrap_or_default();
    let dlc: Vec<u64> = record
        .get("dlc")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let text = crate::lua::render_plan(appid, name, branch, &depots, &st.depot_keys, &dlc, "auto");
    crate::vfsys::write_script(appid, &text, true).map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn settings_with(hubcap: &str, ryuu: &str) -> Settings {
        let mut settings = Settings::default();
        settings.hubcap_key = hubcap.into();
        settings.ryuu_key = ryuu.into();
        settings
    }

    #[test]
    fn skips_sources_without_keys() {
        let list = sources(&settings_with("", ""));
        assert!(list.is_empty());
        let list = sources(&settings_with("", "abc"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "ryuu");
    }

    #[test]
    fn keeps_the_configured_order() {
        let list = sources(&settings_with("hub", "ryuu"));
        let names: Vec<&str> = list.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(names, vec!["hubcap", "ryuu"]);
    }

    #[test]
    fn custom_source_needs_appid_placeholder() {
        let mut settings = Settings::default();
        settings.plan_sources.insert("bad".into(), json!({ "url": "http://example.com/fixed.lua" }));
        settings.plan_sources.insert("good".into(), json!({ "url": "http://127.0.0.1:1/lua/{appid}.lua" }));
        let list = sources(&settings);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "good");
    }

    #[test]
    fn falls_back_to_the_next_source() {
        // One server that answers with a lua file; the dead port in front of it
        // makes the first source fail the way a refused provider does.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let body = "addappid(620)\naddappid(621, 1, \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\")\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let mut settings = Settings::default();
        settings.plan_sources.insert("dead".into(), json!({ "url": "http://127.0.0.1:1/lua/{appid}.lua" }));
        settings.plan_sources.insert("live".into(), json!({ "url": format!("http://127.0.0.1:{port}/lua/{{appid}}.lua") }));
        let list = sources(&settings);
        assert_eq!(list.len(), 2, "both custom sources should register");
        let mut tried = Vec::new();
        let mut lua = None;
        for source in list {
            tried.push(source.name.clone());
            if let Ok(text) = source.fetch(620) {
                lua = Some(text);
                break;
            }
        }
        assert_eq!(tried, vec!["dead", "live"]);
        assert!(lua.unwrap().contains("addappid(621"));
    }
}
