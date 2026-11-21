//! Convert Imp Rust serde JSON (snake_case fields) to Imp TS / spec camelCase.

use serde_json::{Map, Value};

/// Recursively rename Imp structural keys to the TypeScript / Imp-spec camelCase form.
pub fn imp_json_to_ts_camel(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(rename_object(map)),
        Value::Array(items) => Value::Array(items.into_iter().map(imp_json_to_ts_camel).collect()),
        other => other,
    }
}

fn rename_object(map: Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for (key, value) in map {
        let new_key = match key.as_str() {
            "type_params" => "typeParams".to_string(),
            "default_value" => "defaultValue".to_string(),
            "type_args" => "typeArgs".to_string(),
            other => other.to_string(),
        };
        out.insert(new_key, imp_json_to_ts_camel(value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renames_known_imp_fields() {
        let input = json!({
            "id": "sine",
            "type_params": [],
            "inputs": {
                "frequency": {
                    "id": "frequency",
                    "type": { "id": "control" },
                    "default_value": 440.0
                }
            },
            "outputs": {}
        });
        let out = imp_json_to_ts_camel(input);
        assert!(out["typeParams"].is_array());
        assert_eq!(out["inputs"]["frequency"]["defaultValue"], 440.0);
        assert!(out.get("type_params").is_none());
    }
}
