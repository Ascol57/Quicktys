//! Splitting of `{{ ... }}` template strings into regions.
//!
//! This is the *parsing* half of the template system (the CastMate equivalent
//! of `parseTemplateString`). It only slices a string into literal-text and
//! expression regions; it does **not** compile or evaluate the expressions —
//! that is `quicktys-expr`'s job (see `docs/automation-engine.md` §1.4).
//!
//! Brace matching is expression-aware: an expression body may itself contain
//! braces (e.g. a Rhai object map `#{ a: 1 }`), so the closing `}}` is the first
//! one encountered at brace-depth zero.

use crate::expr::Expr;

/// One region of a parsed template string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRegion {
    /// Literal text, emitted verbatim.
    Text(String),
    /// An expression to evaluate, trimmed, without the surrounding `{{ }}`.
    Expr(Expr),
}

/// A template string parsed into ordered [`TemplateRegion`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateString {
    pub regions: Vec<TemplateRegion>,
}

impl TemplateString {
    /// Parse `input` into text/expression regions.
    ///
    /// An unterminated `{{` (no matching `}}` before end-of-input) is treated as
    /// literal text, so malformed templates degrade gracefully rather than
    /// swallowing the rest of the string.
    pub fn parse(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let mut regions = Vec::new();
        let mut text = String::new();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '{' && chars.get(i + 1) == Some(&'{') {
                if let Some((source, end)) = scan_expr(&chars, i + 2) {
                    if !text.is_empty() {
                        regions.push(TemplateRegion::Text(std::mem::take(&mut text)));
                    }
                    regions.push(TemplateRegion::Expr(Expr::new(source.trim())));
                    i = end;
                    continue;
                }
            }
            text.push(chars[i]);
            i += 1;
        }
        if !text.is_empty() {
            regions.push(TemplateRegion::Text(text));
        }
        TemplateString { regions }
    }

    /// True if the string contains at least one expression region.
    pub fn is_template(&self) -> bool {
        self.regions
            .iter()
            .any(|r| matches!(r, TemplateRegion::Expr(_)))
    }

    /// If the template is exactly one expression and nothing else, return it.
    ///
    /// This is the "typed field holding a single expression" case of
    /// `automation-engine.md` §1.4 (e.g. a `Duration` field whose whole value is
    /// `{{ timer.remaining + 5 }}`), as opposed to string interpolation.
    pub fn single_expression(&self) -> Option<&Expr> {
        match self.regions.as_slice() {
            [TemplateRegion::Expr(e)] => Some(e),
            _ => None,
        }
    }
}

/// Scan an expression body starting just after the opening `{{` (at `start`).
///
/// Returns `(source, index_after_closing_braces)`, or `None` if there is no
/// matching `}}` (depth-zero) before the end of input.
fn scan_expr(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut depth = 0u32;
    let mut source = String::new();
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c == '}' && depth == 0 && chars.get(i + 1) == Some(&'}') {
            return Some((source, i + 2));
        }
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        source.push(c);
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exprs(s: &str) -> Vec<String> {
        TemplateString::parse(s)
            .regions
            .into_iter()
            .filter_map(|r| match r {
                TemplateRegion::Expr(e) => Some(e.source().to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn plain_text_has_no_expressions() {
        let t = TemplateString::parse("hello world");
        assert!(!t.is_template());
        assert_eq!(t.regions, vec![TemplateRegion::Text("hello world".into())]);
    }

    #[test]
    fn interpolation_splits_into_regions() {
        let t = TemplateString::parse("Hi {{ user.name }}, {{ points }} pts");
        assert!(t.is_template());
        assert_eq!(
            t.regions,
            vec![
                TemplateRegion::Text("Hi ".into()),
                TemplateRegion::Expr(Expr::new("user.name")),
                TemplateRegion::Text(", ".into()),
                TemplateRegion::Expr(Expr::new("points")),
                TemplateRegion::Text(" pts".into()),
            ]
        );
    }

    #[test]
    fn single_expression_is_detected() {
        let t = TemplateString::parse("{{ timer.remaining + 5 }}");
        assert_eq!(
            t.single_expression().map(|e| e.source()),
            Some("timer.remaining + 5")
        );
        // Surrounding text disqualifies the single-expression case.
        assert!(TemplateString::parse("x {{ a }}").single_expression().is_none());
    }

    #[test]
    fn nested_braces_close_at_depth_zero() {
        // A Rhai object map inside the expression must not end it early.
        assert_eq!(exprs("{{ #{ a: 1, b: 2 } }}"), vec!["#{ a: 1, b: 2 }"]);
    }

    #[test]
    fn unterminated_expression_is_literal() {
        let t = TemplateString::parse("oops {{ unclosed");
        assert!(!t.is_template());
        assert_eq!(t.regions, vec![TemplateRegion::Text("oops {{ unclosed".into())]);
    }

    #[test]
    fn adjacent_expressions() {
        assert_eq!(exprs("{{a}}{{b}}"), vec!["a", "b"]);
    }
}
