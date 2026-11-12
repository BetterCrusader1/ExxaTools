//! A support bundle people can paste into a chat: what the app sees, what each
//! plan reports, and the end of the Steam log with every key masked.

use serde_json::{json, Value};

const KEY_LEN: usize = 64;

fn redact_token(token: &str) -> String {
    if token.len() == KEY_LEN && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        format!("{}…{}", &token[..6], &token[KEY_LEN - 4..])
    } else {
        token.to_string()
    }
}

/// Masks any 64-character hex token, keeping six leading and four trailing
/// characters so a person can still tell two keys apart.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            token.push(ch);
        } else {
            out.push_str(&redact_token(&token));
            token.clear();
            out.push(ch);
        }
    }
    out.push_str(&redact_token(&token));
    out
}

pub fn build() -> Value {
    let state = crate::state::load().unwrap_or_default();
    let plans: Vec<Value> = state
        .apps
        .values()
        .filter_map(|record| {
            let appid = record.get("appid").and_then(Value::as_u64)?;
            Some(json!({
                "appid": appid,
                "name": record.get("name").cloned().unwrap_or(Value::Null),
                "verify": crate::verify::run(appid),
            }))
        })
        .collect();
    let log = std::fs::read_to_string(crate::paths::steam_log()).unwrap_or_default();
    let tail: Vec<&str> = log.lines().rev().take(200).collect::<Vec<_>>().into_iter().rev().collect();

    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "generated": crate::vfsys::stamp(),
        "steam": {
            "root": crate::steamctl::root().map(|path| path.display().to_string()),
            "running": crate::steamctl::is_running(),
        },
        "engine": crate::engine::detect(),
        "keys": state.depot_keys.len(),
        "plans": plans,
        "doctor": crate::doctor::run(),
        "diagnose": crate::diagnose::from_files(),
        "log": redact(&tail.join("\n")),
    })
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn a_key_keeps_only_its_edges() {
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let text = format!("depot 621 key {key} loaded");
        let masked = redact(&text);
        assert!(!masked.contains(key));
        assert!(masked.contains("012345…cdef"), "unexpected mask: {masked}");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        assert_eq!(redact("Portal 2 - 620"), "Portal 2 - 620");
    }
}
