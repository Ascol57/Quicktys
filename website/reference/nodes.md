# Automation nodes

An automation is a tree of typed nodes, serialised to YAML with a `kind`
discriminator and run by a tree-walking interpreter in `quicktys-core`.

Expressions are stored as their **source text** and compiled at load, so a broken
expression is a visible error before it is a runtime surprise.

## Which fields are which language

This is the thing to get right:

| Field | Language |
| --- | --- |
| an action's **config** values | templating — `@` and `{{ }}` |
| `if.cond`, `loop.cond`, `wait-for.cond`, a trigger **filter**, a profile **activation** | **Rhai** |
| `let.value`, `set.value`, `switch.value`, `loop.count`, `loop.list`, `wait-for.ms` | **Rhai** |
| a `switch` case's `equals (JSON)` | JSON, not an expression |

In the editor every Rhai field is a template input in **expression mode**: `@`
autocompletion still works and a leading `@` is stripped, but the `{{ }}` helper
menu is hidden because it would be invalid there.

## `action`

```yaml
- kind: action
  plugin: obs
  action: set-scene
  config: { scene: "Intro" }
  bind: result        # optional
```

| Field | |
| --- | --- |
| `plugin` | the plugin id |
| `action` | the action id |
| `config` | the action's config, templated and validated against its schema |
| `bind` | optional name to bind the action's return value to |

`bind` replaces the older "result mapping" pattern: the bound name is an ordinary
local, usable by every later node in scope.

## `block`

```yaml
- kind: block
  body:
    - { kind: action, plugin: notify, action: log, config: { message: "one" } }
    - { kind: action, plugin: notify, action: log, config: { message: "two" } }
```

Ordered children sharing one scope of local variables. A sequence is a block at
the top level.

## `parallel`

```yaml
- kind: parallel
  join: all           # all | any | race
  branches:
    - { kind: block, body: [ ... ] }
    - { kind: action, plugin: sound, action: play, config: { file: intro.mp3 } }
```

| Join | Behaviour |
| --- | --- |
| `all` | Wait for **every** branch, then continue. |
| `any` | Continue as soon as the first branch finishes; cancel the rest. |
| `race` | First one wins, losers are aborted. |

Each branch is a full node, so a branch can contain actions, conditionals and
loops.

## `timeline`

```yaml
- kind: timeline
  tracks:
    - offset_ms: 0
      body: { kind: action, plugin: sound, action: play, config: { file: sting.mp3 } }
    - offset_ms: 1500
      body: { kind: action, plugin: obs, action: set-scene, config: { scene: "Game" } }
```

Tracks offset from a common start — the visual, drag-on-a-time-axis shape. The
flow **joins** the tracks at the end rather than firing and forgetting them.

## `if`

```yaml
- kind: if
  cond: 'obs.scene == "Intro"'
  then: { kind: block, body: [ ... ] }
  otherwise: { kind: block, body: [ ... ] }     # optional
```

## `switch`

```yaml
- kind: switch
  value: "tier"
  cases:
    - { pattern: "1", body: { ... } }
    - { pattern: "2", body: { ... } }
  default: { ... }
```

## `loop`

Three shapes, each with a mandatory `max`:

```yaml
- kind: loop
  loop: repeat
  count: "3"
  max: 100
  body: { ... }

- kind: loop
  loop: while
  cond: "x < 10"
  max: 100
  body: { ... }

- kind: loop
  loop: for-each
  var: item
  list: "pear.playlist"
  max: 100
  body: { ... }
```

`count`, `cond` and `list` are expressions, so counts and lists can be computed.

::: info `max` is not advisory
Every loop is bounded. That is what makes it safe to let anyone write a loop in
software that runs unattended for eight hours.
:::

## `wait-for`

```yaml
- { kind: wait-for, wait: delay, ms: "1000" }

- kind: wait-for
  wait: until
  cond: "obs.streaming"
  timeout_ms: 30000

- kind: wait-for
  wait: event
  trigger: twitch.chat-message
  filter: 'message == "!go"'
  timeout_ms: 60000
```

| Wait | |
| --- | --- |
| `delay` | Sleep for `ms` — an expression, so delays can be computed. |
| `until` | Block until a condition over reactive state becomes true. |
| `event` | Block until a trigger fires, optionally filtered. |

`until` is genuinely reactive: the host knows which state keys the condition
reads and re-evaluates only when one of them changes.

::: tip Use `wait-for delay`, not a plugin's delay action
The `time` plugin's `delay` action is a pure ABI test fixture that does **not**
actually wait, which is why it is not shipped by default. Real delays are this
node.
:::

## `try`

```yaml
- kind: try
  body: { ... }
  catch: { ... }                          # optional
  retry: { max: 3, delay_ms: 1000 }       # optional
```

Without a `try`, a failing action ends the run with status *failed*.

## `let` and `set`

```yaml
- { kind: let, name: bonus, value: "viewers * 2" }
- { kind: set, target: bonus, value: "100" }
```

`let` introduces a local in the current block's scope; `set` reassigns an existing
one.

## `return`, `break`, `continue`

```yaml
- { kind: return, value: "bonus" }    # value optional
- { kind: break }
- { kind: continue }
```

`return` ends the automation. `break` and `continue` apply to the innermost loop.

## Execution semantics

**Cancellation.** A cancellation token is propagated to every node. An abort stops
actions, wakes waits, breaks loops and cancels parallel branches — nothing is left
running detached. A cancelled action reports `aborted`, which the host does not
surface as an error.

**Structured concurrency.** `parallel` waits according to its join policy; the
losers of `any`/`race` are genuinely cancelled, not orphaned.

**Bounds.** Every loop has a `max`, every `wait-for` has a timeout, and there is a
global time budget. No flow is unbounded.

**Errors.** A failing action propagates up. `try` intercepts it, with an optional
retry policy. Uncaught, the run ends *failed*.

**Progress.** The runner brackets each top-level node with real start and end
events, which the IPC relays as `NodeProgress { automation, path, running }`. That
is what lights up the executing card in the editor — actual execution, not an
estimate.

## Persistence

`<user-data>/quicktys/automations/<id>.yaml`, and inline inside a profile's
bindings for profile-owned sequences. Written atomically (temp file, then
rename).

## Coming from CastMate

| CastMate | Quicktys |
| --- | --- |
| `InstantAction` | `action` |
| `ActionStack` (Promise.all) | `parallel { join: all }` |
| `TimeAction` + offsets | `timeline { tracks }` |
| `FlowAction` + `subFlows` | `if` / `switch` |
| `resultMapping` | `action { bind }` + `let` |
| *(nothing)* | `loop`, `try`/retry, `wait-for`, `set`, lexical scopes |

The [migration tool](/reference/cli#quicktys-migrate) maps the old model onto the
new one and reports what it could not convert rather than guessing.

## Next

- [Templating helpers](/reference/templating)
- [Variables & templating](/guide/variables)
