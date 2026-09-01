---
description: "Build a Quicktys dashboard: add, move, resize and configure widgets, the Button trigger, and why a countdown is ticked by its widget rather than the host."
---

# Dashboard

The Dashboard is a grid of **widgets** — the boxes that show you what is going on
and give you things to click. It is the tab you land on and the one you cannot
close.

<Screenshot src="/screens/dashboard.png" caption="The dashboard" />

## Editing the layout

Click **Edit**. The grid becomes editable:

| Action | How |
| --- | --- |
| Add | **Add widget** opens a searchable palette. |
| Move | Drag the widget. |
| Resize | Drag the bottom-right handle. It snaps to whole grid cells — widgets grow taller as well as wider. |
| Configure | The widget's ⚙ opens its settings form. |
| Remove | The widget's ✕. |

<Screenshot src="/screens/dashboard-edit.png" caption="Edit mode, with the widget palette open" />

The layout is stored locally on this machine, not in your profiles.

## Configuring a widget

Every widget declares a **config schema**, and the form is generated from it. The
form knows four shapes:

| Schema | Control |
| --- | --- |
| `Boolean` | a switch |
| `Color` | a colour picker |
| `Number` with `min`/`max` | a slider |
| `String` with `enum` | a dropdown |

<Screenshot src="/screens/widget-config.png" caption="A widget's settings" />

::: tip Unset booleans read as ON
An unset boolean renders as ON. Widget options are therefore named `showX`, never
`hideX` — otherwise the switch would lie about its own default.
:::

## The widgets that ship

| Widget | What it does |
| --- | --- |
| **Info box** | A count for a chosen source: plugins, modules, profiles, automations, widgets, overlays. |
| **Button** | A big clickable button that fires the built-in `button.clicked` trigger. |
| **Countdown** | A named countdown timer with formats, colours, a progress ring and a warning threshold. |
| **Alert** | An alert box driven by the Alerts plugin — invisible until fired. |
| **Twitch chat** | Live chat with badges, emotes, colours and several row styles. |
| **Now Playing (Pear)** | Cover, title and a play/pause toggle. |
| **Music (Pear)** | A richer player box: cover, title, artists, progress bar, times. |
| **Queue (Pear)** | The play queue, with the current track highlighted. |

Full configuration tables in [Shipped extensions](/reference/extensions).

## Two kinds of widget

You will not usually notice the difference, but it explains why some widgets look
richer than others:

- **Built-in** widgets are drawn by a component inside the app, selected by the
  `template` field in their bundle (the Info box and the Button).
- **Runtime** widgets ship an `index.html` rendered in a **sandboxed iframe**,
  loaded from a custom `widget://` URI scheme so each one is its own document with
  its own CSP. Everything else on the list is one of these.

Both talk to the app over the same message bridge: the host pushes state and
config in, the widget can ask the host to run an action.

## The Button widget

Worth a section because it is the cheapest way to get a manual trigger.

Clicking it emits the built-in trigger `button.clicked` with context
`{ id, label }`, where `id` is the widget's instance id in the layout. No
WebAssembly is involved — the host publishes it straight onto the event bus.

In Edit mode, the button's **Link an action** tool creates (or opens) a profile
named `button-<id>` with the trigger already bound and already filtered to this
button. From there it is an ordinary [profile](/guide/profiles) binding.

## Names, and where autocompletion comes from

Several plugins take a **name** that points at a widget — the Alerts plugin's
`name`, the Countdown plugin's `name`. Those fields autocomplete with the names
of the matching widgets you have actually placed, collected across **both** the
dashboard and your [overlays](/guide/overlays).

If a name field autocompletes nothing, the widget is not placed anywhere yet.

## Countdowns: who does the ticking

The Countdown plugin holds named timers and publishes `countdown.<name>`, but the
host does **not** tick them. The published value carries an absolute end instant,
and the widget derives the remaining time locally — a second-accurate display at
zero host cost.

That has one visible consequence: the plugin's **Countdown finished** trigger is
reported by the widget when it crosses zero. **A countdown nobody displays never
fires that trigger.** If you need the trigger without showing the timer, place
the widget somewhere and hide it, or trigger the end yourself with the *Signal
countdown end* action.

Timers persist across restarts. A run that expired while the app was closed is
restored as finished and pre-acknowledged, so starting the app never fires a
backlog of alerts.

## Next

- [Overlays](/guide/overlays) — the same widgets, on stream.
- [Shipped extensions](/reference/extensions) — every widget's options.
