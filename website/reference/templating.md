---
description: "Every Handlebars helper available in Quicktys action fields — maths, trigonometry, text, comparison and block helpers — plus the @path syntax and escaping."
---

# Templating helpers

Action config fields that opt into templating accept two syntaxes. `@` is
resolved first, then Handlebars.

## `@path`

A simple variable lookup against the flat context, recognised only at the **start
of the string or after whitespace** — so `contact@example.com` is left alone.

```
@pear.title
Now playing: @pear.title — @pear.artist
```

A **lone** `@path` keeps the **typed** value: a list stays a list, a number stays
a number. Mixed into a longer string, it interpolates its display form.

## `{{ }}`

Full [Handlebars](https://handlebarsjs.com). Dotted state keys are nested into a
tree first, so `{{pear.title}}` works even though the key is flat. Missing values
render empty rather than erroring.

## Escaping

A backslash makes the next character literal.

| You type | You get |
| --- | --- |
| `\@notavariable` | `@notavariable` |
| `\{{ literal }}` | `{{ literal }}` |
| `\\` | `\` |

## Helper catalogue

Every helper below is provided by the core. Plugins cannot add helpers — see
[why](#why-plugins-cannot-add-helpers).

### Maths

| Helper | Signature | Result |
| --- | --- | --- |
| `add` | `add a b` | a + b |
| `sub` | `sub a b` | a − b |
| `mul` | `mul a b` | a × b |
| `div` | `div a b` | a ÷ b |
| `mod` | `mod a b` | a % b |
| `idiv` | `idiv a b` | floor division |
| `pow` | `pow a b` | a ^ b |
| `min` | `min a b` | the smaller |
| `max` | `max a b` | the larger |
| `neg` | `neg a` | −a |
| `abs` | `abs a` | \|a\| |
| `round` | `round a` | nearest integer |
| `floor` | `floor a` | round down |
| `ceil` | `ceil a` | round up |
| `sqrt` | `sqrt a` | square root |
| `cbrt` | `cbrt a` | cube root |
| `exp` | `exp a` | e^a |
| `ln` | `ln a` | natural log |
| `log2` | `log2 a` | log base 2 |
| `log10` | `log10 a` | log base 10 |
| `log` | `log a base` | log in any base |
| `hypot` | `hypot a b` | √(a²+b²) |

### Trigonometry

| Helper | Signature | Result |
| --- | --- | --- |
| `sin` / `cos` / `tan` | `sin a` | in radians |
| `asin` / `acos` / `atan` | `asin a` | inverse |
| `atan2` | `atan2 y x` | arctan(y/x) |
| `sinh` / `cosh` / `tanh` | `sinh a` | hyperbolic |
| `rad` | `rad deg` | degrees → radians |
| `deg` | `deg rad` | radians → degrees |
| `pi` | `pi` | π |
| `tau` | `tau` | 2π |
| `e` | `e` | Euler's number |

### Text

| Helper | Signature | Result |
| --- | --- | --- |
| `upper` | `upper s` | UPPERCASE |
| `lower` | `lower s` | lowercase |
| `capitalize` | `capitalize s` | Capitalised |
| `trim` | `trim s` | strip surrounding spaces |
| `truncate` | `truncate s n` | first n characters, then `…` |
| `replace` | `replace s from to` | substitute |
| `default` | `default a b` | `b` if `a` is empty |
| `len` | `len v` | length of a string or list |
| `join` | `join list sep` | join a list |

### Comparison and logic

For use inside `#if`:

| Helper | Signature |
| --- | --- |
| `eq` | `eq a b` |
| `ne` | `ne a b` |
| `gt` | `gt a b` |
| `lt` | `lt a b` |
| `gte` | `gte a b` |
| `lte` | `lte a b` |
| `and` | `and a b` |
| `or` | `or a b` |
| `not` | `not a` |

### Block helpers

Handlebars built-ins:

| Helper | Signature | Use |
| --- | --- | --- |
| `if` | `#if cond` | conditional block |
| `unless` | `#unless cond` | negated conditional |
| `each` | `#each list` | loop |
| `with` | `#with obj` | scope into an object |

## Worked examples

```handlebars
{{upper pear.title}}

{{truncate message 80}}

{{default twitch.chat-channel "nobody"}}

{{#if (gt viewers 50)}}Big raid!{{/if}}

{{#each pear.playlist}}{{this}}, {{/each}}

{{round (mul progress 100)}}%
```

Parentheses nest a helper call as an argument, as in standard Handlebars.

## Why plugins cannot add helpers

A plugin-supplied function would live inside the plugin's WebAssembly component.
Calling it mid-evaluation would mean an async ABI round-trip per call — slow, and
it would break the synchronous sandbox that makes evaluation deterministic and
cheap enough to run on every state change.

Plugins expose **values** through state instead. The helper library is versioned
and extended by core releases.

## Not the same as Rhai

This page is about **templating in action config fields**. Conditions, filters and
value fields are [Rhai](https://rhai.rs), a different language with a different
standard library. `{{ }}` is invalid there.

See [Variables & templating](/guide/variables) for the full split.

## Next

- [Automation nodes](/reference/nodes) — which field uses which language.
- [Schema types](/reference/schema) — how a field opts into templating.
