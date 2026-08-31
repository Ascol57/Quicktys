//! End-to-end field/template resolution — the two uses, one grammar
//! (automation-engine §1.4).
//!
//! * [`ExprEngine::render_template`] — string interpolation: evaluate each
//!   `{{ }}` region, stringify, concatenate.
//! * [`ExprEngine::resolve_field`] — a typed field that may hold a template:
//!   a single whole-value expression is evaluated then coerced; an interpolated
//!   string is rendered then coerced; a plain value is coerced as-is.

use crate::coerce::coerce;
use crate::engine::ExprEngine;
use crate::error::ExprError;
use crate::eval::Context;
use quicktys_schema::{Schema, TemplateRegion, TemplateString};
use serde_json::Value;

impl ExprEngine {
    /// Render an interpolated template string to a final string.
    pub fn render_template(
        &self,
        template: &TemplateString,
        context: &Context,
    ) -> Result<String, ExprError> {
        let mut out = String::new();
        for region in &template.regions {
            match region {
                TemplateRegion::Text(t) => out.push_str(t),
                TemplateRegion::Expr(e) => {
                    let value = self.eval_source(e.source(), context)?;
                    out.push_str(&stringify(&value));
                }
            }
        }
        Ok(out)
    }

    /// Resolve a field value against its schema, applying templating + coercion.
    ///
    /// `raw` is the persisted value (often a string containing `{{ }}`). Returns
    /// the concrete, schema-typed value ready for the action invoke.
    pub fn resolve_field(
        &self,
        raw: &Value,
        schema: &Schema,
        context: &Context,
    ) -> Result<Value, ExprError> {
        // Only string-valued templatable fields go through the template path.
        if let (true, Value::String(s)) = (schema.is_templatable(), raw) {
            let template = TemplateString::parse(s);
            if let Some(expr) = template.single_expression() {
                // Whole value is one expression: evaluate to a typed value.
                let value = self.eval_source(expr.source(), context)?;
                return coerce(value, schema);
            }
            if template.is_template() {
                // Mixed text + expressions: render to a string, then coerce.
                let rendered = self.render_template(&template, context)?;
                return coerce(Value::String(rendered), schema);
            }
        }
        // No templating to do: coerce the literal into the field type.
        coerce(raw.clone(), schema)
    }
}

/// Display-stringify a JSON value for interpolation (null → empty, scalars as
/// their natural text, collections as compact JSON).
fn stringify(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(v: Value) -> Context {
        v.as_object().unwrap().clone()
    }

    fn schema(yaml: &str) -> Schema {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn interpolation_renders_and_stringifies() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "user": { "name": "ada" }, "points": 30 }));
        let t = TemplateString::parse("Hi {{ user.name }}, {{ points }} pts");
        assert_eq!(engine.render_template(&t, &c).unwrap(), "Hi ada, 30 pts");
    }

    #[test]
    fn single_expression_field_evaluates_then_coerces() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "base": 3 }));
        let s = schema("type: Duration\nmin: 0\nmax: 10\ntemplate: true");
        // Single expression → numeric result, clamped to max.
        let out = engine
            .resolve_field(&json!("{{ base + 100 }}"), &s, &c)
            .unwrap();
        assert_eq!(out, json!(10));
    }

    #[test]
    fn mixed_template_renders_then_coerces_to_number() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "a": 1, "b": 2 }));
        let s = schema("type: Number\ntemplate: true");
        // "1" + "2" rendered as the string "12", coerced back to a number.
        let out = engine.resolve_field(&json!("{{ a }}{{ b }}"), &s, &c).unwrap();
        assert_eq!(out, json!(12));
    }

    #[test]
    fn non_template_value_is_just_coerced() {
        let engine = ExprEngine::new();
        let s = schema("type: Number\ntemplate: true");
        let out = engine.resolve_field(&json!(5), &s, &Context::new()).unwrap();
        assert_eq!(out, json!(5));
    }

    #[test]
    fn non_templatable_field_passes_through() {
        let engine = ExprEngine::new();
        let s = schema("type: String"); // template: false
        let out = engine
            .resolve_field(&json!("{{ not.evaluated }}"), &s, &Context::new())
            .unwrap();
        // Left as a literal string (not evaluated) — coerced as-is.
        assert_eq!(out, json!("{{ not.evaluated }}"));
    }
}
