# Building from source

## Quick start

```bash
git clone https://github.com/Ascol57/Quicktys && cd Quicktys
./setup/setup-linux.sh      # or setup-macos.sh, or setup\setup-windows.ps1
./setup/run.sh
```

Full detail, per-OS gotchas and what the scripts install:
[Installation](/guide/installation).

## The two build domains

```bash
# Host workspace — 16 crates, from the repo root
cargo check                  # the acceptance gate
cargo build
cargo test -p quicktys-core
cargo test -p quicktys-core some_test_name

# Extensions — EXCLUDED from the workspace, built per crate for wasm32-wasip2
cd plugins/time
cargo build --target wasm32-wasip2 --release
```

Extensions are standalone packages with their own `Cargo.lock`, a different
target and a different crate type. **Never add one to the workspace `members`.**

## Frontend

```bash
cd frontend
npm install
npm run build          # → frontend/dist, embedded by Tauri at build time
```

`tauri.conf.json` deliberately has **no `devUrl`**, so the binary loads the
embedded `frontend/dist` and runs standalone. Rebuild the frontend after UI
changes.

For hot reload: `npm --prefix frontend run dev`, then re-add `devUrl` and
`beforeDevCommand` (or use `cargo tauri dev`).

## Running

```bash
cargo run -p quicktys-app     # standalone
setup/run.sh --solo           # build everything, then run without the registry
```

A GUI run needs a real desktop session — GTK and WebKit will not initialise
headless.

## Tests

```bash
cargo test                                        # host workspace
cargo test -p quicktys-host                       # the ABI integration suites
```

The `quicktys-host` suites are the executable specification of the ABI:

| Test | Covers |
| --- | --- |
| `plugin_time` | the single-tier plugin ABI end to end |
| `two_tier` | plugin ↔ module calls |
| `net_cap` | system capabilities and certification gating |
| `bundles` | loading a real bundle from disk via its descriptor |
| `notify_plugin` | the `reactivity.notify` path to the UI |
| `twitch_chat` | the IRC login, with a stand-in server decoding masked frames |
| `twitch_eventsub` | EventSub subscription creation |
| `pear_auth` | sticky denial, explicit retry, stale-token replay, client id |
| `pear_song_trigger` | poll → event → trigger → reactive variables |
| `ws_close` | that closing a socket really stops delivery |

They build the reference extensions on demand, so the first run is slow.

`quicktys-ipc` is Tauri-free and unit-tested without a WebView.

## Pinned versions

Exact pins (`=`) for reproducible builds:

| | |
| --- | --- |
| `wasmtime` / `wasmtime-wasi` | `=27.0.0` |
| `tauri` / `tauri-build` | `=2.11.5` / `=2.6.3` |
| `wit-bindgen` (guest, per extension) | `=0.36.0` |

The host generates its bindings with **wasmtime's own** Component Model
`bindgen!`; it does not use the `wit-bindgen` crate. `wit-bindgen` is the
**guest** ABI generator, pinned in each extension's own manifest — newer 0.5x
needs the unstable `wasip2` std feature.

## Conventions

- **Documentation prose is French**; code, identifiers and code comments are
  **English**. Match what is around you.
- **Record structural decisions in `ARCHITECTURE.md`.** The document says so
  explicitly, and it is the reason the codebase is legible: every non-obvious
  choice has its rationale written down, including the ones that were wrong the
  first time.
- **Tick and extend the relevant `ROADMAP.md` milestone.**
- **Stay in the current milestone's scope.** The validation path is deliberate:
  prove the plugin ABI on a simple plugin before the two-tier model, before the
  mass port.
- **Never copy CastMate code.** Studying its concepts and APIs is fine; reusing
  its code is not, including the frontend.
- The maintainer handles all `git commit` and `git push`.

## Contributing

Contributions are accepted under a **CLA**: the contributor assigns or licenses
their rights to the owner, who therefore holds the project and can dual-license
it. The CLA text is not written yet.

## Next

- [Architecture](/contributing/architecture)
- [Licensing](/contributing/license)
