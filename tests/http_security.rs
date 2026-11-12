mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PORT: u16 = 8793;

fn request(raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", PORT)).expect("connect");
    stream.write_all(raw.as_bytes()).expect("write");
    stream.flush().ok();
    let mut text = String::new();
    stream.read_to_string(&mut text).expect("read");
    text
}

fn wait_ready() {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if TcpStream::connect(("127.0.0.1", PORT)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server did not start");
}

fn body_of(response: &str) -> &str {
    response.split_once("\r\n\r\n").map(|(_, body)| body).unwrap_or("")
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
    let mut state = exxa::state::State::default();
    state.settings.hubcap_key = "smm_supersecret_value".into();
    state.settings.ryuu_key = "ryuu_supersecret_value".into();
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

    std::thread::spawn(|| {
        let _ = exxa::server::serve(exxa::server::Config { port: PORT });
    });
    wait_ready();

    let settings = request("GET /api/settings HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n");
    assert!(!settings.contains("smm_supersecret_value"), "hubcap key leaked: {settings}");
    assert!(!settings.contains("ryuu_supersecret_value"), "ryuu key leaked: {settings}");
    assert!(settings.contains("has_hubcap_key"), "presence flag missing");
    assert!(settings.contains("has_ryuu_key"), "presence flag missing");

    let key = request("GET /api/key?depot_id=621 HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n");
    assert!(!key.contains(KEY), "depot key leaked: {key}");
    assert!(key.contains("has_key"));

    let session = request("GET /api/session HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n");
    let token = token(&session);
    assert_eq!(token.len(), 64);

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
