---
description: "The Quicktys extension model: plugin, module and widget, what each one may import, the JSON-over-WIT ABI rule, and how extension bundles are laid out."
---

# The extension model

Everything in Quicktys that talks to the outside world is an extension. There are
three kinds, and which one you write is decided by what you need to touch.

## Pick your tier

| | Plugin | Module | Widget |
| --- | --- | --- | --- |
| **Who may publish** | anyone | certified developers, signed | anyone |
| **Language** | Rust → WebAssembly component | Rust → WebAssembly component | HTML/CSS/JS |
| **Sandbox** | wasmtime, own `Store` | wasmtime, own `Store` | browser iframe |
| **Provides** | actions, triggers, state, settings | services, events | a rendered surface |
| **May import** | `logging`, `storage`, `reactivity`, `modules` | the above plus `events`, `oauth`, and system capabilities | nothing — a message bridge |
| **Certification** | not required | **required** for any system capability | not required |

The decision is mechanical:

- **Needs the network, the filesystem, a device, or to start a process?** That is
  a **module**, and it has to be certified.
- **Just user-facing logic that composes other things?** That is a **plugin**.
- **Just pixels?** That is a **widget** — no WebAssembly at all.

A plugin cannot cheat its way up. The system capability interfaces are not in the
plugin WIT world, so a plugin cannot even name them.

## How the two WASM tiers fit together

```
Host (trusted Rust kernel)
  exposes: logging, storage                        (both tiers)
           reactivity, modules                     (→ plugins)
           events, oauth,                          (→ modules)
           net, input, audio, hid, files,
           process, timers                         (→ certified modules only)
   │
   ├─ MODULES (WASM, certified) — drivers / protocol adapters
   │    import: system capabilities
   │    export: services + events
   │
   └─ PLUGINS (WASM, anyone) — user-facing logic
        import: logging / storage / reactivity / modules
        export: actions / triggers / state / settings
```

When a plugin calls a module, the host routes and validates it: the plugin must
have declared the dependency, the method must exist, and the payload must match
the module's published service schema. Events a module emits are fanned out to
subscribed plugins.

## The ABI rule

One rule governs every contract in the system:

> **The typed part of the contract is the WIT function signature. Every
> structured payload — config, context, service arguments, events, schemas, state
> values — is a JSON string, validated by the host against the schema the
> extension declared in its manifest.**

Why not type everything in WIT? Because the schema system is rich and still
evolving — `Duration`, `Color`, `Resource`, `Range`, `MediaFile`. Encoding those
as WIT types would freeze the ABI every time a new one appeared. JSON plus
schema validation keeps the ABI **stable** while the schema model grows.

The contracts live in [`wit/host.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/host.wit)
(capabilities) and [`wit/world.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/world.wit)
(the `plugin` and `module` worlds), package `quicktys:api@0.2.0`.

## Manifests declare, they do not translate

An extension's `init()` returns a manifest describing everything it offers:
actions with their config schemas, triggers with their context schemas, state
keys, settings, and required modules.

The manifest emits **i18n keys, never translated strings**. Human text lives in
the extension's `lang/` folder, which travels beside the `.wasm` as data. See
[Translations](/dev/i18n).

## Bundles

An extension is not loaded from a raw `.wasm` at a hardcoded path. It is loaded
from a **bundle** — a self-describing directory:

```
plugins/<id>/
├── <id>.wasm        the component
├── lang/            locales (en.json, fr.json, …)
├── ui/              optional: HTML for UI contributions
└── bundle.json      { id, kind: "plugin", wasm, certified }
```

Widgets have no `.wasm` — their bundle is authored, not compiled.

The host scans two roots: `dist/bundles` (development) then
`<user-data>/quicktys/installed/` (marketplace). **Modules load before plugins**,
because plugins depend on them. An id already loaded is skipped, so an installed
bundle shadows a development bundle with the same id.

## What to read next

| You want to | Go to |
| --- | --- |
| Set up a toolchain and build something | [Dev environment](/dev/environment) |
| Write user-facing logic | [Write a plugin](/dev/plugin) |
| Write a driver that talks to a protocol or device | [Write a module](/dev/module) |
| Draw something on the dashboard or on stream | [Write a widget](/dev/widget) |
| Add a button or a whole tab to the app | [UI contributions](/dev/ui) |
| Ship it | [Package & publish](/dev/packaging) |

## Licensing, briefly

The guest SDKs (`quicktys-plugin-sdk`, `quicktys-module-sdk`) are **MIT**, so
nothing copyleft attaches to your extension through them. The core is AGPL-3.0
with an **extension exception**: an extension that talks to the core solely
through the official ABI is not a derivative work and may be licensed however you
like, including closed and commercial.

The extensions in the Quicktys repository are proprietary and source-available —
read them, learn from how they call the ABI, but write your own rather than
copying them. Details in [Licensing](/contributing/license).
