# The interface

Quicktys is a tabbed desktop application. A sidebar on the left lists the
sections; clicking one opens it as a tab, and tabs other than the Dashboard can
be closed.

<Screenshot src="/screens/sidebar.png" caption="The sidebar, with the Profiles folder expanded" />

## The sidebar

| Section | What lives there |
| --- | --- |
| **Dashboard** | Your grid of widgets. Always open, never closable. |
| **Profiles** | A *folder*: it expands to list your profiles, and each one opens in its own tab. |
| **Automations** | Standalone sequences, editable and runnable on their own. |
| **Variables** | Every reactive state value published by loaded plugins and modules. |
| **Marketplace** | Browse and install plugins and widgets. |
| **Settings** | Language, accounts, capability consent, licence keys. |

Installed **modules** can add their own tab. That is not a special case coded
into the app: a module declares a `view` in its manifest, and the app renders the
HTML page it ships in a sandboxed iframe. The **Overlays** tab you get after
installing the Overlays plugin is exactly this — contributed by the `webview`
module, not hardcoded.

## Dashboard

The landing tab: a grid where you place widgets. In **Edit** mode you can add,
move, resize and configure them; the layout is stored locally. Full tour in
[Dashboard](/guide/dashboard).

<Screenshot src="/screens/dashboard.png" caption="The dashboard" />

## Profiles

A profile is a named bundle of trigger→sequence bindings, plus an **activation
condition** that switches the whole bundle on or off. Profiles are what let you
have a "Just Chatting" set of reactions and a "Gaming" set without them fighting
each other.

Each binding has its own inline sequence — a profile *owns* its sequences rather
than referring to shared automations by id. See [Profiles &
triggers](/guide/profiles).

## Automations

Standalone sequences that are not attached to a trigger. Useful for something you
run by hand, reuse from a button, or build and test before wiring it up.

The editor is the same one profiles use: a node canvas with a searchable palette,
drag-and-drop reordering, and **live execution highlighting** — while a sequence
runs, the node currently executing lights up, driven by real start/end events
from the host runner rather than a client-side guess.

<Screenshot src="/screens/automation-editor.png" caption="The automation editor" />

## Variables

Every value the loaded extensions publish, flat and namespaced by extension id:
`obs.scene`, `pear.title`, `twitch.chat`, `countdown.timer`. This is the same
context your conditions and templates read from, so it is the fastest way to find
out what a variable is actually called and what shape it currently has.

<Screenshot src="/screens/variables.png" caption="The Variables view" />

State is namespaced **flat** — the key really is the string `obs.scene`. The
expression engine nests dotted keys into a tree before evaluating, which is why
`obs.scene == "Intro"` works as a path even though the key is not an identifier.

## Marketplace

Browse, install, update and remove **plugins and widgets**. Modules are
deliberately *not* browsable: they arrive only as a plugin's declared dependency,
so you never install a driver you have no use for. See
[Marketplace](/guide/marketplace).

## Settings

<Screenshot src="/screens/settings.png" caption="Settings" />

- **Language** — the UI locale. Every extension contributes its own translations.
- **Accounts** — connect Twitch and anything else that needs OAuth. See
  [Connecting accounts](/guide/accounts).
- **Capabilities** — review and revoke what each installed module is allowed to
  do. Revoking takes effect immediately.
- **Licences** — paste a licence key to unlock a paid extension.
- **Extension settings** — each plugin and module that declares a settings schema
  gets a generated form here.

## Forms are generated, not written

Everywhere you fill in an action's fields, a plugin's settings, or a widget's
configuration, that form was **generated from a schema** the extension declared.
That is why every extension's UI feels the same, and why a field can be a slider,
a colour picker, a duration, a dropdown or a file path without the extension
shipping any UI code.

Fields that accept templates render as a highlighted input with `@` and `{{ }}`
autocompletion, fed by the state variables every loaded plugin declares.

<Screenshot src="/screens/template-input.png" caption="A template field with @ autocompletion open" />
