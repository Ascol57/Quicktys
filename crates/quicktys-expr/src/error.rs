//! Error type for the expression engine.

use thiserror::Error;

/// A failure while compiling, evaluating, or coercing an expression.
#[derive(Debug, Error)]
pub enum ExprError {
    /// The source did not parse as a valid single expression.
    #[error("expression compile error: {0}")]
    Compile(String),
    /// Evaluation failed (runtime error, limit exceeded, unknown variable, ...).
    #[error("expression eval error: {0}")]
    Eval(String),
    /// The evaluated value could not be coerced into the target schema type.
    #[error("coercion error: {0}")]
    Coerce(String),
}
