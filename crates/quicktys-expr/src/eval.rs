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

impl ExprEngine {
    /// Evaluate `expr` over `context`, returning the result as JSON.
    pub fn eval(&self, expr: &CompiledExpr, context: &Context) -> Result<Value, ExprError> {
        let mut scope = Scope::new();
        for (name, value) in context {
            scope.push_dynamic(name.clone(), json_to_dynamic(value));
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
