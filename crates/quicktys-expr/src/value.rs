//! Conversions between `serde_json::Value` (the host/ABI value model) and
//! `rhai::Dynamic` (what the engine evaluates over).
//!
//! The expression context is materialised as JSON on the host side; we convert
//! it to Rhai values to build the evaluation scope, then convert the result
//! back to JSON for coercion and the rest of the pipeline.

use crate::error::ExprError;
use rhai::Dynamic;
use serde_json::Value;

/// Convert a JSON value into a Rhai `Dynamic`.
pub fn json_to_dynamic(value: &Value) -> Dynamic {
    match value {
        Value::Null => Dynamic::UNIT,
        Value::Bool(b) => Dynamic::from(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else {
                // u64 too large for i64, or a float.
                Dynamic::from(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(s) => Dynamic::from(s.clone()),
        Value::Array(arr) => {
            let items: rhai::Array = arr.iter().map(json_to_dynamic).collect();
            Dynamic::from(items)
        }
        Value::Object(obj) => {
            let mut map = rhai::Map::new();
            for (k, v) in obj {
                map.insert(k.as_str().into(), json_to_dynamic(v));
            }
            Dynamic::from(map)
        }
    }
}

/// Convert a Rhai `Dynamic` back into a JSON value.
pub fn dynamic_to_json(value: Dynamic) -> Result<Value, ExprError> {
    if value.is_unit() {
        return Ok(Value::Null);
    }
    if value.is::<bool>() {
        return Ok(Value::Bool(value.as_bool().unwrap()));
    }
    if value.is::<i64>() {
        return Ok(Value::Number(value.as_int().unwrap().into()));
    }
    if value.is::<f64>() {
        let f = value.as_float().unwrap();
        return serde_json::Number::from_f64(f)
            .map(Value::Number)
            .ok_or_else(|| ExprError::Eval(format!("non-finite number: {f}")));
    }
    if value.is::<char>() {
        return Ok(Value::String(value.as_char().unwrap().to_string()));
    }
    if value.is::<rhai::ImmutableString>() {
        return Ok(Value::String(value.into_string().unwrap()));
    }
    if value.is::<rhai::Array>() {
        let arr = value.cast::<rhai::Array>();
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            out.push(dynamic_to_json(item)?);
        }
        return Ok(Value::Array(out));
    }
    if value.is::<rhai::Map>() {
        let map = value.cast::<rhai::Map>();
        let mut out = serde_json::Map::with_capacity(map.len());
        for (k, v) in map {
            out.insert(k.to_string(), dynamic_to_json(v)?);
        }
        return Ok(Value::Object(out));
    }
    Err(ExprError::Eval(format!(
        "unsupported value type: {}",
        value.type_name()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roundtrip(v: Value) {
        let d = json_to_dynamic(&v);
        let back = dynamic_to_json(d).unwrap();
        assert_eq!(back, v, "roundtrip mismatch");
    }

    #[test]
    fn scalars_roundtrip() {
        roundtrip(json!(null));
        roundtrip(json!(true));
        roundtrip(json!(42));
        roundtrip(json!(-7));
        roundtrip(json!(3.5));
        roundtrip(json!("hello"));
    }

    #[test]
    fn nested_collections_roundtrip() {
        roundtrip(json!([1, "two", false, [3, 4]]));
        roundtrip(json!({ "user": { "name": "ada", "pts": 10 }, "list": [1, 2] }));
    }
}
