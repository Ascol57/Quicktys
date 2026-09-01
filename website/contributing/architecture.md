# Architecture

The binding reference is [`ARCHITECTURE.md`](https://github.com/Ascol57/Quicktys/blob/main/ARCHITECTURE.md)
in the repository — it is in French, and every structural decision gets recorded
there. This page is the map.

## The shape of it

```
┌────────────────────────────────────────────────────────────────────┐
│                          Tauri application                          │
│                                                                     │
│  ┌──────────────────────────┐        ┌────────────────────────────┐ │
│  │   WebView (frontend)     │  IPC   │        Rust host           │ │
│  │   Vue 3 + PrimeVue       │◄──────►│  quicktys-app (boot+Tauri) │ │
│  │   - schema-driven forms  │ cmds/  │  quicktys-ipc (bridge)     │ │
│  │   - i18n (vue-i18n)      │ events │  quicktys-core (engine)    │ │
│  └──────────────────────────┘        │  quicktys-schema           │ │
│                                      │  quicktys-host (wasmtime)  │ │
│                                      └──────────────┬─────────────┘ │
│                          system capabilities        │ WIT ABI        │
│                        ┌────────────────────────────┼──────────────┐ │
│                        │   WASM MODULES (certified) ▼              │ │
│                        │   obs · twitch · pear · sound · input     │ │
│                        │   import: net/input/audio/hid/…           │ │
│                        │   export: services + events               │ │
│                        └───────────────┬──────────────────────────┘ │
│                     services/events    │ (host wires and validates)  │
│                        ┌───────────────▼──────────────────────────┐ │
│                        │   WASM PLUGINS (anyone, sandboxed)       │ │
│                        │   actions · triggers · state · UI        │ │
│                        │   import: logging/storage/reactivity/    │ │
│                        │           modules   (NO net/OS)          │ │
│                        └──────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────┘
```

The host is the **only** fully-trusted zone.

## The crates

Sixteen crates in the host workspace. Extensions are **excluded** from it — they
build for `wasm32-wasip2` as `cdylib`s with their own lockfiles.

### Engine

| Crate | Role |
| --- | --- |
| `quicktys-schema` | Typed schemas: validation, YAML/JSON (de)serialisation, template regions, and the automation **node model** (shared with the UI). |
| `quicktys-expr` | The sandboxed **Rhai** engine: compilation, static analysis (dependency extraction from the AST), schema coercion. |
| `quicktys-core` | The domain engine: extension registry, resources, profiles, the **sequence runner**, action queues, reactivity, the event bus, i18n, persistence. |
| `quicktys-host` | The wasmtime runtime: loading components, capabilities, certification gating, the plugin↔module linker. |

### Application

| Crate | Role |
| --- | --- |
| `quicktys-ipc` | The host ↔ WebView command and event catalogue. **No Tauri dependency**, so it is unit-testable without a WebView. |
| `quicktys-app` | The binary: boot, the Tokio runtime, the Tauri shell. Thin `#[tauri::command]` shims plus wiring. |

### Guest SDKs — MIT

| Crate | Role |
| --- | --- |
| `quicktys-plugin-sdk` | Guest SDK for plugins. |
| `quicktys-module-sdk` | Guest SDK for modules. |

### Distribution

| Crate | Role |
| --- | --- |
| `quicktys-pkg` | The `.qtpkg` format and its manifest. |
| `quicktys-install` | Materialising a package into `installed/`, atomically. Host-agnostic. |
| `quicktys-crypto` | The hybrid post-quantum suite: signatures, sealing, watermarking. |
| `quicktys-cert` | Signature chains, trust store, entitlements, licence keys. |
| `quicktys-registry` | Fetching, caching, searching and resolving a registry. Transport-agnostic — HTTP is injected through a `Fetcher` trait, so it is testable offline. |
| `quicktys-regserve` | The registry server. *Proprietary.* |
| `quicktys-pack` | Packing, signing, licence issuing. *Proprietary.* |
| `quicktys-migrate` | The CastMate migration tool. |

## Decisions worth knowing

### JSON over typed WIT

The typed part of the ABI is the WIT signature; payloads are JSON validated
against declared schemas. Encoding the rich schema types in WIT would freeze the
ABI every time a new one appeared.

### Rhai, host-side, on a materialised context

Templates are not arbitrary JavaScript. Because an expression is a syntax tree,
the host extracts what it reads — and that is what buys targeted reactive
re-evaluation, edit-time validation and autocompletion.

Evaluation is synchronous, on an already-materialised context, so no I/O happens
during evaluation.

### Two template syntaxes, deliberately not Rhai

Action config fields use `@path` and Handlebars, not Rhai. The two are separate
languages for separate jobs: producing text versus producing a decision.

The one place they converge: `ExprEngine::eval` nests dotted keys through the same
`nest_context` the Handlebars layer uses, and `compile()` strips a leading `@`.
So `obs.scene == "Intro"` and `@obs.scene == "Intro"` both work, and what
autocompletion inserts into an action field pastes unchanged into a condition.

### Reactivity on `tokio::sync`, not a signals framework

Dependency tracking here is **explicit** — extracted from the Rhai AST — so an
auto-tracking signals framework would be the wrong tool. The `StateStore`
broadcasts changed keys over a `broadcast` channel behind an `RwLock`, and
`wait_until` re-evaluates only when a key in the expression's dependency set
changes.

Because dependencies are **root identifiers** (`obs`) while keys are dotted
(`obs.scene`), matching is by root segment. Exact matching would have meant
`wait-for until` never woke.

### 100% WASM, no native escape hatch

There are no out-of-process native modules. If exotic hardware exceeds what the
host capabilities expose, the answer is to **extend the host capabilities** in
Rust, inside the trusted zone — not to open a hole in the sandbox.

### Local-first

The app is fully functional offline with no account. An account only unlocks
optional extras: multi-machine sync, paid marketplace purchases, a remote relay.
No local feature is gated behind it, and there is no mandatory telemetry.

### The clean-room constraint

Quicktys contains **zero lines** of [CastMate](https://github.com/LordTocs/CastMate)
code. Concepts and APIs were studied; code never was. That is why the frontend is
a from-scratch Vue 3 + PrimeVue application rather than a reuse of CastMate's UI
library — the stack is shared, the code is not.

Reading a user's CastMate YAML for migration is fine: that is their data, not
CastMate's code.

## Open questions

Recorded honestly in `ARCHITECTURE.md` §16:

- **Legal review.** The extension exception and the CLA still need a lawyer. If
  the extension boundary is badly defined, a plugin could be reclassified as
  derivative — or the reverse.
- **`quicktys-schema` is linked into every extension.** The SDKs re-export
  `Schema`, so this AGPL crate is compiled into extensions. That is the same
  problem the exception solves for the SDKs, but the crate is not named in it. It
  needs to be added to the ABI list or moved to MIT.
- **Capability granularity** — fine enough for security, simple enough for module
  authors.
- **JSON ABI performance** — high-frequency actions may need a binary path or
  static component linking.
- **Component Model maturity** — wasmtime and wit-bindgen versions are pinned
  exactly and tracked.
- **Certification infrastructure** — the authority, the review process, key
  revocation.

## Reading order

1. [`ARCHITECTURE.md`](https://github.com/Ascol57/Quicktys/blob/main/ARCHITECTURE.md) — the binding reference.
2. [`docs/automation-engine.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/automation-engine.md) — the runner, node model and expressions.
3. [`docs/marketplace.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/marketplace.md) — packages, registry, signing, resolution.
4. [`docs/drm.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/drm.md) — entitlements, encryption, watermarking, and an honest threat model.
5. [`docs/ROADMAP.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/ROADMAP.md) — milestones.

All in French.

## Next

- [Building from source](/contributing/building)
- [Roadmap](/contributing/roadmap)
