mod support;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};
use serde_json::{json, Value};

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
static PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

fn request(raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", *PORT.get().expect("test port"))).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(20))).expect("read timeout");
    stream.set_write_timeout(Some(Duration::from_secs(20))).expect("write timeout");
    stream.write_all(raw.as_bytes()).expect("write");
    stream.flush().ok();
    let mut text = String::new();
    stream.read_to_string(&mut text).expect("read");
    text
}

fn wait_ready() {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if TcpStream::connect(("127.0.0.1", *PORT.get().expect("test port"))).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server did not start");
}

fn body_of(response: &str) -> &str {
    response.split_once("\r\n\r\n").map(|(_, body)| body).unwrap_or("")
}

fn post_settings(token: &str, value: &Value) -> Value {
    let payload = value.to_string();
    let response = request(&format!(
        "POST /api/settings HTTP/1.1\r\nHost: local\r\nX-Exxa-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()
    ));
    assert!(response.starts_with("HTTP/1.1 200"), "settings save failed");
    assert_public(&response);
    serde_json::from_str(body_of(&response)).expect("saved settings JSON")
}

fn assert_stored_settings(expected: &Value) {
    let stored = serde_json::to_value(exxa::state::load().expect("stored settings").settings)
        .expect("stored settings JSON");
    // Do not print even fixture credentials on assertion failure.
    assert!(&stored == expected, "stored settings differ from the expected private configuration");
}

fn assert_public(response: &str) {
    for marker in ["supersecret", "secret.invalid", KEY] {
        assert!(!response.contains(marker), "a credential or private URL was exposed");
    }
    for field in ["hubcap_key", "ryuu_key", "key_source_url", "url", "header", "headers", "authorization", "password", "api_key", "credentials", "unknown"] {
        assert!(!response.contains(&format!("\"{field}\":")), "private field exposed: {field}");
    }
}

fn assert_settings(view: &Value) {
    assert_eq!(view["country"], "us");
    assert_eq!(view["default_os"], "linux");
    assert_eq!(view["default_source"], "auto");
    assert_eq!(view["theme"], "dark");
    assert_eq!(view["has_hubcap_key"], true);
    assert_eq!(view["has_ryuu_key"], true);
    assert_eq!(view["sources"]["custom"]["label"], "Custom provider");
    assert_eq!(view["sources"]["custom"]["enabled"], true);
    assert_eq!(view["sources"]["custom"]["providers"], json!(["steamrun", "custom-provider"]));
    assert_eq!(view["plan_sources"]["custom-plan"]["label"], "Custom plan");
    assert_eq!(view["manifest_providers"], json!(["steamrun"]));
}

fn token(response: &str) -> String {
    let text = body_of(response);
    let start = text.find("\"token\":\"").expect("token field") + "\"token\":\"".len();
    let rest = &text[start..];
    rest[..rest.find('"').expect("token end")].to_string()
}

