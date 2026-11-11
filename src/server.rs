//! A loopback HTTP server: the JSON API and the web UI, one binary.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::appmanifest;
use crate::engine;
use crate::lua;
use crate::paths;
use crate::state;
use crate::steam;

pub struct Config {
    pub port: u16,
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
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        if status == 200 { "OK" } else { "Error" },
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

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("").to_string();
            let value = parts.next().unwrap_or("").replace('+', " ");
            (key, value)
        })
        .collect()
}

fn get<'a>(query: &'a [(String, String)], key: &str) -> Option<&'a str> {
    query.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
}

fn api(path: &str, query: &[(String, String)]) -> Result<Value, (u16, String)> {
    let st = state::load().map_err(|error| (500, error))?;
    match path {
        "/api/status" => Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "steam_root": crate::steamctl::root().map(|value| value.display().to_string()),
            "steam_running": crate::steamctl::is_running(),
            "engine": engine::detect(),
            "keys": st.depot_keys.len(),
            "apps": st.apps.len(),
        })),
        "/api/engine" => Ok(engine::detect()),
        "/api/discover" => {
            let limit = get(query, "limit").and_then(|value| value.parse().ok()).unwrap_or(18);
            let cc = get(query, "cc").unwrap_or(&st.settings.country).to_string();
            Ok(json!({ "items": steam::discover(&cc, limit) }))
        }
        "/api/search" => {
            let term = get(query, "q").unwrap_or("").to_string();
            let items = steam::search(&term, &st.settings.country).map_err(|error| (502, error))?;
            Ok(json!({ "items": items }))
        }
        "/api/app" => {
            let appid = get(query, "appid").and_then(|value| value.parse().ok()).ok_or((400, "appid is required".into()))?;
            steam::catalog(appid).map_err(|error| (502, error))
        }
        "/api/appmanifest" => {
            let appid = get(query, "appid").and_then(|value| value.parse().ok()).ok_or((400, "appid is required".into()))?;
            Ok(appmanifest::report(appid, None))
        }
        "/api/log" => {
            let lines = get(query, "lines").and_then(|value| value.parse().ok()).unwrap_or(200);
            Ok(engine::log(lines))
        }
        "/api/provider" => {
            let order: Vec<Value> = st.settings.manifest_providers.iter().map(|name| json!(name)).collect();
            let text = lua::render_provider(&order).map_err(|error| (500, error))?;
            Ok(json!({ "lua": text }))
        }
        _ => Err((404, format!("unknown endpoint {path}"))),
    }
}

fn static_file(relative: &str) -> Option<PathBuf> {
    let cleaned = relative.trim_start_matches('/');
    if cleaned.contains("..") {
        return None;
    }
    let candidate = if cleaned.is_empty() { paths::web_dir().join("index.html") } else { paths::web_dir().join(cleaned) };
    candidate.is_file().then_some(candidate)
}

fn handle(mut stream: TcpStream) {
    let reader_stream = match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    };
    let mut reader = BufReader::new(reader_stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let (raw_path, query) = match target.split_once('?') {
        Some((path, query)) => (path, query_pairs(query)),
        None => (target, Vec::new()),
    };

    if method != "GET" {
        respond_json(&mut stream, 405, &json!({ "error": "only GET is supported here yet" }));
        return;
    }

    if raw_path.starts_with("/api/") {
        match api(raw_path, &query) {
            Ok(value) => respond_json(&mut stream, 200, &value),
            Err((status, message)) => respond_json(&mut stream, status, &json!({ "error": message })),
        }
        return;
    }

    match static_file(raw_path) {
        Some(file) => match std::fs::read(&file) {
            Ok(body) => respond(&mut stream, 200, mime_for(&file), &body),
            Err(error) => respond_json(&mut stream, 500, &json!({ "error": error.to_string() })),
        },
        None if raw_path == "/" => {
            let body = std::fs::read(paths::web_dir().join("index.html")).unwrap_or_default();
            respond(&mut stream, 200, "text/html; charset=utf-8", &body);
        }
        None => respond_json(&mut stream, 404, &json!({ "error": "not found" })),
    }
}

pub fn serve(config: Config) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", config.port))
        .map_err(|error| format!("cannot bind 127.0.0.1:{}: {error}", config.port))?;
    eprintln!("exxa {} on http://127.0.0.1:{}/", env!("CARGO_PKG_VERSION"), config.port);
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