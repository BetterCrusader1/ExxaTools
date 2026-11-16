mod support;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

fn request(port: u16, method: &str, path: &str, token: &str, body: &Value) -> (u16, Value) {
    let payload = if method == "POST" { body.to_string() } else { String::new() };
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(20))).expect("read timeout");
    stream.set_write_timeout(Some(Duration::from_secs(20))).expect("write timeout");
    write!(stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nX-Exxa-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()
    ).expect("request");
    stream.flush().expect("flush");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("response");
    let (head, body) = response.split_once("\r\n\r\n").expect("HTTP response");
    let status = head.split_whitespace().nth(1).expect("status").parse().expect("numeric status");
    (status, serde_json::from_str(body).expect("JSON response"))
}

// One fixture and one server per integration-test binary: environment and the
// server's token/port OnceLocks are process-wide. No external provider is used.
#[test]
fn concurrent_routes_merge_deltas_and_failed_updates_roll_back() {
    let tree = support::Tree::new("api-transactions");
    tree.apply();
    std::env::set_var("EXXATOOLS_OFFLINE", "1");
    let mut initial = exxa::state::State::default();
    initial.codes.insert("unrelated".into(), 77);
    initial.plugins.insert("unrelated".into(), false);
    initial.settings.plan_sources.insert("retained".into(), json!({"url": "https://example.invalid/{appid}"}));
    for round in 0..8u64 {
        for index in 0..4u64 {
            let appid = 10000 + round * 10 + index;
            initial.apps.insert(appid.to_string(), json!({
                "appid": appid, "name": "Concurrent fixture", "branch": "public",
                "depots": [{"depot_id": appid + 1, "gid": "1", "enabled": true}], "dlc": []
            }));
        }
        for appid in [30000 + round, 40000 + round] {
            initial.apps.insert(appid.to_string(), json!({"appid": appid, "depots": []}));
        }
    }
    exxa::state::save(&initial).expect("initial state");
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    std::thread::spawn(move || {
        exxa::server::serve(exxa::server::Config { port }).expect("server");
    });
    let start = Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() { break; }
        assert!(start.elapsed() < Duration::from_secs(10), "server did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    let (status, session) = request(port, "GET", "/api/session", "", &Value::Null);
    assert_eq!(status, 200);
    let token = session["token"].as_str().expect("session token").to_string();
    // The reserved port was released before serve bound it; never mutate a
    // different server if another process won that small bind race.
    assert!(token == exxa::state::ensure_token().expect("fixture token"), "not the fixture server");

    for round in 0..8u64 {
        let mut jobs: Vec<(&str, String, Value)> = Vec::new();
        for index in 0..4u64 {
            let appid = 10000 + round * 10 + index;
            let imported = 20000 + round * 10 + index;
            jobs.push(("POST", "/api/import".into(), json!({
                "text": format!("addappid({imported})\naddappid({}, 1, \"{}\")\n", imported + 1, "a".repeat(64)),
                "create": true, "name": "Imported fixture"
            })));
            jobs.push(("POST", "/api/pin".into(), json!({"appid": appid, "depot_id": appid + 1, "gid": "900"})));
            jobs.push(("POST", "/api/depot".into(), json!({"appid": appid, "depot_id": appid + 1, "enabled": false})));
        }
        let country = if round % 2 == 0 { "gb" } else { "us" };
        let theme = if round % 2 == 0 { "light" } else { "dark" };
        let os = if round % 2 == 0 { "windows" } else { "linux" };
        jobs.push(("POST", "/api/settings".into(), json!({"country": country})));
        jobs.push(("POST", "/api/settings".into(), json!({"theme": theme})));
        jobs.push(("POST", "/api/settings".into(), json!({"default_os": os})));
        jobs.push(("POST", "/api/hubcap".into(), json!({"action": "save", "key": ""})));
        jobs.push(("POST", "/api/key".into(), json!({"depot_id": 50000 + round, "key": "b".repeat(64)})));
        jobs.push(("POST", "/api/fix".into(), json!({"action": "remove", "appid": 30000 + round})));
        jobs.push(("DELETE", format!("/api/plan?appid={}", 40000 + round), Value::Null));
        let barrier = Arc::new(Barrier::new(jobs.len()));
        let handles: Vec<_> = jobs.into_iter().map(|(method, path, body)| {
            let barrier = Arc::clone(&barrier);
            let token = token.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let (status, _) = request(port, method, &path, &token, &body);
                assert_eq!(status, 200, "{method} {path} failed");
            })
        }).collect();
        for handle in handles { handle.join().expect("route worker"); }

        let saved = exxa::state::load().expect("state after concurrent requests");
        for completed in 0..=round {
            for index in 0..4u64 {
                let appid = 10000 + completed * 10 + index;
                let imported = 20000 + completed * 10 + index;
                assert_eq!(saved.apps[&appid.to_string()]["depots"][0]["gid"], "900", "pin lost");
                assert_eq!(saved.apps[&appid.to_string()]["depots"][0]["enabled"], false, "depot flag lost");
                assert!(saved.apps.contains_key(&imported.to_string()), "imported plan lost");
                assert!(saved.depot_keys.contains_key(&(imported + 1).to_string()), "imported key lost");
            }
            assert!(!saved.apps.contains_key(&(30000 + completed).to_string()), "fix removal undone");
            assert!(!saved.apps.contains_key(&(40000 + completed).to_string()), "delete undone");
            assert!(saved.depot_keys.contains_key(&(50000 + completed).to_string()), "key write lost");
        }
        assert_eq!(saved.settings.country, country, "country update lost");
        assert_eq!(saved.settings.theme, theme, "theme update lost");
        assert_eq!(saved.settings.default_os, os, "OS update lost");
        assert_eq!(saved.settings.plan_sources, initial.settings.plan_sources);
        assert_eq!(saved.codes.get("unrelated"), Some(&77));
        assert_eq!(saved.plugins.get("unrelated"), Some(&false));
        assert_eq!(saved.token, token);
    }

    let before = serde_json::to_value(exxa::state::load().expect("before rejection")).expect("serialize");
    for (path, body, expected) in [
        ("/api/settings", json!({"country": "ca", "manifest_providers": 7}), 400),
        ("/api/pin", json!({"appid": 10000, "depot_id": 10001, "gid": "not-decimal"}), 400),
        ("/api/depot", json!({"appid": 10000, "depot_id": 999999, "enabled": true}), 404),
        ("/api/depot", json!({"appid": 999999, "depot_id": 1, "enabled": true}), 404),
    ] {
        let (status, _) = request(port, "POST", path, &token, &body);
        assert_eq!(status, expected, "wrong validation status for {path}");
    }
    let after = serde_json::to_value(exxa::state::load().expect("after rejection")).expect("serialize");
    assert_eq!(before, after, "a rejected route persisted a partial update");
}
