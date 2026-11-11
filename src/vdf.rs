//! VDF reading through keyvalues-parser, flattened into serde_json values so the
//! rest of the code never fights lifetimes.

use serde_json::{Map, Value};

pub fn parse(text: &str) -> Result<Value, String> {
    let partial = keyvalues_parser::parse(text).map_err(|error| error.to_string())?;
    let vdf = keyvalues_parser::Vdf::from(partial);
    Ok(convert(&vdf.value))
}

fn convert(value: &keyvalues_parser::Value<'_>) -> Value {
    match value {
        keyvalues_parser::Value::Str(text) => Value::String(text.to_string()),
        keyvalues_parser::Value::Obj(obj) => {
            let mut map = Map::new();
            for (key, values) in obj.iter() {
                let first = values.first().map(convert).unwrap_or(Value::Null);
                map.insert(key.to_string(), first);
            }
            Value::Object(map)
        }
    }
}

/// Case-insensitive lookup of a direct key.
pub fn child<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    let object = node.as_object()?;
    if let Some(found) = object.get(key) {
        return Some(found);
    }
    object
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
}

pub fn scalar(node: &Value, key: &str) -> Option<String> {
    match child(node, key)? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub fn scalar_i64(node: &Value, key: &str) -> Option<i64> {
    scalar(node, key).and_then(|text| text.trim().parse::<i64>().ok())
}

/// Every `path` value anywhere in the tree, used for libraryfolders.vdf.
pub fn collect_paths(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if key.eq_ignore_ascii_case("path") {
                    if let Value::String(text) = value {
                        out.push(text.clone());
                    }
                }
                collect_paths(value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_paths(item, out);
            }
        }
        _ => {}
    }
}