//! Turns log lines into named problems with a repair attached.
//!
//! Steam and Vapor Forge both fail with the same handful of messages, and each
//! one has exactly one useful repair. The matcher is deliberately literal: a
//! line that does not carry a known phrase produces nothing, because a wrong
//! guess teaches people to ignore the panel.

use serde_json::{json, Value};

struct Rule {
    id: &'static str,
    title: &'static str,
    detail: &'static str,
    action: &'static str,
    needles: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        id: "no-internet",
        title: "Steam reports no internet connection",
        detail: "The licence for this app is missing, not the network: rewrite the keys and restart Steam.",
        action: "keys",
        needles: &["no internet connection", "no connection to steam"],
    },
    Rule {
        id: "update-required",
        title: "Steam asks for an update that never finishes",
        detail: "The appmanifest is stuck in an update state; repair its flags with Steam closed.",
        action: "manifest",
        needles: &["update required", "app state is 0x", "update queued"],
    },
    Rule {
        id: "still-encrypted",
        title: "Downloaded content is still encrypted",
        detail: "A depot key is missing or wrong for this build; fetch keys again.",
        action: "keys",
        needles: &["content still encrypted", "still encrypted", "depot key"],
    },
    Rule {
        id: "disk-write",
        title: "Steam cannot write the download",
        detail: "Check free space on the library drive and the folder permissions.",
        action: "",
        needles: &["disk write error", "failed to write", "no space left"],
    },
    Rule {
        id: "no-license",
        title: "Vapor Forge did not find a licence",
        detail: "The engine ignored the app: refresh keys, then restart Steam so it reloads scripts.",
        action: "keys",
        needles: &["no license", "license missing", "not licensed", "license not found"],
    },
    Rule {
        id: "manifest-missing",
        title: "Manifest is missing for a depot",
        detail: "The plan has a depot without a manifest id; pin one or fetch the plan again.",
        action: "manifest",
        needles: &["manifest not found", "missing manifest", "no manifest"],
    },
];

/// Matches the rules against raw log text, one incident per rule, first line wins.
pub fn scan(text: &str) -> Vec<Value> {
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    for rule in RULES {
        let mut hit: Option<&str> = None;
        for needle in rule.needles {
            if let Some(index) = lower.find(needle) {
                // Pull the original line back out for the excerpt.
                let start = lower[..index].rfind('\n').map(|value| value + 1).unwrap_or(0);
                let end = lower[index..].find('\n').map(|value| index + value).unwrap_or(lower.len());
                hit = Some(text[start..end].trim());
                break;
            }
        }
        if let Some(line) = hit {
            out.push(json!({
                "id": rule.id,
                "title": rule.title,
                "detail": rule.detail,
                "action": if rule.action.is_empty() { Value::Null } else { json!(rule.action) },
                "line": line.chars().take(200).collect::<String>(),
            }));
        }
    }
    out
}

/// Engine-specific signals that need arithmetic rather than a substring: the
/// pattern summary says how many hooks found their target, and repeated
/// "pattern match failed" lines mean the client build moved under the engine.
fn engine_incidents(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut short: Vec<String> = Vec::new();
    for line in text.lines() {
        if !line.contains("pattern summary") {
            continue;
        }
        let field = |name: &str| -> Option<i64> {
            let start = line.find(&format!("{name}="))? + name.len() + 1;
            let rest = &line[start..];
            let end = rest.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(rest.len());
            rest[..end].parse().ok()
        };
        let found = field("found").unwrap_or(0);
        let total = field("total").unwrap_or(0);
        let missing = field("missing").unwrap_or(0);
        if missing > 0 || (total > 0 && found < total) {
            let module = line
                .split("module=")
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .unwrap_or("client");
            short.push(format!("{module}: {found}/{total}"));
        }
    }
    if !short.is_empty() {
        out.push(json!({
            "id": "engine-patterns",
            "title": "Vapor Forge could not patch the client",
            "detail": "The engine's signatures do not match this Steam build; install the current engine release.",
            "action": "engine",
            "line": short.join(", "),
        }));
    }
    let failures = text.matches("pattern match failed").count();
    if failures >= 3 {
        out.push(json!({
            "id": "engine-hooks",
            "title": "Vapor Forge hooks keep missing",
            "detail": format!("{failures} hook patterns did not match; the client build probably changed."),
            "action": "engine",
            "line": format!("{failures} pattern match failures in this log"),
        }));
    }
    out
}