#[test]
fn secrets_stay_inside_the_state_file() {
    let tree = support::Tree::new("httpsec");
    tree.apply();
    std::env::set_var("EXXATOOLS_OFFLINE", "1");
    let mut state = exxa::state::State::default();
    state.settings.hubcap_key = "smm_supersecret_value".into();
    state.settings.ryuu_key = "ryuu_supersecret_value".into();
    state.settings.key_source_url = "https://secret.invalid/?key=supersecret".into();
    state.settings.manifest_providers = vec!["steamrun".into()];
    state.settings.sources.insert("custom".into(), json!({
        "label": "Custom provider", "enabled": true,
        "key_source_url": "https://secret.invalid/keys",
        "api_key": "source_supersecret",
        "unknown": {"deep": [{"password": "nested_supersecret"}]},
        "providers": ["steamrun", {
            "name": "custom-provider", "url": "https://secret.invalid/{gid}",
            "headers": {"Authorization": "Bearer provider_supersecret"},
            "credentials": {"password": "provider_password_supersecret"}
        }, "https://secret.invalid/provider?token=supersecret"]
    }));
    state.settings.plan_sources.insert("custom-plan".into(), json!({
        "label": "Custom plan", "url": "https://secret.invalid/{appid}",
        "header": "Authorization: Bearer plan_supersecret",
        "headers": {"X-Key": "plan_header_supersecret"},
        "unknown": {"credentials": ["future_supersecret"]}
    }));
    state.depot_keys.insert("621".to_string(), KEY.to_string());
    exxa::state::save(&state).expect("state");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = std::fs::metadata(exxa::paths::state_path()).expect("state file").permissions().mode() & 0o777;
        assert_eq!(file, 0o600, "state file must be private");
        let dir = std::fs::metadata(exxa::paths::state_dir()).expect("state dir").permissions().mode() & 0o777;
        assert_eq!(dir, 0o700, "state dir must be private");
    }

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve test port");
    let port = listener.local_addr().expect("test address").port();
    PORT.set(port).expect("one server per test binary");
    drop(listener);
    std::thread::spawn(move || {
        exxa::server::serve(exxa::server::Config { port }).expect("fixture server");
    });
    wait_ready();

    for path in ["/api/settings", "/api/status", "/api/library", "/api/sources"] {
        let response = request(&format!("GET {path} HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n"));
        assert!(response.starts_with("HTTP/1.1 200"), "GET {path} failed");
        assert_public(&response);
        let value: Value = serde_json::from_str(body_of(&response)).expect("JSON response");
        match path {
            "/api/settings" => assert_settings(&value),
            "/api/status" | "/api/library" => assert_settings(&value["settings"]),
            _ => assert!(value["sources"].as_array().expect("sources").iter().any(|source| source["name"] == "custom")),
        }
    }

    let key = request("GET /api/key?depot_id=621 HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n");
    assert!(!key.contains(KEY), "depot key leaked: {key}");
    assert!(key.contains("has_key"));

    let session = request("GET /api/session HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n");
    let token = token(&session);
    assert_eq!(token.len(), 64);
    // Refuse to send mutations if another process won the bind race.
    assert!(token == exxa::state::ensure_token().expect("fixture token"), "not the fixture server");

    let payload = serde_json::to_string(&state.settings).expect("settings payload");
    let saved = request(&format!(
        "POST /api/settings HTTP/1.1\r\nHost: local\r\nX-Exxa-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()
    ));
    assert!(saved.starts_with("HTTP/1.1 200"), "settings save failed");
    assert_public(&saved);
    assert_settings(&serde_json::from_str::<Value>(body_of(&saved)).expect("saved settings"));
    let stored = exxa::state::load().expect("stored settings");
    assert_eq!(stored.settings.sources, state.settings.sources, "projection changed stored provider configuration");
    assert_eq!(stored.settings.plan_sources, state.settings.plan_sources);
    assert_eq!(stored.settings.hubcap_key, state.settings.hubcap_key);

    // Re-post the actual public GET body, not a serialization of private state.
    // In particular, provider objects and URL-only providers must survive their
    // lossy public name list, along with all omitted source fields.
    let response = request("GET /api/settings HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert_public(&response);
    let public: Value = serde_json::from_str(body_of(&response)).expect("public settings");
    let mut expected = serde_json::to_value(&state.settings).expect("expected settings");
    assert_settings(&post_settings(&token, &public));
    assert_stored_settings(&expected);

    // Read-only indicators cannot clear credentials or enter stored objects.
    let mut indicators = public.clone();
    indicators["has_hubcap_key"] = json!(false);
    indicators["has_ryuu_key"] = json!(false);
    indicators["sources"]["custom"]["has_api_key"] = json!(false);
    indicators["plan_sources"]["custom-plan"]["has_headers"] = json!(false);
    post_settings(&token, &indicators);
    assert_stored_settings(&expected);

    // Partial maps add IDs without deleting omitted IDs. Explicit field
    // updates still work; omitted provider-object credentials are retained.
    post_settings(&token, &json!({
        "sources": {"added": {"label": "Added", "enabled": false}},
        "plan_sources": {"added-plan": {"label": "Added plan"}}
    }));
    expected["sources"]["added"] = json!({"label": "Added", "enabled": false});
    expected["plan_sources"]["added-plan"] = json!({"label": "Added plan"});
    assert_stored_settings(&expected);
    post_settings(&token, &json!({
        "sources": {"custom": {
            "label": "Updated provider", "enabled": false,
            "key_source_url": "https://secret.invalid/updated",
            "providers": [{"name": "custom-provider", "url": "https://secret.invalid/updated/{gid}"}, "steamrun"]
        }},
        "plan_sources": {"custom-plan": {
            "label": "Updated plan", "url": "https://secret.invalid/updated/{appid}",
            "header": "", "headers": null
        }}
    }));
    expected["sources"]["custom"]["label"] = json!("Updated provider");
    expected["sources"]["custom"]["enabled"] = json!(false);
    expected["sources"]["custom"]["key_source_url"] = json!("https://secret.invalid/updated");
    let mut provider = expected["sources"]["custom"]["providers"][1].clone();
    provider["url"] = json!("https://secret.invalid/updated/{gid}");
    expected["sources"]["custom"]["providers"] = json!([provider, "steamrun"]);
    expected["plan_sources"]["custom-plan"]["label"] = json!("Updated plan");
    expected["plan_sources"]["custom-plan"]["url"] = json!("https://secret.invalid/updated/{appid}");
    expected["plan_sources"]["custom-plan"]["header"] = json!("");
    expected["plan_sources"]["custom-plan"]["headers"] = Value::Null;
    assert_stored_settings(&expected);

    // Changing order via public names must retain the matching private object.
    post_settings(&token, &json!({"sources": {"custom": {"providers": ["steamrun", "custom-provider"]}}}));
    let provider = expected["sources"]["custom"]["providers"][0].clone();
    expected["sources"]["custom"]["providers"] = json!(["steamrun", provider]);
    assert_stored_settings(&expected);

    // Empty per-source objects omit every field, whereas an explicitly empty
    // source map keeps the existing full-map removal semantics.
    post_settings(&token, &json!({"sources": {"custom": {}}, "plan_sources": {"custom-plan": {}}}));
    assert_stored_settings(&expected);
    post_settings(&token, &json!({"sources": {}, "plan_sources": {}}));
    expected["sources"] = json!({});
    expected["plan_sources"] = json!({});
    assert_stored_settings(&expected);

    // Deserialization errors must not echo rejected secret-bearing values.
    let payload = json!({"country": "gb", "manifest_providers": "invalid_supersecret"}).to_string();
    let invalid = request(&format!(
        "POST /api/settings HTTP/1.1\r\nHost: local\r\nX-Exxa-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()
    ));
    assert!(invalid.starts_with("HTTP/1.1 400"), "invalid settings accepted");
    assert_public(&invalid);
    assert_eq!(exxa::state::load().expect("state after rejection").settings.country, "us");

    let payload = format!("{{\"depot_id\":622,\"key\":\"{KEY}\"}}");
    let denied = request(&format!(
        "POST /api/key HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    ));
    assert!(denied.starts_with("HTTP/1.1 403"), "write without token was allowed: {denied}");

    let allowed = request(&format!(
        "POST /api/key HTTP/1.1\r\nHost: local\r\nX-Exxa-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    ));
    assert!(allowed.starts_with("HTTP/1.1 200"), "write with token failed: {allowed}");
    assert!(!allowed.contains(KEY), "the write echoed the key back");

    let wrong_type = request(&format!(
        "POST /api/key HTTP/1.1\r\nHost: local\r\nX-Exxa-Token: {token}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    ));
    assert!(wrong_type.starts_with("HTTP/1.1 415"), "text/plain body accepted: {wrong_type}");

    let huge = format!("{{\"pad\":\"{}\"}}", "x".repeat(2 * 1024 * 1024));
    let too_big = request(&format!(
        "POST /api/key HTTP/1.1\r\nHost: local\r\nX-Exxa-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        huge.len()
    ));
    assert!(too_big.starts_with("HTTP/1.1 413"), "oversized body accepted: {}", &too_big[..40.min(too_big.len())]);
}
