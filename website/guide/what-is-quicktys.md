---
description: "What Quicktys is, its three tiers of extension trust, and what it changes versus CastMate: a real automation model, analysable expressions, translations everywhere."
---

# What is Quicktys?

Quicktys is a desktop application that reacts to things happening around your
stream. Someone follows on Twitch, a song changes, a scene switches in OBS, you
press a button on your dashboard — Quicktys runs a sequence you built: change the
scene, play a sound, post in chat, show an alert on your overlay, wait three
seconds, do two things at once.

That idea is not new. [CastMate](https://github.com/LordTocs/CastMate) pioneered
it and Quicktys is an unashamed descendant of its concepts. What is new is
everything underneath: Quicktys is a **clean-room rewrite in Rust**, where every
integration is a **sandboxed WebAssembly extension** rather than a JavaScript
module with full run-of-the-house privileges.

## The core idea: three tiers of trust

The single design decision that shapes the whole product is this: **the code
anybody can publish is the code that can do the least.**

| | Who writes it | Sandbox | What it can reach |
| --- | --- | --- | --- |
| **Plugin** | anyone | WebAssembly | Only safe capabilities: logging, its own key-value storage, reactive state, and calls into modules. **No network, no filesystem, no OS.** |
| **Module** | certified developers, signed | WebAssembly | System capabilities — `net`, `input`, `audio`, `hid`, `files`, `process`, `timers` — and only those it declared and you consented to. |
| **Widget** | anyone | browser sandbox | Nothing. It is frontend code in an isolated iframe, fed state through a message bridge. |

A plugin is the part you interact with: it declares **actions** ("send a chat
message"), **triggers** ("a new follower arrived") and **state** ("the current
song title"). A module is a *driver*: it knows how to speak obs-websocket, or the
Twitch IRC and Helix protocols, or how to decode an MP3. A widget is what shows
up on your dashboard or on stream.

When the Twitch plugin wants to send a chat message, it cannot open a socket — it
is not allowed to. It calls a service on the certified Twitch module, and the
host validates that the plugin declared that dependency, that the service exists,
and that the payload matches the module's published schema. Every hop is
mediated.

::: tip Why this matters in practice
A malicious or simply buggy plugin cannot exfiltrate your stream key, read your
`~/.ssh`, or install anything. The blast radius of "I installed a random plugin
someone posted" is bounded by design rather than by trust.
:::

## What ships with the core: nothing

The core application contains **zero modules and zero widgets**. Installing a
plugin makes the host resolve its declared dependencies (`requires: obs@^1`) and
download exactly those certified modules, showing you a consent screen for the
system capabilities they ask for. Nothing you did not ask for is ever fetched,
and a module no plugin uses any more can be removed.

## What is different from CastMate

Quicktys keeps the concepts CastMate got right — profiles, triggers, sequences,
action queues, the visual timeline — and rebuilds the parts that were hitting
their ceiling.

**A real automation model.** A CastMate sequence is a flat list of actions with
some special cases. A Quicktys automation is a *tree of typed nodes*: `if`,
`switch`, bounded `loop`s, `try`/retry, `wait-for`, local variables with `let` and
`set`, and `parallel` with a join policy. "Start three things, wait for all of
them, then continue" is a first-class node instead of something you fake.

**Expressions you can analyse.** CastMate evaluates templates by compiling
arbitrary JavaScript. Quicktys uses [Rhai](https://rhai.rs), sandboxed and
evaluated host-side on an already-materialised context. Because an expression is
a syntax tree rather than opaque JS, the host can extract which variables it
reads — which is what makes targeted reactive re-evaluation, edit-time validation
and autocompletion possible at all.

**Translations everywhere.** In CastMate only plugins had locales, imported into
the JS bundle. In Quicktys the core, every plugin, every module and every widget
carries a `lang/` folder of per-locale JSON, discovered by the host and namespaced
automatically. Manifests emit **keys**, never translated strings, and the locales
travel as data next to the `.wasm`, so a translator never touches code.

**A marketplace built in from the start.** Signed packages, a static signed
registry you can self-host, dependency resolution, offline-verifiable licences,
per-capability consent that takes effect immediately, and post-quantum hybrid
signature chains. Free and paid extensions were designed for on day one, even if
the launch is free.

## What Quicktys is not

- **Not a CastMate fork.** It shares zero lines of code with it, deliberately.
  Concepts and APIs were studied; code never was. That constraint is why the
  frontend is a from-scratch Vue 3 + PrimeVue app rather than a reuse of
  CastMate's UI library.
- **Not cloud software.** It runs entirely offline with no account. An account
  only ever unlocks optional extras — multi-machine sync, paid marketplace
  purchases, a remote relay. No local feature is gated behind it, and there is no
  mandatory telemetry.
- **Not finished.** See the [roadmap](/contributing/roadmap) for the state of
  each milestone.

## Where to go next

- [Installation](/guide/installation) — get it running on your machine.
- [Your first automation](/guide/getting-started) — a ten-minute walkthrough.
- [The extension model](/dev/overview) — if you would rather build than use.
