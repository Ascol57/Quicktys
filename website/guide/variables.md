---
description: "The three expression languages in Quicktys: @path and Handlebars templating in action fields, and sandboxed Rhai in conditions, filters and values."
---

# Variables & templating

There are **three** places you type an expression in Quicktys, and they do not
all use the same language. Getting them straight saves a lot of confusion.

| Where | Syntax | Language |
| --- | --- | --- |
| An action's **config fields** ("Message", "Scene"…) | `@path` and `{{ }}` | variable lookup + **Handlebars** |
| **Conditions and filters** (`if`, `while`, `until`, trigger filters, profile activation) | bare paths, `@` optional | **Rhai** |
| **Values** (`let`, `set`, `repeat` count, `for-each` list, `delay` ms, `switch` value) | bare paths, `@` optional | **Rhai** |

The rule of thumb: **fields that produce text use templating; fields that produce
a decision or a value use Rhai.**

## The context

Everything you can read comes from one flat context, assembled fresh for each
run:

```
{
  …the trigger context,     // this occurrence: user, message, viewers…
  …plugin and module state, // reactive values: obs.scene, pear.title…
  …local variables,         // let / set inside the automation
}
```

State keys are namespaced **flat**: the key is literally the string
`obs.scene`. The **Variables** view lists every one currently published, with its
value — the fastest way to find out what something is actually called.

<Screenshot src="/screens/variables.png" caption="The Variables view" />

## Templating in action fields

### `@path` — one variable

```
@pear.title
Now playing: @pear.title
```

`@` is only recognised at the **start of the string or after whitespace**, which
is why `contact@example.com` is left alone.

A **lone** `@path` — nothing else in the field — keeps the **typed** value. A list
stays a list, a number stays a number. That matters for fields that are not
strings:

```
@pear.playlist      → the actual list
Songs: @pear.playlist → "Songs: a, b, c" (interpolated for display)
```

### `{{ }}` — Handlebars

For anything more than a lookup, the field is [Handlebars](https://handlebarsjs.com):

```handlebars
{{upper pear.title}} — {{pear.artist}}
{{#if twitch.chat-channel}}watching {{twitch.chat-channel}}{{/if}}
{{#each pear.playlist}}{{this}}, {{/each}}
{{truncate message 80}}
```

Dotted keys are nested into a tree before rendering, so `{{pear.title}}` works
even though the key is flat. A missing value renders as empty rather than
erroring.

The full helper catalogue — maths, trigonometry, text, comparison, and the
built-in block helpers — is in [Templating helpers](/reference/templating).

### Order and escaping

`@` is resolved **first**, then Handlebars. A backslash makes the next character
literal:

| You type | You get |
| --- | --- |
| `\@notaviariable` | `@notaviariable` |
| `\{{ literal }}` | `{{ literal }}` |
| `\\` | `\` |

### In the editor

Template fields render as a highlighted input with autocompletion: type `@` for
variables (fed by the state every loaded plugin declares) or `{{` for the helper
menu.

<Screenshot src="/screens/template-input.png" caption="A template field with @ autocompletion" />

## Rhai in conditions and values

Conditions, filters, loop counts and `let`/`set` values are
[Rhai](https://rhai.rs) — a small embedded language with Rust-and-JavaScript-shaped
syntax.

```rhai
obs.scene == "Intro"
viewers >= 50 && message.starts_with("!")
pear.title != ""
[1, 2, 3]
count * 2 + 1
```

Both spellings of a state read work:

```rhai
obs.scene == "Intro"
@obs.scene == "Intro"    // identical — a leading @ is stripped
```

That is deliberate: whatever the editor's autocompletion drops into an action
field pastes unchanged into a condition.

### The sandbox

Rhai here has no filesystem, no network, no `eval`, no module loading, and hard
limits on operations, expression depth and value sizes. Evaluation is
**synchronous** and happens on an **already-materialised** context — no I/O
occurs during evaluation, which is what makes it deterministic and fast enough to
run on every state change.

Expressions call only a **standard library provided by the core**. Plugins cannot
inject functions: a plugin function would live in its WebAssembly module and
calling it mid-evaluation would mean an async ABI round-trip per call. Plugins
expose **values** through state instead.

### The trap

::: danger `=` is not `==`, and errors are silently false
In Rhai `=` assigns and `==` compares. Every compile error and every evaluation
error is coerced to **`false`**. A typo therefore disables a binding with no
message anywhere.

If a binding, condition or activation never fires, check the expression first —
and check the exact variable name in the Variables view.
:::

## Where the values come from

| Source | Example | Written by |
| --- | --- | --- |
| **Trigger context** | `message`, `user`, `viewers` | the trigger occurrence |
| **Plugin state** | `pear.title`, `obs.scene`, `twitch.chat` | a plugin calling `set-state` |
| **Locals** | `bonus`, `x` | your own `let` / `set` |
| **Action results** | whatever you bound | an action node's `bind` |

Plugins declare their state in their manifest, so the editor knows the names
before anything has ever run. [Integrations](/guide/integrations) lists what each
shipped plugin publishes.

## Reactivity

Because an expression is a syntax tree, the host extracts which variables it
reads. A `wait-for until` only re-evaluates when one of *its* dependencies
changes, not on every state write.

One consequence worth knowing: dependencies are **root identifiers** (`obs`)
while state keys are dotted (`obs.scene`), so a changed key is matched against a
dependency by its root segment.

## Next

- [Templating helpers](/reference/templating) — every helper, with signatures.
- [Automation nodes](/reference/nodes) — which field of which node is which
  language.
