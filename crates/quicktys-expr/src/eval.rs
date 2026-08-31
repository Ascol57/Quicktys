//! Evaluation of a [`CompiledExpr`] against a materialised context.
//!
//! The context is a flat JSON object — the merge of trigger context, plugin/
//! module state, and automation locals (automation-engine §1.3). Each top-level
//! entry becomes a variable in the Rhai scope. Evaluation is synchronous and
//! does no I/O: the context is already resolved before we get here.

use crate::compile::CompiledExpr;
use crate::engine::ExprEngine;
use crate::error::ExprError;
use crate::value::{dynamic_to_json, json_to_dynamic};
use rhai::{Dynamic, Scope};
use serde_json::Value;

/// The evaluation context: top-level name → value.
pub type Context = serde_json::Map<String, Value>;

/// Nest flat dotted keys (`obs.scene`, `pear.title`) into a tree.
///
/// The context is flat — the state store namespaces plugin variables as
/// `<plugin>.<key>` — but a dotted name is not a usable identifier: in Rhai
/// `obs.scene` parses as a property read on a variable `obs`, and in Handlebars
/// it is a path. Both therefore need the same tree, which is why this lives
/// here and is shared with the templating layer rather than duplicated.
///
/// A parent occupied by a non-object loses to the subtree (a `pear` string and a
/// `pear.title` both present: `pear` becomes the object), because the dotted
/// keys are the ones a user writes paths against.
pub fn nest_context(context: &Context) -> serde_json::Map<String, Value> {
    let mut root = serde_json::Map::new();
    for (key, value) in context {
        let parts: Vec<&str> = key.split('.').collect();
        let mut node = &mut root;
        for part in &parts[..parts.len() - 1] {
            let entry = node
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(serde_json::Map::new());
            }
            node = entry.as_object_mut().unwrap();
        }
        node.insert(parts[parts.len() - 1].to_string(), value.clone());
    }
    root
}

impl ExprEngine {
    /// Evaluate `expr` over `context`, returning the result as JSON.
    ///
    /// Dotted context keys are nested first, so a condition reads state the same
    /// way a template does: `obs.scene == "Intro"`.
    pub fn eval(&self, expr: &CompiledExpr, context: &Context) -> Result<Value, ExprError> {
        let mut scope = Scope::new();
        for (name, value) in nest_context(context) {
            scope.push_dynamic(name, json_to_dynamic(&value));
        }
        let result: Dynamic = self
            .raw()
            .eval_ast_with_scope(&mut scope, expr.ast())
            .map_err(|e| ExprError::Eval(e.to_string()))?;
        dynamic_to_json(result)
    }

    /// Compile and evaluate in one step (no caching). Convenience for one-shot
    /// use; cache the [`CompiledExpr`] when evaluating repeatedly.
    pub fn eval_source(&self, source: &str, context: &Context) -> Result<Value, ExprError> {
        let compiled = self.compile(source)?;
        self.eval(&compiled, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(v: Value) -> Context {
        v.as_object().unwrap().clone()
    }

    /// State is namespaced flat (`obs.scene`), which is not an identifier. It
    /// must still be readable from a condition — this is what makes `if` nodes
    /// and trigger filters able to look at plugin state at all.
    #[test]
    fn dotted_state_keys_are_readable_as_paths() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "obs.scene": "Intro", "pear.title": "Song", "points": 3 }));
        assert_eq!(
            engine.eval_source(r#"obs.scene == "Intro""#, &c).unwrap(),
            json!(true)
        );
        assert_eq!(
            engine.eval_source(r#"obs.scene == "Outro""#, &c).unwrap(),
            json!(false)
        );
        // The `@` spelling of the config templating is accepted too, so one
        // syntax covers action fields, `if` conditions and trigger filters.
        assert_eq!(
            engine.eval_source(r#"@obs.scene == "Intro""#, &c).unwrap(),
            json!(true)
        );
        assert_eq!(
            engine
                .eval_source(r#"@pear.title != "" && @points > 1"#, &c)
                .unwrap(),
            json!(true)
        );
    }

    /// The sigil is a *source* rewrite, so it must not touch string contents.
    #[test]
    fn the_at_sigil_is_left_alone_inside_strings() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "mail": "a@b.com" }));
        assert_eq!(
            engine.eval_source(r#""a@b.com" == mail"#, &c).unwrap(),
            json!(true)
        );
    }

    #[test]
    fn evaluates_with_context() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "points": 10, "user": { "name": "ada" } }));
        assert_eq!(engine.eval_source("points + 5", &c).unwrap(), json!(15));
        assert_eq!(
            engine.eval_source(r#""hi " + user.name"#, &c).unwrap(),
            json!("hi ada")
        );
    }

    #[test]
    fn missing_variable_is_an_eval_error() {
        let engine = ExprEngine::new();
        let err = engine.eval_source("nope + 1", &Context::new()).unwrap_err();
        assert!(matches!(err, ExprError::Eval(_)));
    }

    #[test]
    fn arrays_and_maps_flow_through() {
        let engine = ExprEngine::new();
        let c = ctx(json!({ "xs": [1, 2, 3] }));
        assert_eq!(engine.eval_source("xs.len()", &c).unwrap(), json!(3));
    }
}
