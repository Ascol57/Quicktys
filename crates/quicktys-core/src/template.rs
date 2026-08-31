//! Template resolution for action config strings: `@variable` (simple flat-key
//! lookup) and `{{ }}` (Handlebars, with math / text / comparison helpers).
//! `\` escapes the next char (`\@`, `\{{` → literal; `\\` → `\`).
//! Conditions/filters/loops are Rhai `Expr` (no `{{ }}`) and live in `sequence`.

use crate::sequence::truthy;
use quicktys_expr::Context;
use serde_json::Value;

// Placeholders (Unicode private-use) for escaped `{` `}` `\` — they hide from
// Handlebars (so an escaped `{{` is not parsed) and are restored last.
const PH_LBRACE: char = '\u{E001}';
const PH_RBRACE: char = '\u{E002}';
const PH_BSLASH: char = '\u{E003}';

/// Fully resolve a config string: `@`-variables + escapes, then `{{ }}`
/// Handlebars, then restore escaped literals. A lone `@path` keeps its typed
/// value; anything else yields a string.
pub(crate) fn resolve(s: &str, ctx: &Context) -> Result<Value, String> {
    let text = match resolve_ats(s, ctx) {
        Value::String(t) => t,
        typed => return Ok(typed), // lone `@path` — no escapes possible
    };
    let text = if text.contains("{{") {
        render_str(&text, ctx)?
    } else {
        text
    };
    let out = if text.contains(|c| matches!(c, PH_LBRACE | PH_RBRACE | PH_BSLASH)) {
        text.replace(PH_LBRACE, "{").replace(PH_RBRACE, "}").replace(PH_BSLASH, "\\")
    } else {
        text
    };
    Ok(Value::String(out))
}

