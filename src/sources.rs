//! Named manifest sources: where request codes come from for a plan.

use serde_json::{json, Value};

use crate::state::Settings;

pub fn builtin() -> Vec<Value> {
    let pairs = [
        ("auto", "Auto (all providers, in order)", vec!["steamrun", "20770407", "wudrm", "manifestdex", "opensteamtool"]),
        ("steamrun", "steam.run", vec!["steamrun"]),
        ("20770407", "20770407.xyz", vec!["20770407"]),
        ("wudrm", "Wudrm", vec!["wudrm"]),
        ("manifestdex", "ManifestDeX", vec!["manifestdex"]),
        ("opensteamtool", "OpenSteamTool", vec!["opensteamtool"]),
    ];
    pairs
        .into_iter()
        .map(|(name, label, providers)| {
            json!({ "name": name, "label": label, "providers": providers, "key_source_url": "" })
        })
        .collect()
}

pub fn catalog(settings: &Settings) -> Vec<Value> {
    let mut out = builtin();
    for (name, spec) in &settings.sources {
        let Some(object) = spec.as_object() else { continue };
        let providers: Vec<Value> = object
            .get("providers")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter(|value| value.is_string() || value.is_object()).cloned().collect())
            .unwrap_or_default();
        if providers.is_empty() {
            continue;
        }
        out.push(json!({
            "name": name,
            "label": object.get("label").and_then(Value::as_str).unwrap_or(name),
            "providers": providers,
            "key_source_url": object.get("key_source_url").and_then(Value::as_str).unwrap_or(""),
        }));
    }
    out
}
