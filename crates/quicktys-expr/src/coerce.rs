//! Coercion of an evaluated value into a schema field's type.
//!
//! After an expression is evaluated for a typed field, its result is coerced to
//! the field's declared type (the table in automation-engine.md §1.6). This is
//! lenient on input shape (a stringy number coerces to a number) but always
//! yields a value of the target type — or an error if no sensible coercion
//! exists.

use crate::error::ExprError;
use quicktys_schema::Schema;
use serde_json::Value;

/// Coerce `value` to the type declared by `schema`.
pub fn coerce(value: Value, schema: &Schema) -> Result<Value, ExprError> {
    match schema {
        Schema::String { max_length, .. } => Ok(coerce_string(value, *max_length)),
        Schema::Color { .. } | Schema::FilePath { .. } | Schema::Resource { .. } => {
            Ok(coerce_string(value, None))
        }
        Schema::Number { min, max, .. } | Schema::Duration { min, max, .. } => {
            coerce_number(value, *min, *max)
        }
        Schema::Boolean { .. } => Ok(Value::Bool(truthy(&value))),
        Schema::Toggle { .. } => Ok(coerce_toggle(value)),
        Schema::Range { .. } => coerce_range(value),
        // Composite types are validated structurally, not coerced here.
        Schema::Object { .. } | Schema::Array { .. } => Ok(value),
    }
}

fn coerce_string(value: Value, max_length: Option<usize>) -> Value {
    let s = match value {
        Value::String(s) => s,
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    };
    let s = match max_length {
        Some(max) if s.chars().count() > max => s.chars().take(max).collect(),
        _ => s,
    };
    Value::String(s)
}

fn coerce_number(value: Value, min: Option<f64>, max: Option<f64>) -> Result<Value, ExprError> {
    let mut n = match &value {
        Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| ExprError::Coerce("number out of range".into()))?,
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| ExprError::Coerce(format!("cannot parse {s:?} as a number")))?,
        Value::Bool(b) => {
            return Err(ExprError::Coerce(format!("cannot coerce bool {b} to number")))
        }
        other => {
            return Err(ExprError::Coerce(format!(
                "cannot coerce {} to number",
                type_name(other)
            )))
        }
    };
    if let Some(min) = min {
        n = n.max(min);
    }
    if let Some(max) = max {
        n = n.min(max);
    }
    Ok(number_value(n))
}

/// A toggle is `true`/`false`, or the literal `"toggle"` (flip current state).
fn coerce_toggle(value: Value) -> Value {
    if value.as_str() == Some("toggle") {
        Value::String("toggle".into())
    } else {
        Value::Bool(truthy(&value))
    }
}

fn coerce_range(value: Value) -> Result<Value, ExprError> {
    let Value::Object(obj) = value else {
        return Err(ExprError::Coerce("range must be an object { min, max }".into()));
    };
    let mut out = serde_json::Map::new();
    for bound in ["min", "max"] {
        if let Some(v) = obj.get(bound).filter(|v| !v.is_null()) {
            out.insert(bound.into(), coerce_number(v.clone(), None, None)?);
        }
    }
    Ok(Value::Object(out))
}

/// Boolean coercion (§1.6): only the string `"false"` (and falsey scalars) are
/// false; everything else is true.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::String(s) => s != "false",
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Produce a clean JSON number: integral floats become integers.
fn number_value(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
        Value::Number((n as i64).into())
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(yaml: &str) -> Schema {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn number_clamps_to_range() {
        let s = schema("type: Number\nmin: 0\nmax: 100");
        assert_eq!(coerce(json!(150), &s).unwrap(), json!(100));
        assert_eq!(coerce(json!(-5), &s).unwrap(), json!(0));
        // Stringy numbers coerce.
        assert_eq!(coerce(json!("42"), &s).unwrap(), json!(42));
    }

    #[test]
    fn string_truncates_to_max_length() {
        let s = schema("type: String\nmax_length: 3");
        assert_eq!(coerce(json!("hello"), &s).unwrap(), json!("hel"));
        // Non-strings stringify.
        assert_eq!(coerce(json!(true), &schema("type: String")).unwrap(), json!("true"));
    }

    #[test]
    fn boolean_only_false_string_is_false() {
        let s = schema("type: Boolean");
        assert_eq!(coerce(json!("false"), &s).unwrap(), json!(false));
        assert_eq!(coerce(json!("anything"), &s).unwrap(), json!(true));
        assert_eq!(coerce(json!(0), &s).unwrap(), json!(false));
    }

    #[test]
    fn toggle_keeps_keyword_or_becomes_bool() {
        let s = schema("type: Toggle");
        assert_eq!(coerce(json!("toggle"), &s).unwrap(), json!("toggle"));
        assert_eq!(coerce(json!(true), &s).unwrap(), json!(true));
    }

    #[test]
    fn duration_is_numeric() {
        let s = schema("type: Duration\nmin: 0");
        assert_eq!(coerce(json!(-3), &s).unwrap(), json!(0));
        assert_eq!(coerce(json!(2.5), &s).unwrap(), json!(2.5));
    }
}