/// Full analysis: literal rules plus the arithmetic engine checks.
pub fn analyse(text: &str) -> Vec<Value> {
    let mut out = scan(text);
    for incident in engine_incidents(text) {
        let id = incident.get("id").cloned();
        if !out.iter().any(|seen| seen.get("id") == id.as_ref()) {
            out.push(incident);
        }
    }
    out
}

/// Scans the Steam console log and the engine log, newest lines first.
pub fn from_files() -> Value {
    let mut sources: Vec<Value> = Vec::new();
    let mut incidents: Vec<Value> = Vec::new();
    for path in [crate::paths::steam_log()] {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let tail: String = {
            let lines: Vec<&str> = text.lines().rev().take(4000).collect();
            lines.into_iter().rev().collect::<Vec<&str>>().join("\n")
        };
        let found = analyse(&tail);
        sources.push(json!({
            "path": path.display().to_string(),
            "bytes": text.len(),
            "incidents": found.len(),
        }));
        for incident in found {
            if !incidents.iter().any(|seen| seen.get("id") == incident.get("id")) {
                incidents.push(incident);
            }
        }
    }
    json!({ "ok": incidents.is_empty(), "incidents": incidents, "sources": sources })
}

#[cfg(test)]
mod tests {
    use super::scan;

    fn ids(text: &str) -> Vec<String> {
        scan(text).iter().filter_map(|entry| entry.get("id").and_then(|value| value.as_str()).map(str::to_string)).collect()
    }

    #[test]
    fn matches_no_internet() {
        assert_eq!(ids("Error: No internet connection\n"), vec!["no-internet"]);
    }

    #[test]
    fn matches_update_required() {
        assert_eq!(ids("App state is 0x6 after update required"), vec!["update-required"]);
    }

    #[test]
    fn matches_encrypted_content() {
        assert_eq!(ids("Mounting depot 621: Content still encrypted"), vec!["still-encrypted"]);
    }

    #[test]
    fn matches_disk_write() {
        assert_eq!(ids("Disk write error while writing chunk"), vec!["disk-write"]);
    }

    #[test]
    fn matches_missing_license() {
        assert_eq!(ids("[vapor-forge] no license for app 620"), vec!["no-license"]);
    }

    #[test]
    fn every_rule_has_a_fixture() {
        let fixtures = [
            "No internet connection",
            "update required",
            "content still encrypted",
            "disk write error",
            "no license",
            "manifest not found",
        ];
        for fixture in fixtures {
            assert!(!ids(fixture).is_empty(), "no rule matched {fixture:?}");
        }
    }

    #[test]
    fn garbage_and_empty_stay_quiet() {
        assert!(ids("").is_empty());
        assert!(ids("\u{0}\u{1}\u{2} random bytes \u{ff}").is_empty());
        assert!(ids("downloading 42% of depot 621").is_empty());
    }

    #[test]
    fn healthy_pattern_summary_stays_quiet() {
        let text = "[vapor-forge][INFO] Diagnostics: pattern summary module=steamclient.so found=30 missing=0 total=30";
        assert!(super::engine_incidents(text).is_empty());
    }

    #[test]
    fn missing_patterns_raise_an_engine_incident() {
        let text = "[vapor-forge][INFO] Diagnostics: pattern summary module=steamui.so found=6 missing=3 total=9";
        let found = super::engine_incidents(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["id"], "engine-patterns");
        assert!(found[0]["line"].as_str().unwrap_or("").contains("steamui.so: 6/9"));
    }

    #[test]
    fn repeated_hook_failures_raise_an_incident() {
        let text = "pattern match failed\npattern match failed\npattern match failed";
        let found = super::engine_incidents(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["id"], "engine-hooks");
    }

    #[test]
    fn two_hook_failures_are_noise() {
        assert!(super::engine_incidents("pattern match failed\npattern match failed").is_empty());
    }

    #[test]
    fn one_incident_per_rule() {
        let text = "no internet connection\nno internet connection again";
        assert_eq!(ids(text).len(), 1);
    }
}