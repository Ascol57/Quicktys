# Profiles & triggers

A **profile** is a named set of trigger→sequence bindings that can be switched on
and off as a whole. It is the unit of "what is Quicktys currently reacting to".

Why not just bind triggers globally? Because a stream has modes. During *Just
Chatting* you want follow alerts to be chatty; during a boss fight you want them
silent. Rather than editing a dozen bindings twice a session, you put them in two
profiles and let an **activation condition** choose.

<Screenshot src="/screens/profile.png" caption="A profile: activation condition + trigger bindings" />

## Anatomy

```yaml
# profiles/gaming.yaml
id: gaming
name: Gaming
activation: 'obs.scene == "Game"'
bindings:
  - trigger: twitch.chat-message
    filter: 'message.starts_with("!")'
    sequence:
      kind: block
      body:
        - kind: action
          plugin: notify
          action: log
          config: { message: "command from {{ user }}: {{ message }}" }
```

Three moving parts:

| | |
| --- | --- |
| **Activation** | A Rhai expression. While it is false, none of the profile's bindings fire. Leave it empty for "always on". |
| **Trigger** | Which event to listen for, as `<plugin>.<trigger>` — e.g. `twitch.follow`, `obs.scene-changed`, `button.clicked`. |
| **Filter** | A Rhai expression over the trigger's **context**. The sequence runs only if it is true. |

Each binding owns its sequence **inline**. A profile is self-contained: it does
not point at an automation by id. That keeps a profile copyable and readable as
one unit, at the cost of not sharing a sequence between two triggers — for that,
use a standalone [automation](/guide/automations) and call it.

## Picking a trigger

Click **Add trigger** and the picker opens: a search box over every trigger every
loaded plugin declares, grouped by plugin.

<Screenshot src="/screens/trigger-picker.png" caption="The trigger picker" />

The built-in **Buttons** group holds `button.clicked`, which the host fires
directly with no plugin in the loop. Everything else comes from an installed
plugin — install the Twitch plugin and its four triggers appear here.

## Trigger context

Every trigger occurrence carries a **context**: the payload describing *this*
particular occurrence. A chat message carries the user, the message text, the
badges; a song change carries the title and artist.

The context is available two ways inside the binding:

- in the **filter**, as bare identifiers — `message`, `user`;
- in the **sequence**, as template fields — `{{ message }}`, or `@message` for
  the typed value.

Plugins declare the shape of their context in their manifest, which is what
drives the autocompletion. What a specific trigger carries is listed in
[Integrations](/guide/integrations).

## Filters

The filter is the right place to narrow a trigger down. It is evaluated before
the sequence starts and before any queue is involved.

```rhai
// Only commands
message.starts_with("!")

// Only one specific button
id == "w3"

// Only a big raid
viewers >= 50

// Combine — `&&` and `||` work as expected
message.starts_with("!") && user != "mybot"
```

Filters and activation conditions read reactive state too, with or without the
`@` sigil:

```rhai
obs.scene == "Intro"
@obs.scene == "Intro"     // identical — the sigil is stripped
```

Both spellings work so that what the editor's autocompletion inserts into an
action field pastes unchanged into a condition.

::: danger A broken expression is silently false
Compile and evaluation errors are both coerced to `false`. A typo — `=` instead
of `==`, a variable that does not exist — disables the binding with no error
anywhere. If a binding never fires, suspect the filter before anything else, and
check the exact variable name in the [Variables](/guide/interface#variables) view.
:::

## Activation conditions

Same language, different scope: the activation gates the entire profile. Common
patterns:

```rhai
obs.streaming                      // only while live
obs.scene == "Game"                // scene-driven mode
obs.streaming && obs.scene != "BRB"
```

Because the expression engine extracts the variables an expression reads, the
host knows a profile's activation depends on `obs` and re-evaluates it only when
an `obs.*` key changes.

## Where profiles live

`<user-data>/quicktys/profiles/<id>.yaml`, one file per profile, written
atomically. They are plain YAML: readable, diffable, and copyable between
machines. See [Files & data directory](/reference/files).

The dispatcher shares the profile store with the UI, so a profile you edit takes
effect immediately — no restart, no reload.

## Next

- [Automations](/guide/automations) — what you can put in a sequence.
- [Variables & templating](/guide/variables) — the expression and template
  languages in full.
