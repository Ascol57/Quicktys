//! Validation of JSON values against a [`Schema`].
//!
//! Every payload crossing the WASM ABI is validated here before the host hands
//! it to an extension (ARCHITECTURE.md §5, usage 2). Validation is structural:
//! it checks the JSON shape, the declared constraints (ranges, lengths, enum
//! membership, required object fields) and recurses into objects/arrays.
//!
//! **Templating is deferred.** A field that opts into `{{ }}` (see
//! [`Schema::is_templatable`]) whose value is a template string is accepted
//! as-is: its concrete type only exists after `quicktys-expr` resolves it, so we
//! cannot type-check it here. A plain (non-template) value is still checked.

use crate::schema::Schema;
use crate::template::TemplateString;
use serde_json::Value;

/// A single validation failure, located by a dotted/indexed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Path to the offending value, e.g. `volume` or `tracks[0].offset`.
    /// Empty for the root value.
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

/// Validate `value` against `schema`. An empty result means the value is valid.
pub fn validate(value: &Value, schema: &Schema) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    check(value, schema, &mut Path::default(), &mut errors);
    errors
}

/// A path being built during recursion (cheap join only when emitting an error).
#[derive(Default)]
struct Path(Vec<String>);

impl Path {
    fn render(&self) -> String {
        let mut out = String::new();
        for seg in &self.0 {
            if seg.starts_with('[') {
                out.push_str(seg);
            } else {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(seg);
            }
        }
        out
    }
}

