//! Expression source, as stored in the schema/automation layer.
//!
//! At this layer an expression is just its **source text** — the persisted form
//! (`automation-engine.md` §2.8: "les expressions sont stockées en source").
//! Compilation to a Rhai AST and dependency extraction (`{ source, ast, deps }`)
//! happens in `quicktys-expr` at load time, so this crate stays free of any
//! expression-engine dependency.
//!
//! It (de)serializes transparently as a plain string, so YAML/JSON authors write
//! `cond: "points > 100"`, not a tagged object.

use serde::{Deserialize, Serialize};

/// The source text of a single expression (no surrounding `{{ }}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Expr(String);

impl Expr {
    pub fn new(source: impl Into<String>) -> Self {
        Expr(source.into())
    }

    /// The raw expression source, as it will be handed to `quicktys-expr`.
    pub fn source(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Expr {
    fn from(s: &str) -> Self {
        Expr(s.to_string())
    }
}

impl From<String> for Expr {
    fn from(s: String) -> Self {
        Expr(s)
    }
}
