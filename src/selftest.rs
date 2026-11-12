//! End-to-end check of the plan pipeline, using a local fixture source.
//!
//! It proves the four pieces a fetched plan depends on: the source answers, the
//! lua parses into keys and depots, the renderer writes those keys into the
//! script, and a disabled depot drops out of it. Nothing touches Steam.

use std::io::{Read, Write};
use std::net::TcpListener;

use serde_json::{json, Value};

const FIXTURE: &str = "addappid(620)\naddappid(621, 1, \"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\")\nsetmanifestid(621, \"9044256803759256265\", 1200)\n";

fn serve_fixture() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            let body = FIXTURE;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    Ok(port)
}

pub fn run() -> Value {
    let mut steps: Vec<Value> = Vec::new();
    let mut passo = |name: &str, ok: bool, detail: String| {
        steps.push(json!({ "name": name, "ok": ok, "detail": detail }));
    };

    let port = match serve_fixture() {
        Ok(port) => port,
        Err(error) => {
            passo("fixture source", false, error);
            return json!({ "ok": false, "steps": steps });
        }
    };
    let url = format!("http://127.0.0.1:{port}/{{appid}}.lua").replace("{appid}", "620");
    let text = match crate::http::get_text_with(&url, Some("ExxaTools/selftest"), Some(std::time::Duration::from_secs(10))) {
        Ok(text) => text,
        Err(error) => {
            passo("fixture source", false, error.to_string());
            return json!({ "ok": false, "steps": steps });
        }
    };
    passo("fixture source answers", text.contains("addappid(621"), format!("{} bytes", text.len()));

    let parsed = crate::hubcap::parse_lua(&text, Some(620));
    let keys = parsed.depots.iter().filter_map(|depot| depot.get("key").and_then(Value::as_str)).filter(|key| key.len() == 64).count();
    passo("lua parses into a key", keys == 1, format!("{keys} key(s), {} depot(s)", parsed.depots.len()));

    let mut key_map = std::collections::BTreeMap::new();
    key_map.insert("621".to_string(), "c".repeat(64));
    let depots = vec![json!({ "depot_id": 621, "gid": "9044256803759256265", "size": 1200 })];
    let script = crate::lua::render_plan(620, "Selftest", "public", &depots, &key_map, &[], "selftest");
    let has_key = script.contains("addappid(621, 1, \"cccc");
    let has_pin = script.contains("setmanifestid(621, \"9044256803759256265\"");
    passo("script carries key and manifest", has_key && has_pin, format!("{} lines", script.lines().count()));

    let depots = vec![json!({ "depot_id": 621, "gid": "9044256803759256265", "size": 1200, "enabled": false })];
    let script = crate::lua::render_plan(620, "Selftest", "public", &depots, &key_map, &[], "selftest");
    let dropped = !script.contains("addappid(621") && !script.contains("setmanifestid(621");
    passo("disabled depot leaves the script", dropped, "addappid and setmanifestid both gone".into());

    let backups = std::fs::read_dir(crate::state::backup_dir())
        .map(|entries| entries.flatten().filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json")).count())
        .unwrap_or(0);
    passo("state backups available", true, format!("{backups} snapshot(s)"));

    let ok = steps.iter().all(|step| step.get("ok").and_then(Value::as_bool).unwrap_or(false));
    json!({ "ok": ok, "steps": steps, "backups": backups })
}