/// Resolve `@variable` tokens and `\`-escapes in `s`. A string that is exactly
/// `@path` returns that key's **typed** value (so `@pear.artists` stays a list);
/// otherwise every valid `@path` is interpolated as its display string. `@` is
/// only a variable at the start of the string or right after whitespace (so
/// `a@b.com` is left alone). `\` escapes the next char; escaped braces/backslash
/// become placeholders (restored after Handlebars runs).
fn resolve_ats(s: &str, ctx: &Context) -> Value {
    if !s.contains('@') && !s.contains('\\') {
        return Value::String(s.to_string());
    }
    let lookup = |path: &str| ctx.get(path).cloned().unwrap_or(Value::Null);
    if let Some(path) = lone_at(s.trim()) {
        return lookup(path);
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        // `\` escapes the next char.
        if chars[i] == '\\' {
            match chars.get(i + 1) {
                Some('{') => out.push(PH_LBRACE),
                Some('}') => out.push(PH_RBRACE),
                Some('\\') => out.push(PH_BSLASH),
                Some(&ch) => out.push(ch), // @, letters, … → literal
                None => out.push(PH_BSLASH), // trailing `\` → literal backslash
            }
            i += if i + 1 < chars.len() { 2 } else { 1 };
            continue;
        }
        let boundary = i == 0 || chars[i - 1].is_whitespace();
        if chars[i] == '@' && boundary {
            let (path, next) = read_path(&chars, i + 1);
            if !path.is_empty() {
                out.push_str(&display(&lookup(&path)));
                i = next;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    Value::String(out)
}

/// Whole floats render as ints (`3`, not `3.0`).
fn num(x: f64) -> Value {
    if x.is_finite() && x.fract() == 0.0 {
        Value::from(x as i64)
    } else {
        Value::from(x)
    }
}

// Math helpers (prefix form — Handlebars has no infix operators). Binary:
//   {{add a b}} +   {{sub a b}} -   {{mul a b}} *   {{div a b}} /
//   {{mod a b}} %   {{idiv a b}} //  (floor division)   {{pow a b}} ^
//   {{min a b}}     {{max a b}}
// Unary: {{neg a}}  {{abs a}}  {{round a}}  {{floor a}}  {{ceil a}}
handlebars::handlebars_helper!(hb_add: |a: f64, b: f64| num(a + b));
handlebars::handlebars_helper!(hb_sub: |a: f64, b: f64| num(a - b));
handlebars::handlebars_helper!(hb_mul: |a: f64, b: f64| num(a * b));
handlebars::handlebars_helper!(hb_div: |a: f64, b: f64| if b != 0.0 { num(a / b) } else { Value::Null });
handlebars::handlebars_helper!(hb_mod: |a: f64, b: f64| if b != 0.0 { num(a % b) } else { Value::Null });
handlebars::handlebars_helper!(hb_idiv: |a: f64, b: f64| if b != 0.0 { num((a / b).floor()) } else { Value::Null });
handlebars::handlebars_helper!(hb_pow: |a: f64, b: f64| num(a.powf(b)));
handlebars::handlebars_helper!(hb_min: |a: f64, b: f64| num(a.min(b)));
handlebars::handlebars_helper!(hb_max: |a: f64, b: f64| num(a.max(b)));
handlebars::handlebars_helper!(hb_neg: |a: f64| num(-a));
handlebars::handlebars_helper!(hb_abs: |a: f64| num(a.abs()));
handlebars::handlebars_helper!(hb_round: |a: f64| num(a.round()));
handlebars::handlebars_helper!(hb_floor: |a: f64| num(a.floor()));
handlebars::handlebars_helper!(hb_ceil: |a: f64| num(a.ceil()));
// Powers / roots / exp / log: {{sqrt a}} {{cbrt a}} {{exp a}} {{ln a}}
// {{log2 a}} {{log10 a}} {{log a base}} {{hypot a b}}
handlebars::handlebars_helper!(hb_sqrt: |a: f64| num(a.sqrt()));
handlebars::handlebars_helper!(hb_cbrt: |a: f64| num(a.cbrt()));
handlebars::handlebars_helper!(hb_exp: |a: f64| num(a.exp()));
handlebars::handlebars_helper!(hb_ln: |a: f64| num(a.ln()));
handlebars::handlebars_helper!(hb_log2: |a: f64| num(a.log2()));
handlebars::handlebars_helper!(hb_log10: |a: f64| num(a.log10()));
handlebars::handlebars_helper!(hb_log: |a: f64, base: f64| num(a.log(base)));
handlebars::handlebars_helper!(hb_hypot: |a: f64, b: f64| num(a.hypot(b)));
// Trigonometry (radians): sin cos tan asin acos atan {{atan2 y x}} sinh cosh tanh.
handlebars::handlebars_helper!(hb_sin: |a: f64| num(a.sin()));
handlebars::handlebars_helper!(hb_cos: |a: f64| num(a.cos()));
handlebars::handlebars_helper!(hb_tan: |a: f64| num(a.tan()));
handlebars::handlebars_helper!(hb_asin: |a: f64| num(a.asin()));
handlebars::handlebars_helper!(hb_acos: |a: f64| num(a.acos()));
handlebars::handlebars_helper!(hb_atan: |a: f64| num(a.atan()));
handlebars::handlebars_helper!(hb_atan2: |y: f64, x: f64| num(y.atan2(x)));
handlebars::handlebars_helper!(hb_sinh: |a: f64| num(a.sinh()));
handlebars::handlebars_helper!(hb_cosh: |a: f64| num(a.cosh()));
handlebars::handlebars_helper!(hb_tanh: |a: f64| num(a.tanh()));
handlebars::handlebars_helper!(hb_rad: |deg: f64| num(deg.to_radians()));
handlebars::handlebars_helper!(hb_deg: |rad: f64| num(rad.to_degrees()));
// Constants: {{pi}} {{tau}} {{e}}.
handlebars::handlebars_helper!(hb_pi: | | num(std::f64::consts::PI));
handlebars::handlebars_helper!(hb_tau: | | num(std::f64::consts::TAU));
handlebars::handlebars_helper!(hb_e: | | num(std::f64::consts::E));

/// A value counts as "empty" for `{{default}}`: null, "", or [].
fn is_empty_json(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

// Text helpers: {{upper s}} {{lower s}} {{capitalize s}} {{trim s}}
// {{truncate s n}} {{replace s from to}} {{default a b}} {{len v}} {{join v sep}}
handlebars::handlebars_helper!(hb_upper: |s: str| s.to_uppercase());
handlebars::handlebars_helper!(hb_lower: |s: str| s.to_lowercase());
handlebars::handlebars_helper!(hb_trim: |s: str| s.trim().to_string());
handlebars::handlebars_helper!(hb_capitalize: |s: str| {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
});
handlebars::handlebars_helper!(hb_truncate: |s: str, n: i64| {
    let n = n.max(0) as usize;
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
});
handlebars::handlebars_helper!(hb_replace: |s: str, from: str, to: str| s.replace(from, to));
handlebars::handlebars_helper!(hb_default: |a: Json, b: Json| if is_empty_json(a) { b.clone() } else { a.clone() });
handlebars::handlebars_helper!(hb_len: |v: Json| {
    match v {
        Value::String(s) => s.chars().count() as i64,
        Value::Array(a) => a.len() as i64,
        Value::Object(o) => o.len() as i64,
        _ => 0,
    }
});
handlebars::handlebars_helper!(hb_join: |v: Json, sep: str| {
    match v {
        Value::Array(a) => a
            .iter()
            .map(display)
            .collect::<Vec<_>>()
            .join(sep),
        other => display(other),
    }
});

// Comparison / logic (for {{#if (eq a b)}} …): {{eq a b}} {{ne a b}}
// {{gt a b}} {{lt a b}} {{gte a b}} {{lte a b}} {{and a b}} {{or a b}} {{not a}}
handlebars::handlebars_helper!(hb_eq: |a: Json, b: Json| a == b);
handlebars::handlebars_helper!(hb_ne: |a: Json, b: Json| a != b);
handlebars::handlebars_helper!(hb_gt: |a: f64, b: f64| a > b);
handlebars::handlebars_helper!(hb_lt: |a: f64, b: f64| a < b);
handlebars::handlebars_helper!(hb_gte: |a: f64, b: f64| a >= b);
handlebars::handlebars_helper!(hb_lte: |a: f64, b: f64| a <= b);
handlebars::handlebars_helper!(hb_and: |a: Json, b: Json| truthy(a) && truthy(b));
handlebars::handlebars_helper!(hb_or: |a: Json, b: Json| truthy(a) || truthy(b));
handlebars::handlebars_helper!(hb_not: |a: Json| !truthy(a));

/// The shared Handlebars registry (built once): non-strict, no HTML escaping,
/// with the math helpers registered.
fn hb_registry() -> &'static handlebars::Handlebars<'static> {
    static HB: std::sync::OnceLock<handlebars::Handlebars<'static>> = std::sync::OnceLock::new();
    HB.get_or_init(|| {
        let mut hb = handlebars::Handlebars::new();
        hb.set_strict_mode(false);
        hb.register_escape_fn(handlebars::no_escape); // plain text, not HTML
        hb.register_helper("add", Box::new(hb_add));
        hb.register_helper("sub", Box::new(hb_sub));
        hb.register_helper("mul", Box::new(hb_mul));
        hb.register_helper("div", Box::new(hb_div));
        hb.register_helper("mod", Box::new(hb_mod));
        hb.register_helper("idiv", Box::new(hb_idiv));
        hb.register_helper("pow", Box::new(hb_pow));
        hb.register_helper("min", Box::new(hb_min));
        hb.register_helper("max", Box::new(hb_max));
        hb.register_helper("neg", Box::new(hb_neg));
        hb.register_helper("abs", Box::new(hb_abs));
        hb.register_helper("round", Box::new(hb_round));
        hb.register_helper("floor", Box::new(hb_floor));
        hb.register_helper("ceil", Box::new(hb_ceil));
        hb.register_helper("sqrt", Box::new(hb_sqrt));
        hb.register_helper("cbrt", Box::new(hb_cbrt));
        hb.register_helper("exp", Box::new(hb_exp));
        hb.register_helper("ln", Box::new(hb_ln));
        hb.register_helper("log2", Box::new(hb_log2));
        hb.register_helper("log10", Box::new(hb_log10));
        hb.register_helper("log", Box::new(hb_log));
        hb.register_helper("hypot", Box::new(hb_hypot));
        hb.register_helper("sin", Box::new(hb_sin));
        hb.register_helper("cos", Box::new(hb_cos));
        hb.register_helper("tan", Box::new(hb_tan));
        hb.register_helper("asin", Box::new(hb_asin));
        hb.register_helper("acos", Box::new(hb_acos));
        hb.register_helper("atan", Box::new(hb_atan));
        hb.register_helper("atan2", Box::new(hb_atan2));
        hb.register_helper("sinh", Box::new(hb_sinh));
        hb.register_helper("cosh", Box::new(hb_cosh));
        hb.register_helper("tanh", Box::new(hb_tanh));
        hb.register_helper("rad", Box::new(hb_rad));
        hb.register_helper("deg", Box::new(hb_deg));
        hb.register_helper("pi", Box::new(hb_pi));
        hb.register_helper("tau", Box::new(hb_tau));
        hb.register_helper("e", Box::new(hb_e));
        hb.register_helper("upper", Box::new(hb_upper));
        hb.register_helper("lower", Box::new(hb_lower));
        hb.register_helper("trim", Box::new(hb_trim));
        hb.register_helper("capitalize", Box::new(hb_capitalize));
        hb.register_helper("truncate", Box::new(hb_truncate));
        hb.register_helper("replace", Box::new(hb_replace));
        hb.register_helper("default", Box::new(hb_default));
        hb.register_helper("len", Box::new(hb_len));
        hb.register_helper("join", Box::new(hb_join));
        hb.register_helper("eq", Box::new(hb_eq));
        hb.register_helper("ne", Box::new(hb_ne));
        hb.register_helper("gt", Box::new(hb_gt));
        hb.register_helper("lt", Box::new(hb_lt));
        hb.register_helper("gte", Box::new(hb_gte));
        hb.register_helper("lte", Box::new(hb_lte));
        hb.register_helper("and", Box::new(hb_and));
        hb.register_helper("or", Box::new(hb_or));
        hb.register_helper("not", Box::new(hb_not));
        hb
    })
}

/// Render a `{{ }}` Handlebars template against the run context. The flat,
/// dotted state keys (`pear.title`) are nested into a tree so `{{pear.title}}`,
/// `{{#each pear.playlist}}…{{/each}}`, `{{sub a b}}`, etc. resolve naturally.
/// Missing values render empty (non-strict).
fn render_str(source: &str, ctx: &Context) -> Result<String, String> {
    hb_registry()
        .render_template(source, &nest_context(ctx))
        .map_err(|e| format!("template error: {e}"))
}

/// Nest flat dotted keys (`pear.title`) into a tree for Handlebars path lookup.
/// Shared with the expression engine, which needs the very same tree so a
/// condition and a template read state identically.
fn nest_context(ctx: &Context) -> Value {
    Value::Object(quicktys_expr::nest_context(ctx))
}

/// Characters allowed inside one `@path` segment (keys may hold `-`, e.g.
/// `pear.video-id`).
fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Read a dotted path (`seg('.'seg)*`) starting at `start`; returns the path and
/// the index just past it. Stops before a trailing `.` (e.g. end of a sentence).
fn read_path(chars: &[char], start: usize) -> (String, usize) {
    let mut j = start;
    while j < chars.len() {
        let dot_segment = chars[j] == '.' && j + 1 < chars.len() && is_path_char(chars[j + 1]);
        if is_path_char(chars[j]) || dot_segment {
            j += 1;
        } else {
            break;
        }
    }
    (chars[start..j].iter().collect(), j)
}

/// If `s` is exactly one `@path` (nothing else), return that path.
fn lone_at(s: &str) -> Option<&str> {
    let rest = s.strip_prefix('@')?;
    if rest.is_empty() || rest.starts_with('.') || rest.ends_with('.') {
        return None;
    }
    rest.chars()
        .all(|c| is_path_char(c) || c == '.')
        .then_some(rest)
}

/// How a value renders when interpolated into text: strings verbatim, `null` as
/// empty, everything else as JSON.
fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
