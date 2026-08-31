//! Compilation of expression source into a reusable, analysed form.
//!
//! [`CompiledExpr`] holds the source, the compiled Rhai `AST`, and the set of
//! **context variables it reads** (`deps`). Dependency extraction is the payoff
//! of having a real AST instead of opaque JS: the host re-evaluates a template
//! only when one of its `deps` changes (targeted reactivity, automation-engine
//! §1.5).

use crate::engine::ExprEngine;
use crate::error::ExprError;
use rhai::{ASTNode, Expr, AST};

/// A compiled expression: source + AST + the context variables it depends on.
pub struct CompiledExpr {
    source: String,
    ast: AST,
    deps: Vec<String>,
}

impl CompiledExpr {
    /// Original source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Context variables this expression reads, sorted and de-duplicated.
    /// Drives reactive re-evaluation.
    pub fn deps(&self) -> &[String] {
        &self.deps
    }

    pub(crate) fn ast(&self) -> &AST {
        &self.ast
    }
}

impl ExprEngine {
    /// Compile a single expression, extracting its variable dependencies.
    ///
    /// The `@` sigil of the config templating (`@obs.scene`) is accepted and
    /// stripped first, so one spelling works everywhere: what the editor's
    /// autocomplete inserts into an action field also compiles in an `if`
    /// condition or a trigger filter. The stored `source` stays what the user
    /// wrote.
    pub fn compile(&self, source: &str) -> Result<CompiledExpr, ExprError> {
        let ast = self
            .raw()
            .compile_expression(&strip_at_sigils(source))
            .map_err(|e| ExprError::Compile(e.to_string()))?;
        let deps = extract_deps(&ast);
        Ok(CompiledExpr {
            source: source.to_string(),
            ast,
            deps,
        })
    }

    /// Convenience: compile a schema [`quicktys_schema::Expr`] (source-only).
    pub fn compile_expr(&self, expr: &quicktys_schema::Expr) -> Result<CompiledExpr, ExprError> {
        self.compile(expr.source())
    }
}

/// Drop the `@` sigil in front of a variable read, so `@obs.scene` compiles as
/// `obs.scene`.
///
/// Only outside string literals, and only when an identifier actually follows —
/// so an email in a string, a `@` used as an operator, or a lone `@` are left
/// exactly as they are.
fn strip_at_sigils(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' && q != '`' {
                escaped = true; // backtick strings are raw in Rhai
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' | '`' => {
                quote = Some(c);
                out.push(c);
            }
            '@' if matches!(chars.peek(), Some(n) if n.is_alphabetic() || *n == '_') => {}
            _ => out.push(c),
        }
    }
    out
}

/// Walk the AST and collect the names of (non-namespaced) variables read.
///
/// We collect *root* identifiers: in `user.name` only `user` is a variable
/// (`name` is a property access), which is the right granularity for reactivity
/// — depend on the `user` context entry.
fn extract_deps(ast: &AST) -> Vec<String> {
    let mut deps: Vec<String> = Vec::new();
    ast.walk(&mut |path| {
        if let Some(ASTNode::Expr(Expr::Variable(x, ..))) = path.last() {
            // x = (long index, name, namespace, hash); only count non-qualified
            // (no `module::` prefix) reads as context dependencies.
            let namespace = &x.2;
            if namespace.is_empty() {
                let name = x.1.as_str();
                if !deps.iter().any(|d| d == name) {
                    deps.push(name.to_string());
                }
            }
        }
        true
    });
    deps.sort();
    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deps(src: &str) -> Vec<String> {
        ExprEngine::new().compile(src).unwrap().deps().to_vec()
    }

    #[test]
    fn extracts_root_variables() {
        assert_eq!(deps("points + 5"), vec!["points"]);
        assert_eq!(deps("user.name"), vec!["user"]);
        assert_eq!(deps("a + b * a"), vec!["a", "b"]);
    }

    #[test]
    fn function_calls_are_not_deps() {
        // `max` is a function, not a context variable; only `x`/`y` are.
        assert_eq!(deps("max(x, y)"), vec!["x", "y"]);
    }

    #[test]
    fn constants_have_no_deps() {
        assert!(deps("1 + 2 * 3").is_empty());
        assert!(deps(r#""hello""#).is_empty());
    }

    #[test]
    fn compile_rejects_garbage() {
        assert!(ExprEngine::new().compile("1 +").is_err());
    }
}
