//! Render Vapor Forge Lua scripts: a plan for one app, and the provider that
//! answers manifest-code requests at download time.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::providers;

pub const PROVIDER_FILE: &str = "zz-exxatools-provider.lua";
const PROVIDER_TEMPLATE: &str = include_str!("../assets/provider.lua");

fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_secs())
        .unwrap_or(0);
    format!("{}", now)
}

/// Comment fields come from Steam metadata or a user-supplied name. A newline in
/// one of them would end the comment line and turn the rest into executable Lua,
/// so control characters collapse to spaces and the length is bounded.
fn comment_value(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(120)
        .collect();
    cleaned.trim().to_string()
}

// A pure template: the arguments are the plan fields, and grouping them into a
// struct would only move the same list one line up at every call site.
#[allow(clippy::too_many_arguments)]
pub fn render_plan(
    appid: u64,
    name: &str,
    branch: &str,
    depots: &[Value],
    keys: &BTreeMap<String, String>,
    dlc: &[u64],
    generated: &str,
    source: &str,
) -> String {
    // A depot without a key cannot be decrypted, so pinning it only makes
    // Steam retry a mount it will always fail ("content still encrypted",
    // update canceled). Mount only what has a key; if a key arrives later the
    // depot comes back on the next write.
    let key_for = |depot: &Value| -> Option<String> {
        depot
            .get("depot_id")
            .and_then(value_to_id)
            .and_then(|id| keys.get(&id))
            .and_then(|key| crate::state::valid_key(key))
    };
    let active: Vec<&Value> = depots
        .iter()
        .filter(|depot| {
            depot
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
        .filter(|depot| key_for(depot).is_some())
        .collect();
    let depot_ids: Vec<String> = active
        .iter()
        .filter_map(|depot| depot.get("depot_id").and_then(value_to_id))
        .collect();
    let mut lines: Vec<String> = vec![
        "-- ExxaTools plan".into(),
        format!("-- @appid {appid}"),
        format!("-- @name {}", comment_value(name)),
        format!("-- @branch {}", comment_value(branch)),
        format!("-- @source {}", comment_value(source)),
        format!("-- @depots {}", depot_ids.join(",")),
    ];
    if !generated.is_empty() {
        lines.push(format!("-- generated {generated}"));
    }
    lines.push(String::new());
    lines.push(format!("addappid({appid})"));

    if !active.is_empty() {
        lines.push(String::new());
        lines.push("-- depot decryption keys".into());
        for depot in &active {
            if let (Some(depot_id), Some(key)) =
                (depot.get("depot_id").and_then(value_to_id), key_for(depot))
            {
                lines.push(format!("addappid({depot_id}, 1, {})", quote(&key)));
            }
        }
    }

    let pinned: Vec<&Value> = active
        .iter()
        .copied()
        .filter(|depot| depot.get("gid").and_then(value_to_gid).is_some())
        .collect();
    if !pinned.is_empty() {
        lines.push(String::new());
        lines.push("-- manifest pinning".into());
        for depot in pinned {
            let Some(depot_id) = depot.get("depot_id").and_then(value_to_id) else {
                continue;
            };
            let Some(gid) = depot.get("gid").and_then(value_to_gid) else {
                continue;
            };
            match depot.get("size").and_then(Value::as_u64) {
                Some(size) if size > 0 => lines.push(format!(
                    "setmanifestid({depot_id}, {}, {size})",
                    quote(&gid)
                )),
                _ => lines.push(format!("setmanifestid({depot_id}, {})", quote(&gid))),
            }
        }
    }

    // A DLC id that is also a keyless depot would ask Steam for content it
    // cannot decrypt, so it is left out until a key shows up. Disabled depots
    // count here too: their DLC app id must not be granted on its own.
    let keyless_depots: std::collections::BTreeSet<String> = depots
        .iter()
        .filter(|depot| key_for(depot).is_none())
        .filter_map(|depot| depot.get("depot_id").and_then(value_to_id))
        .collect();
    let mounted_dlc: Vec<u64> = dlc
        .iter()
        .copied()
        .filter(|dlc_id| !keyless_depots.contains(&dlc_id.to_string()))
        .collect();
    if !mounted_dlc.is_empty() {
        lines.push(String::new());
        lines.push("-- DLC".into());
        for dlc_id in mounted_dlc {
            lines.push(format!("addappid({dlc_id})"));
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Depot and app ids arrive as numbers from appinfo and as strings from state.
/// Ids reach the script as bare digits; anything else would produce a call the
/// runtime cannot parse, so it is dropped instead of quoted and hoped for.
pub fn value_to_id(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => number.as_u64().map(|id| id.to_string()),
        Value::String(text) => {
            let trimmed = text.trim();
            let digits = !trimmed.is_empty()
                && trimmed.len() <= 20
                && trimmed.chars().all(|ch| ch.is_ascii_digit());
            digits.then(|| trimmed.to_string())
        }
        _ => None,
    }
}

/// Manifest ids are decimal and fit in u64; a quoted string here would pin a
/// build that cannot exist.
pub fn value_to_gid(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => number.as_u64().map(|gid| gid.to_string()),
        Value::String(text) => {
            let trimmed = text.trim();
            let digits = !trimmed.is_empty()
                && trimmed.len() <= 20
                && trimmed.chars().all(|ch| ch.is_ascii_digit());
            digits.then(|| trimmed.to_string())
        }
        _ => None,
    }
}

/// Renders a saved plan record. One builder for the status panel, the installed
/// script and the verify pass, so all three show the same text.
pub fn render_record(record: &Value, keys: &BTreeMap<String, String>) -> String {
    let appid = record.get("appid").and_then(Value::as_u64).unwrap_or(0);
    let name = record.get("name").and_then(Value::as_str).unwrap_or("");
    let branch = record
        .get("branch")
        .and_then(Value::as_str)
        .unwrap_or("public");
    let depots = record
        .get("depots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let dlc: Vec<u64> = record
        .get("dlc")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let generated = record.get("updated").and_then(Value::as_str).unwrap_or("");
    let source = record
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("auto");
    render_plan(appid, name, branch, &depots, keys, &dlc, generated, source)
}

/// Render the provider chain from `order` and write it where Vapor Forge
/// reads it. One place so the worker and the API stay in sync.
pub fn republish_provider(order: &[Value]) -> Result<std::path::PathBuf, String> {
    let text = render_provider(order)?;
    let check = crate::engine::check_script(&text, PROVIDER_FILE);
    if !check.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(format!(
            "provider script failed to parse: {}",
            check.get("output").and_then(Value::as_str).unwrap_or("")
        ));
    }
    crate::vfsys::write_provider(&text)
}

/// The ordered provider chain as JSON values, ready for republish. A chain
/// entry is either a builtin name or a JSON object describing a custom
/// mirror; objects are parsed and anything unparseable stays a name so a
/// typo degrades to "unknown builtin" instead of dropping the entry.
pub fn provider_order(state: &crate::state::State) -> Vec<Value> {
    state
        .settings
        .manifest_providers
        .iter()
        .map(|entry| {
            // Only `{...}` entries are custom mirrors. A builtin id can be all
            // digits ("20770407"), and parsing it as JSON would turn it into a
            // number that resolve() silently drops.
            if entry.trim_start().starts_with('{') {
                serde_json::from_str::<Value>(entry).unwrap_or_else(|_| json!(entry))
            } else {
                json!(entry)
            }
        })
        .collect()
}

pub fn render_provider(order: &[Value]) -> Result<String, String> {
    let mut rows: Vec<String> = Vec::new();
    for entry in order {
        let Some(spec) = providers::resolve(entry) else {
            continue;
        };
        let mut parts = vec![
            format!("name = {}", quote(&spec.name)),
            format!("url = {}", quote(&spec.url)),
            format!("kind = {}", quote(&spec.format)),
        ];
        if let Some(field) = &spec.json_field {
            parts.push(format!("field = {}", quote(field)));
        }
        if let Some(agent) = &spec.user_agent {
            parts.push(format!("agent = {}", quote(agent)));
        }
        rows.push(format!("  {{ {} }},", parts.join(", ")));
    }
    if rows.is_empty() {
        return Err("no usable manifest provider configured".into());
    }
    let block = format!("{{\n{}\n}}", rows.join("\n"));
    Ok(PROVIDER_TEMPLATE
        .replace("__STAMP__", &stamp())
        .replace("__SOURCES__", &block))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn keys() -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert("621".to_string(), "ab".repeat(32));
        map.insert("624".to_string(), "cd".repeat(32));
        map
    }

    #[test]
    fn renders_keys_pins_and_dlc() {
        let depots = vec![
            json!({ "depot_id": 621, "gid": "111", "size": 1200 }),
            json!({ "depot_id": 624, "gid": "222" }),
            json!({ "depot_id": 625 }),
            json!({ "depot_id": 626, "gid": "333" }),
        ];
        let text = render_plan(
            620,
            "Portal 2",
            "public",
            &depots,
            &keys(),
            &[650],
            "test",
            "auto",
        );
        assert!(text.contains("addappid(620)"));
        assert!(text.contains("addappid(621, 1, \""));
        assert!(text.contains("setmanifestid(621, \"111\", 1200)"));
        assert!(text.contains("setmanifestid(624, \"222\")"));
        assert!(text.contains("addappid(650)"));
        assert!(!text.contains("setmanifestid(625"));
        // 626 has a pin but no key: Steam could never decrypt it, so it must
        // not reach the script at all.
        assert!(!text.contains("setmanifestid(626"));
        assert_eq!(lint_hint(&text), 0);
    }

    fn lint_hint(text: &str) -> usize {
        text.lines()
            .filter(|line| line.starts_with("setmanifestid"))
            .count()
            - 2
    }

    #[test]
    fn escapes_quotes_in_names() {
        let text = render_plan(
            1,
            "Some \"Game\"",
            "public",
            &[],
            &BTreeMap::new(),
            &[],
            "test",
            "auto",
        );
        assert!(text.contains("-- @name Some \"Game\""));
    }

    #[test]
    fn provider_rows_are_lua() {
        let order = vec![json!("steamrun")];
        let text = render_provider(&order).unwrap();
        assert!(text.contains("kind = \"json\""));
        assert!(text.contains("field = \"content\""));
        assert!(!text.contains("__SOURCES__"));
    }

    #[test]
    fn numeric_builtin_ids_survive_provider_order() {
        let mut state = crate::state::State::default();
        state.settings.manifest_providers = vec![
            "steamrun".into(),
            "20770407".into(),
            "{\"name\":\"mirror\",\"url\":\"https://m.example/{gid}\"}".into(),
        ];
        let order = provider_order(&state);
        assert_eq!(order[1], json!("20770407"));
        assert!(order[2].is_object());
        let text = render_provider(&order).unwrap();
        assert!(text.contains("name = \"20770407\""));
        assert!(text.contains("name = \"mirror\""));
    }

    #[test]
    fn disabled_depot_disappears() {
        let depots = vec![
            json!({ "depot_id": 621, "gid": "9044256803759256265", "size": 1200, "enabled": false }),
        ];
        let text = render_plan(
            620,
            "Portal 2",
            "public",
            &depots,
            &keys(),
            &[],
            "test",
            "auto",
        );
        assert!(!text.contains("addappid(621"));
        assert!(!text.contains("setmanifestid(621"));
        assert!(text.contains("addappid(620)"));
    }

    #[test]
    fn enabled_depot_stays() {
        let depots =
            vec![json!({ "depot_id": 621, "gid": "9044256803759256265", "enabled": true })];
        let text = render_plan(
            620,
            "Portal 2",
            "public",
            &depots,
            &keys(),
            &[],
            "test",
            "auto",
        );
        assert!(text.contains("addappid(621, 1, \""));
        assert!(text.contains("setmanifestid(621"));
    }
}

#[cfg(test)]
mod comment_tests {
    use super::render_plan;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn render(name: &str) -> String {
        let depots = vec![json!({ "depot_id": 621, "gid": "9044256803759256265" })];
        render_plan(
            620,
            name,
            "public",
            &depots,
            &BTreeMap::new(),
            &[],
            "test",
            "selftest",
        )
    }

    #[test]
    fn newline_in_name_cannot_inject_lua() {
        let script = render("Evil\naddappid(999)");
        assert!(
            !script.lines().any(|line| line.trim() == "addappid(999)"),
            "injected line survived: {script}"
        );
        assert!(script.contains("-- @name Evil addappid(999)"));
    }

    #[test]
    fn carriage_return_and_tabs_are_flattened() {
        let script = render("A\r\nB\tC");
        let name_line = script
            .lines()
            .find(|line| line.starts_with("-- @name"))
            .unwrap_or("");
        assert_eq!(name_line, "-- @name A  B C");
    }

    #[test]
    fn long_names_are_bounded() {
        let script = render(&"x".repeat(400));
        let name_line = script
            .lines()
            .find(|line| line.starts_with("-- @name"))
            .unwrap_or("");
        assert!(name_line.len() <= "-- @name ".len() + 120);
    }
}

#[cfg(test)]
mod id_tests {
    use super::{render_plan, value_to_gid, value_to_id};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn ids_must_be_digits() {
        assert_eq!(value_to_id(&json!(621)), Some("621".into()));
        assert_eq!(value_to_id(&json!("621")), Some("621".into()));
        assert_eq!(value_to_id(&json!("621abc")), None);
        assert_eq!(value_to_id(&json!("")), None);
        assert_eq!(value_to_id(&json!(-5)), None);
    }

    #[test]
    fn gids_must_be_decimal() {
        assert_eq!(
            value_to_gid(&json!("9044256803759256265")),
            Some("9044256803759256265".into())
        );
        assert_eq!(value_to_gid(&json!("42")), Some("42".into()));
        assert_eq!(value_to_gid(&json!("42; os.execute('x')")), None);
        assert_eq!(value_to_gid(&json!("")), None);
    }

    #[test]
    fn non_numeric_depot_is_not_written() {
        let depots = vec![json!({ "depot_id": "62x1", "gid": "123" })];
        let script = render_plan(
            620,
            "Test",
            "public",
            &depots,
            &BTreeMap::new(),
            &[],
            "test",
            "selftest",
        );
        assert!(!script.contains("addappid(62x1"));
        assert!(!script.contains("setmanifestid(62x1"));
    }

    #[test]
    fn invalid_key_is_not_written() {
        let depots = vec![json!({ "depot_id": 621, "gid": "123" })];
        let mut keys = BTreeMap::new();
        keys.insert("621".to_string(), "not-a-key".to_string());
        let script = render_plan(
            620,
            "Test",
            "public",
            &depots,
            &keys,
            &[],
            "test",
            "selftest",
        );
        assert!(!script.contains("not-a-key"));
        assert!(!script.contains("addappid(621, 1"));
    }
}

#[cfg(test)]
mod stable_tests {
    use super::render_plan;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn render(generated: &str) -> String {
        let depots = vec![json!({ "depot_id": 621, "gid": "9044256803759256265" })];
        render_plan(
            620,
            "Portal 2",
            "public",
            &depots,
            &BTreeMap::new(),
            &[],
            generated,
            "auto",
        )
    }

    #[test]
    fn same_plan_renders_the_same_text() {
        assert_eq!(render("1794480536"), render("1794480536"));
    }

    #[test]
    fn stored_stamp_survives_a_rerender() {
        let first = render("1794480536");
        let second = render("1794480536");
        assert!(first.contains("-- generated 1794480536"));
        assert_eq!(first, second);
    }

    #[test]
    fn missing_stamp_omits_the_line() {
        assert!(!render("").contains("-- generated"));
    }
}

#[cfg(test)]
mod record_tests {
    use super::{render_plan, render_record};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn record() -> serde_json::Value {
        json!({
            "appid": 620,
            "name": "Portal 2",
            "branch": "public",
            "source": "hubcap",
            "updated": "1794480536",
            "depots": [{"depot_id": 621, "gid": "9044256803759256265", "size": 1200, "enabled": true}],
            "dlc": [323180],
        })
    }

    #[test]
    fn record_render_is_stable_and_matches_the_plan_builder() {
        let keys = BTreeMap::new();
        let first = render_record(&record(), &keys);
        let second = render_record(&record(), &keys);
        assert_eq!(first, second);
        let direct = render_plan(
            620,
            "Portal 2",
            "public",
            record()["depots"].as_array().unwrap(),
            &keys,
            &[323180],
            "1794480536",
            "hubcap",
        );
        assert_eq!(first, direct);
        assert!(first.contains("-- @source hubcap"));
        assert!(first.contains("-- generated 1794480536"));
    }

    #[test]
    fn disabled_depot_is_absent_from_the_record_render() {
        let mut value = record();
        value["depots"] = json!([
            {"depot_id": 621, "gid": "1", "enabled": false},
            {"depot_id": 622, "gid": "2", "enabled": true}
        ]);
        let mut keys = BTreeMap::new();
        keys.insert("621".to_string(), "ab".repeat(32));
        keys.insert("622".to_string(), "cd".repeat(32));
        let text = render_record(&value, &keys);
        assert!(
            !text.contains("setmanifestid(621"),
            "disabled depot was pinned: {text}"
        );
        assert!(text.contains("setmanifestid(622"));
    }
}
