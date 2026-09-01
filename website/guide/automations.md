# Automations

An automation is a **tree of typed nodes**. Not a list of actions with a couple
of special cases bolted on — an actual tree, run by an interpreter in the host
with cancellation, structured concurrency and bounded loops.

You meet it in two places: inline inside a [profile](/guide/profiles) binding,
and as a standalone automation in the **Automations** tab.

<Screenshot src="/screens/automation-editor.png" caption="The automation editor" />

## The editor

Click **+** to open the node palette — searchable, grouped by category.

<Screenshot src="/screens/node-palette.png" caption="The node palette" />

| Category | Nodes |
| --- | --- |
| **Core** | Action |
| **Structure** | Block, Parallel, Timeline |
| **Flow** | If, Switch, Loop, Wait, Try |
| **Vars** | Let, Set |
| **Jump** | Return, Break, Continue |

Nodes reorder by drag-and-drop and nest by dropping into a container node's body.
While a sequence runs, the node currently executing lights up — the host reports
real start and end events for each top-level node, so the highlight follows
actual execution rather than a client-side guess at durations.

## The nodes

Full field-by-field reference in [Automation nodes](/reference/nodes). The short
version:

### Action

Invokes a plugin action. Pick the plugin, pick the action, fill in the form —
which is **generated from the schema** the plugin declared for that action.

Optionally **bind** the result to a name, which makes it available to later nodes
as a local variable. That replaces the awkward "result mapping" pattern.

### Block

An ordered list of children sharing one scope of local variables. This is what a
sequence *is* at the top level.

### Parallel

Runs its branches at the same time, with a **join policy**:

| Join | Behaviour |
| --- | --- |
| `all` | Wait for **every** branch to finish, then continue. |
| `any` | Continue as soon as the first branch finishes; the rest are cancelled. |
| `race` | Same as `any` — first one wins, losers are aborted. |

Each branch is a whole node, so a branch can be a block containing actions,
conditionals and loops — not just a single action.

```yaml
- kind: parallel
  join: all
  branches:
    - kind: block
      body:
        - { kind: action, plugin: obs, action: start-record }
        - { kind: wait-for, wait: delay, ms: "3000" }
    - kind: action
      plugin: sound
      action: play
      config: { file: intro.mp3, volume: 80 }
# reached only once BOTH branches have finished
- { kind: action, plugin: twitch, action: chat, config: { message: "Here we go" } }
```

::: tip This is the node people miss
"Start three things and wait for all of them" is a first-class node. It is not a
delay you tuned by hand, and the losers of an `any` join are genuinely cancelled
rather than left running detached.
:::

### Timeline

Tracks offset in time from a common start — the visual, drag-on-a-time-axis shape
familiar from CastMate. Each track has an `offset_ms` and a body. Unlike
CastMate's offsets, the flow *joins* the tracks at the end rather than firing
them and forgetting them.

### If / Switch

Inline conditionals. `if` takes a condition and a `then`, optionally an `else`.
`switch` evaluates a value and matches it against cases with an optional default.

Conditions are Rhai expressions. See [Variables & templating](/guide/variables).

### Loop

Three shapes, all with a mandatory `max` iteration cap:

| Loop | Fields |
| --- | --- |
| `repeat` | `count` — an expression, so the number can be dynamic |
| `while` | `cond` — re-evaluated each pass |
| `for-each` | `var` + `list` — iterate an expression that yields a list |

The cap is not optional and not advisory. It is what makes it safe to let anyone
write a loop in software that runs unattended for eight hours.

### Wait

Three shapes:

| Wait | Meaning |
| --- | --- |
| `delay` | Sleep for `ms` — an expression, so delays can be computed. |
| `until` | Block until a condition over reactive state becomes true, with a timeout. |
| `event` | Block until a trigger fires, with an optional filter and a timeout. |

`until` is genuinely reactive: the host knows which state keys the condition
reads and only re-evaluates when one of them changes.

### Try

Wraps a body with an optional `catch` handler and an optional `retry` policy
(`max` attempts, `delay_ms` between them). Without a `try`, a failing action ends
the run with status *failed*.

### Let / Set

`let` introduces a local in the current block's scope; `set` reassigns an
existing one. Both take an expression.

```yaml
- { kind: let, name: bonus, value: "viewers * 2" }
- { kind: if, cond: "bonus > 100", then: { kind: set, target: bonus, value: "100" } }
```

### Return / Break / Continue

`return` ends the automation, optionally with a value. `break` and `continue`
apply to the innermost loop.

## Cancellation

Every run carries a cancellation token propagated to every node. Aborting a run
stops actions, wakes waits, breaks loops and cancels parallel branches — nothing
is left running detached.

An action that is cancelled reports `aborted` rather than `failed`, so the host
does not surface it as an error.

## Action queues

Some things must not overlap. Two follower alerts firing at once means two sounds
playing over each other and two overlay animations fighting.

An **action queue** is a resource that runs sequences **one at a time**:

- a configurable **gap** between consecutive runs,
- **pause** and **resume**,
- **skip**, which aborts the running job and moves on,
- **clear**,
- a **history** you can replay from.

An automation either targets a queue or runs immediately. Alerts belong in a
queue; a scene change does not.

## Where automations live

`<user-data>/quicktys/automations/<id>.yaml`. Expressions are stored as their
**source text** and compiled at load time, so a broken expression is a visible
error before it is a runtime surprise.

## Next

- [Variables & templating](/guide/variables) — the two template syntaxes and the
  expression language.
- [Automation nodes reference](/reference/nodes) — every field of every node.