fn check(value: &Value, schema: &Schema, path: &mut Path, errors: &mut Vec<ValidationError>) {
    // A templatable field holding a `{{ }}` string is checked later, post-eval.
    if schema.is_templatable() {
        if let Value::String(s) = value {
            if TemplateString::parse(s).is_template() {
                return;
            }
        }
    }
    // `null` is treated as "absent"; presence/requiredness is enforced by the
    // enclosing object, so a null leaf is not itself a type error.
    if value.is_null() {
        return;
    }

    match schema {
        Schema::String {
            enum_values,
            min_length,
            max_length,
            ..
        } => {
            let Some(s) = expect_str(value, path, errors) else { return };
            let len = s.chars().count();
            if let Some(min) = min_length {
                if len < *min {
                    err(path, errors, format!("shorter than min length {min}"));
                }
            }
            if let Some(max) = max_length {
                if len > *max {
                    err(path, errors, format!("longer than max length {max}"));
                }
            }
            if let Some(allowed) = enum_values {
                if !allowed.iter().any(|a| a == s) {
                    err(path, errors, format!("not one of {allowed:?}"));
                }
            }
        }
        Schema::Number { min, max, .. } | Schema::Duration { min, max, .. } => {
            if let Some(n) = expect_number(value, path, errors) {
                check_range(n, *min, *max, path, errors);
            }
        }
        Schema::Boolean { .. } => {
            if !value.is_boolean() {
                err(path, errors, type_msg("boolean", value));
            }
        }
        Schema::Color { .. } | Schema::FilePath { .. } | Schema::Resource { .. } => {
            expect_str(value, path, errors);
        }
        Schema::Object { properties, .. } => {
            let Some(map) = value.as_object() else {
                err(path, errors, type_msg("object", value));
                return;
            };
            for (key, prop) in properties {
                let present = map.get(key).filter(|v| !v.is_null());
                match present {
                    Some(v) => {
                        path.0.push(key.clone());
                        check(v, prop, path, errors);
                        path.0.pop();
                    }
                    None if prop.common().required => {
                        path.0.push(key.clone());
                        err(path, errors, "required field is missing".to_string());
                        path.0.pop();
                    }
                    None => {}
                }
            }
        }
        Schema::Array {
            items,
            min_items,
            max_items,
            ..
        } => {
            let Some(arr) = value.as_array() else {
                err(path, errors, type_msg("array", value));
                return;
            };
            if let Some(min) = min_items {
                if arr.len() < *min {
                    err(path, errors, format!("fewer than {min} item(s)"));
                }
            }
            if let Some(max) = max_items {
                if arr.len() > *max {
                    err(path, errors, format!("more than {max} item(s)"));
                }
            }
            for (i, item) in arr.iter().enumerate() {
                path.0.push(format!("[{i}]"));
                check(item, items, path, errors);
                path.0.pop();
            }
        }
        Schema::Range { .. } => {
            let Some(map) = value.as_object() else {
                err(path, errors, type_msg("range object", value));
                return;
            };
            for bound in ["min", "max"] {
                if let Some(v) = map.get(bound).filter(|v| !v.is_null()) {
                    if !v.is_number() {
                        path.0.push(bound.to_string());
                        err(path, errors, type_msg("number", v));
                        path.0.pop();
                    }
                }
            }
        }
        Schema::Toggle { .. } => {
            let is_toggle_kw = value.as_str() == Some("toggle");
            if !value.is_boolean() && !is_toggle_kw {
                err(path, errors, r#"expected boolean or "toggle""#.to_string());
            }
        }
    }
}

fn expect_str<'a>(
    value: &'a Value,
    path: &Path,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a str> {
    match value.as_str() {
        Some(s) => Some(s),
        None => {
            err(path, errors, type_msg("string", value));
            None
        }
    }
}

fn expect_number(value: &Value, path: &Path, errors: &mut Vec<ValidationError>) -> Option<f64> {
    match value.as_f64() {
        Some(n) => Some(n),
        None => {
            err(path, errors, type_msg("number", value));
            None
        }
    }
}

fn check_range(n: f64, min: Option<f64>, max: Option<f64>, path: &Path, errors: &mut Vec<ValidationError>) {
    if let Some(min) = min {
        if n < min {
            err(path, errors, format!("below minimum {min}"));
        }
    }
    if let Some(max) = max {
        if n > max {
            err(path, errors, format!("above maximum {max}"));
        }
    }
}

fn type_msg(expected: &str, got: &Value) -> String {
    format!("expected {expected}, got {}", json_type_name(got))
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn err(path: &Path, errors: &mut Vec<ValidationError>, message: String) {
    errors.push(ValidationError {
        path: path.render(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Common;
    use serde_json::json;

    fn schema(yaml: &str) -> Schema {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn number_range_is_enforced() {
        let s = schema("type: Number\nmin: 0\nmax: 100");
        assert!(validate(&json!(50), &s).is_empty());
        let errs = validate(&json!(150), &s);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("above maximum"));
        assert!(!validate(&json!("nope"), &s).is_empty());
    }

    #[test]
    fn object_requires_and_paths_nested_errors() {
        let s = schema(
            "\
type: Object
properties:
  volume: { type: Number, min: 0, max: 100, required: true }
  label:  { type: String }
",
        );
        // Missing required field.
        let errs = validate(&json!({ "label": "hi" }), &s);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].path, "volume");
        // Nested out-of-range carries the property path.
        let errs = validate(&json!({ "volume": 200 }), &s);
        assert_eq!(errs[0].path, "volume");
    }

    #[test]
    fn array_items_get_indexed_paths() {
        let s = schema("type: Array\nitems: { type: Number, min: 0 }\nmax_items: 3");
        let errs = validate(&json!([1, -5, 2]), &s);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].path, "[1]");
        assert!(!validate(&json!([1, 2, 3, 4]), &s).is_empty());
    }

    #[test]
    fn templatable_field_defers_when_value_is_a_template() {
        let s = Schema::Duration {
            common: Common::default(),
            default: None,
            min: Some(0.0),
            max: Some(10.0),
            template: true,
        };
        // A `{{ }}` value can't be type-checked yet → accepted.
        assert!(validate(&json!("{{ timer.remaining + 5 }}"), &s).is_empty());
        // A concrete value is still range-checked.
        assert!(!validate(&json!(999), &s).is_empty());
    }

    #[test]
    fn string_enum_membership() {
        let s = schema("type: String\nenum: [a, b, c]");
        assert!(validate(&json!("b"), &s).is_empty());
        assert!(!validate(&json!("z"), &s).is_empty());
    }

    #[test]
    fn toggle_accepts_bool_or_keyword() {
        let s = schema("type: Toggle");
        assert!(validate(&json!(true), &s).is_empty());
        assert!(validate(&json!("toggle"), &s).is_empty());
        assert!(!validate(&json!("on"), &s).is_empty());
    }
}
