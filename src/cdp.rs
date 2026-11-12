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

pub fn targets(port: u16) -> Vec<Value> {
    let url = format!("http://127.0.0.1:{port}/json/list");
    match http::get_json_with(&url, None) {
        Ok(Value::Array(list)) => list,
        _ => Vec::new(),
    }
}

pub fn evaluate(ws_url: &str, expression: &str, timeout: std::time::Duration) -> Result<Value, String> {
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;

    let (mut socket, _) = tungstenite::connect(ws_url).map_err(|error| format!("cannot connect to CDP: {error}"))?;
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(timeout));
    }
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
        if result.get("exceptionDetails").is_some() {
            let detail = result.get("exceptionDetails").and_then(|value| value.get("text")).cloned();
            return Err(detail.map(|value| value.to_string()).unwrap_or_else(|| "evaluation threw".into()));
        }
        return Ok(result.pointer("/result/value").cloned().unwrap_or(Value::Null));
    }
}