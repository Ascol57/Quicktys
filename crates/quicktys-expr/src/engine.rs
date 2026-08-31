//! The sandboxed Rhai engine.
//!
//! Configured for safe host-side evaluation: bounded operations/depth/sizes
//! (anti runaway-loop), no module imports, and the dynamic `eval` disabled.
//! Rhai exposes no OS/network/filesystem by default, so the sandbox is about
//! resource limits, not capability removal.
//!
//! Expressions may only call Rhai's built-in standard library (math, strings,
//! arrays, maps). Plugins/modules never inject functions — they expose values
//! through the context (see `docs/automation-engine.md` §1.3).

use rhai::Engine;

/// Hard cap on Rhai operations per evaluation (bounds wall-time too, since each
/// op is cheap and there is no I/O).
const MAX_OPERATIONS: u64 = 1_000_000;
const MAX_EXPR_DEPTH: usize = 64;
const MAX_STRING_SIZE: usize = 256 * 1024;
const MAX_ARRAY_SIZE: usize = 100_000;
const MAX_MAP_SIZE: usize = 100_000;

/// A configured, sandboxed expression engine.
///
/// Cheap to hold; create one per host and reuse it across compiles/evals.
pub struct ExprEngine {
    engine: Engine,
}

impl ExprEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_operations(MAX_OPERATIONS);
        // Expressions are single-expression (no function defs), but bound depth
        // anyway to reject pathological nesting.
        engine.set_max_expr_depths(MAX_EXPR_DEPTH, MAX_EXPR_DEPTH);
        engine.set_max_string_size(MAX_STRING_SIZE);
        engine.set_max_array_size(MAX_ARRAY_SIZE);
        engine.set_max_map_size(MAX_MAP_SIZE);
        // No `import`, and no dynamic `eval` of strings: keep evaluation static
        // and analysable.
        engine.set_max_modules(0);
        engine.disable_symbol("eval");
        ExprEngine { engine }
    }

    /// The underlying Rhai engine (crate-internal: compile/eval go through it).
    pub(crate) fn raw(&self) -> &Engine {
        &self.engine
    }
}

impl Default for ExprEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_deep_expression_is_rejected() {
        // A pathologically nested expression exceeds the depth cap and is
        // refused at compile time rather than risking a stack blow-up.
        let engine = ExprEngine::new();
        let deep = (0..MAX_EXPR_DEPTH + 50)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("+");
        assert!(engine.compile(&deep).is_err());
        // A modest expression of the same shape compiles fine.
        assert!(engine.compile("1 + 2 + 3 + 4 + 5").is_ok());
    }

    #[test]
    fn dynamic_eval_is_disabled() {
        let engine = ExprEngine::new();
        let r = engine
            .raw()
            .eval_expression::<i64>(r#"eval("1+1")"#);
        assert!(r.is_err(), "eval() must be disabled in the sandbox");
    }
}
