---
description: "The honest state of every Quicktys milestone, from the host kernel and both extension ABIs through the marketplace to the remaining mass port."
---

# Roadmap

Migration from [CastMate](https://github.com/LordTocs/CastMate) is **incremental**
— every milestone is shippable and testable. The source of truth is
[`docs/ROADMAP.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/ROADMAP.md).

::: info The checklist lags the code in places
M5 is a good example: its boxes are unticked, but the overlay system, the widget
extension type and the local server are implemented and in daily use. Where this
page and the checklist disagree, this page reflects what the code actually does.
:::

## M0 — Scaffolding & architecture ✅

Repository layout, Cargo workspace, the two-tier WIT contract, the architecture
document, exact version pins, `cargo check` green across the host crates.

## M1 — Host kernel ✅

- `quicktys-schema` — schemas, validation, YAML/JSON, template regions, the node
  model.
- `quicktys-expr` — the sandboxed Rhai engine with operation, depth and size
  limits, static dependency analysis, schema coercion.
- `quicktys-core` — resources with a YAML-mirrored store, the **automation
  runner** over every node type with cooperative abort and structured
  concurrency, reactivity (`StateStore` with dependency-targeted re-evaluation),
  the trigger bus, profiles, the dispatcher, action queues, i18n, the extension
  registry with dependency resolution and certification gating.

Remaining: lexical scopes, a compiled-expression cache.

## M2 — Plugin ABI end to end ✅

Host bindings against the `plugin` world, the four safe capabilities implemented,
`quicktys-plugin-sdk` with the `define_plugin!` macro, and a plugin compiled to a
WebAssembly component, loaded and executed end to end with its state observed
host-side.

## M3 — Tauri shell & UI ✅

`quicktys-ipc` as a Tauri-free, unit-tested façade, plus the from-scratch Vue 3 +
PrimeVue frontend: dashboard, profiles, the automation editor with live execution
highlighting, variables, marketplace, settings.

## M4 — Module tier ✅

Host bindings against the `module` world, `quicktys-module-sdk`, the validated
plugin↔module linker, certification gating, and the system capabilities wired for
real: HTTP and WebSocket (including TLS), audio playback and TTS, process
spawning, input injection, persistent storage, OAuth token storage with
transparent refresh.

Real drivers followed: `obs`, `twitch`, `pear`, `sound`, `os`, `input`, `http`.

Remaining: `audio.play-buffer`, `files`, `hid` and `timers` — declared in the ABI,
wired when a driver needs them.

## M5 — Overlays & widgets ✅ *(checklist not updated)*

Implemented: the local HTTP server, SSE state streaming to browser sources, the
third extension type, the WYSIWYG overlay editor with real live previews and
snapping, widget isolation in its own URI scheme, and the dashboard grid.

Eight widgets ship. See [Overlays](/guide/overlays) and
[Dashboard](/guide/dashboard).

## M6 — Marketplace, DRM & distribution ✅

All ten sub-milestones done:

| | |
| --- | --- |
| M6.1 | the `.qtpkg` format and manifest |
| M6.2 | the install runtime — atomic materialisation, lockfile, hot-load |
| M6.3 | semver and dependency resolution |
| M6.4 | `quicktys-crypto` — the hybrid post-quantum suite |
| M6.5 | signature chains, trust store, CRL, computed certification |
| M6.6 | the static signed registry, transport-agnostic |
| M6.7 | per-capability consent, enforced at every dispatch |
| M6.8 | offline-verifiable entitlements and licence keys |
| M6.9 | encrypted payloads, decrypted in memory at load |
| M6.10 | forensic watermarking |

Remaining: the **online activation service** — device binding through the OS
keystore, seats, subscription grace. The hybrid `seal`/`open` crypto is already in
`quicktys-crypto`; what is missing is the server.

## M7 — Buttons ✅

A design that was **implemented and then removed**: a generic remote surface
protocol (a web deck, pairing, a Stream Deck driver over `hid`) turned out to be
too much plumbing for the value. It was replaced by a simple **Button widget** on
the dashboard.

The event→profile model is identical; only the surface changed. The `hid`
capability is kept as generic infrastructure, unused by any shipped extension.

::: tip Worth reading as a design lesson
The removal is documented rather than quietly reverted. A remote surface may
reappear later as a runtime widget served by the overlay server — which would
cost almost nothing, because the widget infrastructure already exists.
:::

## M8 — Migration & mass port 🔨

**Done:** the `quicktys-migrate` tool. It reads a user's CastMate YAML, writes
Quicktys resources valid by construction, and produces a Markdown report.
Deliberately, it attempts **no** automatic JavaScript → Rhai translation, and
reports anything ambiguous rather than guessing.

**In progress:** porting CastMate's ~30 plugins, in batches of increasing
complexity. Batch 1 (`random`, `twitch`) is done, and the extensions that have
followed are visible in `scripts/build-bundles.sh`.

## What is not done

Being plain about it:

- **No release binaries.** You build it yourself today.
- **The certification programme is not open.** The signing chain works and is
  exercised by the development keys, but the authority and review process are
  still being set up.
- **No online activation service.**
- **Tokens are in plain JSON**, not the OS keyring.
- **The data directory path is hardcoded** and not idiomatic on macOS or Windows.
- **`files`, `hid`, `timers` and `audio.play-buffer`** are declared but not
  implemented. The three-second module poller stands in for `timers`.
- **Widget settings forms exist twice** — dashboard and overlay editor — with
  separate code that can drift.
- **The CLA is unwritten** and the licence texts have not had a legal review.

## Next

- [Architecture](/contributing/architecture)
- [Licensing](/contributing/license)
