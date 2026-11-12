mod support;

use std::io::{Read, Write};
use std::net::TcpListener;

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn serve_one(lua: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(4) {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                lua.len(),
                lua
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

#[test]
fn a_plan_source_becomes_an_installed_script() {
    let tree = support::Tree::new("plan-flow");
    tree.apply();
    let lua: &'static str = Box::leak(
        format!("addappid(620)\naddappid(621, 1, \"{KEY}\")\nsetmanifestid(621, \"9044256803759256265\", 1200)\n").into_boxed_str(),
    );
    let port = serve_one(lua);

    let mut state = exxa::state::State::default();
    state.settings.plan_sources.insert(
        "fixture".to_string(),
        serde_json::json!({ "url": format!("http://127.0.0.1:{port}/{{appid}}.lua") }),
    );
    exxa::state::save(&state).expect("state save");

    let result = exxa::plans::fetch_plan(620, Some("Portal 2")).expect("fetch plan");
    assert_eq!(result["summary"]["keys"], 1, "key count: {result}");
    assert_eq!(result["installed"], true, "script not installed: {result}");

    let reloaded = exxa::state::load().expect("state");
    assert!(reloaded.depot_keys.contains_key("621"));

    let script = std::fs::read_to_string(exxa::paths::vf_scripts().join("exxatools-620.lua")).expect("script");
    assert!(script.contains("addappid(621, 1"), "key missing from script: {script}");

    let report = exxa::verify::run(620);
    let items = report["items"].as_array().expect("items");
    let status = |id: &str| {
        items
            .iter()
            .find(|item| item["id"] == id)
            .and_then(|item| item["status"].as_str())
            .unwrap_or("missing")
            .to_string()
    };
    assert_eq!(status("script"), "ok", "script check: {report}");
    assert_eq!(status("keys"), "ok", "keys check: {report}");
    assert_eq!(status("lua"), "ok", "lua check: {report}");
}
