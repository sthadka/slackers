use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use serde_json::Value;

static PRETTY: AtomicBool = AtomicBool::new(false);
static QUIET: AtomicBool = AtomicBool::new(false);

pub fn set_pretty(val: bool) {
    PRETTY.store(val, Ordering::Relaxed);
}

pub fn is_pretty() -> bool {
    PRETTY.load(Ordering::Relaxed)
}

pub fn set_quiet(val: bool) {
    QUIET.store(val, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

pub fn serialize_json(value: &Value) -> String {
    if is_pretty() {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    } else {
        serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Recursively prune empty values from a JSON value
///
/// Removes:
/// - null values
/// - empty strings
/// - empty arrays
/// - empty objects
///
/// This produces more compact, token-efficient JSON output for AI agents.
pub fn prune_empty(value: Value) -> Value {
    match value {
        Value::Null => Value::Null, // Keep null at top level
        Value::Bool(b) => Value::Bool(b),
        Value::Number(n) => Value::Number(n),
        Value::String(s) => {
            if s.is_empty() {
                Value::Null
            } else {
                Value::String(s)
            }
        }
        Value::Array(arr) => {
            let pruned: Vec<Value> = arr
                .into_iter()
                .map(prune_empty)
                .filter(|v| !is_empty_value(v))
                .collect();

            if pruned.is_empty() {
                Value::Null
            } else {
                Value::Array(pruned)
            }
        }
        Value::Object(obj) => {
            let pruned: serde_json::Map<String, Value> = obj
                .into_iter()
                .map(|(k, v)| (k, prune_empty(v)))
                .filter(|(_, v)| !is_empty_value(v))
                .collect();

            if pruned.is_empty() {
                Value::Null
            } else {
                Value::Object(pruned)
            }
        }
    }
}

/// Check if a value is considered "empty" and should be pruned
fn is_empty_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(arr) => arr.is_empty(),
        Value::Object(obj) => obj.is_empty(),
        _ => false,
    }
}

/// Convert a value to JSON with empty fields pruned.
///
/// This is the primary output format for all CLI commands.
/// Produces compact single-line JSON by default, or 2-space indented
/// JSON when `--pretty` is passed.
pub fn to_json_output<T: Serialize>(value: &T) -> String {
    let json_value = serde_json::to_value(value).unwrap_or(Value::Null);
    let pruned = prune_empty(json_value);
    serialize_json(&pruned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prune_empty_strings() {
        let input = json!({
            "a": "value",
            "b": "",
            "c": "another"
        });

        let result = prune_empty(input);
        assert_eq!(
            result,
            json!({
                "a": "value",
                "c": "another"
            })
        );
    }

    #[test]
    fn test_prune_empty_arrays() {
        let input = json!({
            "a": [1, 2, 3],
            "b": [],
            "c": [null, "", []],
            "d": ["valid"]
        });

        let result = prune_empty(input);
        assert_eq!(
            result,
            json!({
                "a": [1, 2, 3],
                "d": ["valid"]
            })
        );
    }

    #[test]
    fn test_prune_empty_objects() {
        let input = json!({
            "a": { "x": 1 },
            "b": {},
            "c": { "y": null, "z": "" },
            "d": { "valid": "value" }
        });

        let result = prune_empty(input);
        assert_eq!(
            result,
            json!({
                "a": { "x": 1 },
                "d": { "valid": "value" }
            })
        );
    }

    #[test]
    fn test_prune_nested() {
        let input = json!({
            "user": {
                "id": "U123",
                "name": "test",
                "email": "",
                "profile": {
                    "title": "",
                    "phone": null
                }
            },
            "empty_field": null,
            "another_empty": []
        });

        let result = prune_empty(input);
        assert_eq!(
            result,
            json!({
                "user": {
                    "id": "U123",
                    "name": "test"
                }
            })
        );
    }

    #[test]
    fn test_to_json_output() {
        #[derive(Serialize)]
        struct TestStruct {
            name: String,
            value: Option<String>,
            count: u32,
        }

        let data = TestStruct {
            name: "test".to_string(),
            value: None,
            count: 42,
        };

        let output = to_json_output(&data);
        assert!(output.contains("\"name\":\"test\""));
        assert!(output.contains("\"count\":42"));
        assert!(!output.contains("value")); // null field should be pruned
    }
}
