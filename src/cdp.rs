//! A very small Chrome DevTools Protocol client.
//!
//! Steam's UI is Chromium. With `-cef-enable-debugging` it exposes the CDP
//! endpoint on localhost, which is the only supported way to run our own script
//! inside the client's main window.

use serde_json::{json, Value};

use crate::http;

pub fn version(port: u16) -> Option<Value> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    http::get_json_with(&url, None).ok()
}

/// Open a URL as a new Steam browser window and return its target.
pub fn create_target(port: u16, url: &str) -> Result<Value, String> {
    let body = http::put(&format!("http://127.0.0.1:{port}/json/new?{url}"))
        .map_err(|error| format!("cannot open a Steam window: {error}"))?;
    serde_json::from_slice(&body).map_err(|error| format!("bad target from CDP: {error}"))
}

pub fn targets(port: u16) -> Vec<Value> {
    let url = format!("http://127.0.0.1:{port}/json/list");
    match http::get_json_with(&url, None) {
        Ok(Value::Array(list)) => list,
        _ => Vec::new(),
    }
}

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// One live socket per target. A handshake costs a few milliseconds and the pump
/// asks for state every 400 ms, so reconnecting each time was pure churn.
static SOCKETS: std::sync::Mutex<Option<std::collections::HashMap<String, Socket>>> =
    std::sync::Mutex::new(None);

fn take_socket(ws_url: &str, timeout: std::time::Duration) -> Result<Socket, String> {
    let cached = {
        let mut guard = SOCKETS.lock().map_err(|_| "CDP socket cache is poisoned".to_string())?;
        guard
            .get_or_insert_with(std::collections::HashMap::new)
            .remove(ws_url)
    };
    let mut socket = match cached {
        Some(socket) => socket,
        None => tungstenite::connect(ws_url)
            .map_err(|error| format!("cannot connect to CDP: {error}"))?
            .0,
    };
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(timeout));
    }
    Ok(socket)
}

fn keep_socket(ws_url: &str, socket: Socket) {
    if let Ok(mut guard) = SOCKETS.lock() {
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        if map.len() >= 24 {
            map.clear();
        }
        map.insert(ws_url.to_string(), socket);
    }
}

pub fn evaluate(ws_url: &str, expression: &str, timeout: std::time::Duration) -> Result<Value, String> {
    use tungstenite::Message;

    let mut socket = take_socket(ws_url, timeout)?;
    let request = json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": { "expression": expression, "returnByValue": true, "awaitPromise": true },
    });
    socket
        .send(Message::Text(request.to_string().into()))
        .map_err(|error| format!("cannot send to CDP: {error}"))?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() > deadline {
            return Err("CDP timed out".into());
        }
        let message = match socket.read() {
            Ok(message) => message,
            Err(error) => return Err(format!("CDP read failed: {error}")),
        };
        let text = match message {
            Message::Text(text) => text.to_string(),
            Message::Close(_) => return Err("CDP closed the connection".into()),
            _ => continue,
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else { continue };
        if parsed.get("id").and_then(Value::as_i64) != Some(1) {
            continue;
        }
        if let Some(error) = parsed.get("error") {
            return Err(error.to_string());
        }
        let result = parsed.get("result").cloned().unwrap_or_else(|| json!({}));
        if let Some(details) = result.get("exceptionDetails") {
            let detail = details.get("text").cloned();
            return Err(detail.map(|value| value.to_string()).unwrap_or_else(|| "evaluation threw".into()));
        }
        keep_socket(ws_url, socket);
        return Ok(result.pointer("/result/value").cloned().unwrap_or(Value::Null));
    }
}

/// Send any CDP command and return its result. Used for Page.bringToFront.
pub fn command(ws_url: &str, method: &str, params: Value, timeout: std::time::Duration) -> Result<Value, String> {
    use tungstenite::Message;
    let mut socket = take_socket(ws_url, timeout)?;
    let request = json!({ "id": 1, "method": method, "params": params });
    socket
        .send(Message::Text(request.to_string().into()))
        .map_err(|error| format!("cannot send to CDP: {error}"))?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() > deadline {
            return Err(format!("CDP timed out on {method}"));
        }
        let message = socket.read().map_err(|error| format!("CDP read failed: {error}"))?;
        let Message::Text(text) = message else { continue };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else { continue };
        if parsed.get("id").and_then(Value::as_i64) != Some(1) {
            continue;
        }
        if let Some(error) = parsed.get("error") {
            return Err(error.to_string());
        }
        keep_socket(ws_url, socket);
        return Ok(parsed.get("result").cloned().unwrap_or_else(|| json!({})));
    }
}

/// Evaluate inside a named isolated world of the page. The world is created on the
/// current document, so it shares the DOM with Steam's UI but not its JavaScript
/// globals; a plugin cannot see or overwrite the page's own objects.
pub fn evaluate_isolated(
    ws_url: &str,
    world_name: &str,
    expression: &str,
    timeout: std::time::Duration,
) -> Result<Value, String> {
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;

    let (mut socket, _) = tungstenite::connect(ws_url).map_err(|error| format!("cannot connect to CDP: {error}"))?;
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(timeout));
    }

    let mut next_id = 0i64;
    let mut send = |socket: &mut tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>,
                    method: &str,
                    params: Value|
     -> Result<Value, String> {
        next_id += 1;
        let id = next_id;
        let request = json!({ "id": id, "method": method, "params": params });
        socket
            .send(Message::Text(request.to_string().into()))
            .map_err(|error| format!("cannot send to CDP: {error}"))?;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() > deadline {
                return Err(format!("CDP timed out waiting for {method}"));
            }
            let message = match socket.read() {
                Ok(message) => message,
                Err(error) => return Err(format!("CDP read failed: {error}")),
            };
            let text = match message {
                Message::Text(text) => text.to_string(),
                Message::Close(_) => return Err("CDP closed the connection".into()),
                _ => continue,
            };
            let Ok(parsed) = serde_json::from_str::<Value>(&text) else { continue };
            if parsed.get("id").and_then(Value::as_i64) != Some(id) {
                continue;
            }
            if let Some(error) = parsed.get("error") {
                return Err(error.to_string());
            }
            return Ok(parsed.get("result").cloned().unwrap_or_else(|| json!({})));
        }
    };

    // Page.enable is best-effort: some windows answer it with an error and still
    // accept the rest.
    let _ = send(&mut socket, "Page.enable", json!({}));
    let tree = send(&mut socket, "Page.getFrameTree", json!({}))?;
    let frame = tree
        .pointer("/frameTree/frame/id")
        .and_then(Value::as_str)
        .ok_or("no frame to attach to")?
        .to_string();
    let world = send(
        &mut socket,
        "Page.createIsolatedWorld",
        json!({ "frameId": frame, "worldName": world_name, "grantUniversalAccess": false }),
    )?;
    let context = world
        .get("executionContextId")
        .and_then(Value::as_i64)
        .ok_or("CDP did not return an isolated world")?;
    let result = send(
        &mut socket,
        "Runtime.evaluate",
        json!({ "expression": expression, "contextId": context, "returnByValue": true, "awaitPromise": true }),
    )?;
    if let Some(details) = result.get("exceptionDetails") {
        return Err(details.to_string());
    }
    Ok(result.pointer("/result/value").cloned().unwrap_or(Value::Null))
}