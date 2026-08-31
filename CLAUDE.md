# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Quicktys is a clean-room rewrite of CastMate (a streamer automation tool) in **Rust + Tauri + WebAssembly plugins**. It is at milestone **M0 — scaffolding only**: the crates are stubs (`pub mod x {}` with doc comments), the WIT contracts and architecture are defined, but there is no functional runtime yet. The substance lives in three docs, all in **French**:

- [ARCHITECTURE.md](ARCHITECTURE.md) — the binding design reference. Read it before any non-trivial work; structural decisions are recorded here and new ones must be added.
- [docs/ROADMAP.md](docs/ROADMAP.md) — milestones M0–M8 with concrete checklists. Use it to know what belongs in the current milestone.
- [docs/automation-engine.md](docs/automation-engine.md) — the automation runner / node-model / expression design.

## Two hard constraints (do not violate)

1. **Clean-room — never copy CastMate code.** This repo lives inside the CastMate tree (`/home/constant/CastMate`, parent dir is `../`, gitignored as `/Quicktys`) but must contain **zero lines** of CastMate code. Studying CastMate's *concepts/APIs* is fine; reusing its *code* is not — including the frontend (no vendored `castmate-ui-core`; reuse the Vue 3 + PrimeVue *stack*, not the code). Reading a user's CastMate YAML data for migration is fine — that's their data, not CastMate code.
2. **License.** Core is `AGPL-3.0-only` + the Quicktys Extension Exception + CLA (ARCHITECTURE §0bis). The texts now exist: [LICENSE](LICENSE) (AGPL verbatim), [LICENSE-EXCEPTION](LICENSE-EXCEPTION), [LICENSE-EXTENSIONS](LICENSE-EXTENSIONS), mapped in [COPYING.md](COPYING.md); only the CLA is still unwritten. Crate `license` fields are deliberately not MPL. *Third-party* extensions are independently licensable and may be closed/commercial (that's what the exception buys) — but the reference extensions **in this repo** are **proprietary, source-available, © Ascol57**: their `Cargo.toml` declares `license = "LicenseRef-Ascol57-Proprietary"`, not MIT. Keep new reference extensions on that same id, and keep the guest SDKs (`quicktys-plugin-sdk`/`quicktys-module-sdk`) inside the exception's "Official Extension ABI" §0(b) — they are linked into every extension, so dropping them there would make every extension a derivative of an AGPL crate.

The user handles all `git commit`/`git push` himself — do not run them.

## Build & check

Two separate build domains:

```bash
# Host workspace (the 8 crates/* — see Cargo.toml `members`)
cargo check                 # the M0 acceptance gate ("cargo check vert sur tous les crates")
cargo build
cargo test -p quicktys-core # run one crate's tests
cargo test -p quicktys-core some_test_name   # run a single test

# WASM extensions — EXCLUDED from the host workspace (Cargo.toml `exclude`),
# built individually for the wasm32-wasip2 target. They are standalone packages
# (own Cargo.lock), so build from the crate dir, not with `-p` from the root:
(cd plugins/time && cargo build --target wasm32-wasip2 --release)   # -> a Component
# (wasm32-wasip2 target is installed. wit-bindgen is pinned to =0.36.0 in the
#  plugin: newer 0.5x pulls crates needing the unstable `wasip2` std feature.)
```

**Bundling.** The desktop app does not load raw `.wasm` from dev `target/` paths; it loads
**bundles** — self-contained dirs (`<id>.wasm` + `lang/` + `bundle.json`) built by
`scripts/build-bundles.sh` into `dist/bundles/{plugins,modules,widgets}/<id>/` (gitignored).
Boot scans `dist/bundles` (dev) then `installed/` (marketplace), modules before plugins. Edit
the `EXTENSIONS`/`WIDGETS` lists in the script to change what ships (`time` is intentionally
excluded — its `delay` doesn't wait). Run `scripts/build-bundles.sh` after changing an
extension, then `cargo run -p quicktys-app`. `cargo test -p quicktys-host --test bundles` loads
a real bundle from disk via its descriptor (ARCHITECTURE §9.1).

**Widgets** are the third extension type — **frontend-only, no WASM** (dashboard boxes /
overlays). A widget bundle is `widgets/<id>/{bundle.json, lang/}` (authored, not compiled); the
script just copies it. `bundle.json` has `kind:"widget"`, `name`, `icon`, a `template` (which
in-app renderer draws it) and a `config` schema (its settings). The host does NOT run widgets;
it catalogs them (scans `<root>/widgets`, loads lang namespaced `widget.<id>.*`, embeds any
`index.html` into the manifest) and exposes the manifests via `list_widgets`. The frontend
Dashboard (`DashboardView.vue`) is a customizable grid: add/reorder/resize/configure widgets
(Edit mode), layout persisted in localStorage (`stores/dashboard.js`). Two widget kinds:
**built-in** (`template` → an in-app Vue component, e.g. `info` → `InfoWidget.vue`, a count box
for a chosen source) and **runtime** (ships `index.html` → rendered by `WidgetFrame.vue` in a
**sandboxed iframe**, `sandbox="allow-scripts"`). Native loads the iframe from a custom
**`widget://localhost/<id>/`** URI scheme (registered in `main.rs::register_uri_scheme_protocol`,
serves files from the widget bundle dir) so each widget is its **own document with no inherited
CSP** — inline `<script>` runs (a `srcdoc` iframe would inherit the app CSP, whose Tauri nonce
blocks inline scripts); the browser mock, which has no strict CSP, falls back to `srcdoc` with
the host-embedded HTML. `WidgetFrame` picks `widget://localhost/<id>/` or (Windows)
`http://widget.localhost/<id>/`. `frame-src` in the CSP allows `widget:`. Dashboard widgets are
sized by **dragging the bottom-right handle** (snaps to whole grid cells: `w` cols × `h` rows,
so widgets can grow taller too); "Add widget" is a searchable palette (Popover) like the
action/trigger pickers. The runtime bridge (postMessage): host →
widget `{source:"quicktys",type:"state",state}`; widget → host `{source:"quicktys-widget",
type:"ready"|"action",plugin,action,config}` (an action runs as a one-node sequence via
`run_sequence`). Reference widgets: `widgets/pear-now-playing` (cover + title + play/pause
toggle driven by `pear.playing`, from `/song`'s `isPaused`, + next/previous) and
`widgets/pear-queue` (the play queue, current track highlighted). The `pear` module's `poll`
emits `song` on a title **or** pause change (so `pear.playing` tracks the player),
`song-changed` only on a real title change; on a title change it also fetches the queue
(**`GET /api/v1/queue`** — the current endpoint; `/queue-info` is deprecated — parsed
defensively across YouTube-renderer shapes into `pear.queue` = `[{title,artist,thumbnail,
videoId,selected}]`, with a fallback across endpoints + console logging). Verify the API
against the pear-desktop repo, don't guess. `list_plugins`/`list_modules`/`list_widgets` feed
the dashboard counts + widget catalogue.

Reference extensions used by the host integration tests (built on demand): `plugins/time`
(plugin, no module), `plugins/notify` (a plugin with `log`/`toast` actions — `toast` uses the
`reactivity.notify` host cap forwarded to the UI; `tests/notify_plugin.rs`), `modules/counter`
+ `plugins/counter-client` (a module and a plugin that calls it), `modules/netdemo` (a
certified module using the `net` system cap), and
`modules/pear` + `plugins/pear` — a **real** integration: the `pear` module drives Pear
Desktop / YouTube Music's local HTTP API (`/api/v1` on `:26538`, bearer token from
`/auth/{id}`) over `net.fetch`, and the `pear` plugin exposes Play/Pause/Next/… actions.
The `pear` module's endpoint is overridable via its `set-base-url` service (tests point it
at a stand-in server so they never touch a real running player). The `pear` plugin also
declares a **`song-changed` trigger**: the module's `poll` service diffs the current song and
emits two module events — `song` on **any** change **including the first observation** (so
launching Quicktys does NOT fire the trigger; it just syncs state), and `song-changed` only on
a change from a **known** previous song. The plugin (subscribed to both on load) mirrors `song`
into state and bridges `song-changed` to `reactivity.emit-trigger`, so the host fires
`pear.song-changed` on the trigger bus (what a profile binding listens for).
`tests/pear_song_trigger.rs` proves the full path. The desktop
app makes it **live**: boot spawns a poller thread that calls the `poll` service of every
module declaring one (every 3s), and wires the `Dispatcher` (`run_forever(bus)`, sharing the
backend's `Arc<RwLock<ResourceStore<Profile>>>` via `AppBackend::profiles_store`) so a matching
profile binding runs its inline sequence. So: song changes → poll → `song-changed` → trigger →
profile sequence runs. (A real `timers` cap would later replace the poller thread.)
On each song change the plugin also mirrors the song's fields into **`pear.*` reactive
variables** (`title`, `artist`, `artists` list, `cover`, `url`, `album`, `video-id`, `lyrics`,
`playlist`) via `reactivity.set-state` — the host namespaces them `pear.<key>` in the
`StateStore`, so they show in the Variables view and are usable in templates.
`tests/pear_song_trigger.rs::song_change_exposes_reactive_variables` covers this.

**Templating in action configs (decision).** Two syntaxes, resolved before an action is
invoked (`sequence.rs::resolve_config` → the `template` module, `template.rs`), over the flat context (base +
dotted state keys like `pear.title` + locals): **`@name.path`** — a simple variable, only at
start-of-string or after whitespace (so `a@b.com` is left alone); a lone `@path` keeps the
**typed** value (a list stays a list), otherwise it interpolates the display string. **`{{ }}`**
— **Handlebars** (the `handlebars` crate), with the flat keys nested into a tree so
`{{pear.title}}` / `{{#each pear.playlist}}…{{/each}}` work; missing values render empty. `@`
runs first, then Handlebars. This is NOT Rhai — conditions/filters/loops stay Rhai `Expr` with
no `{{ }}`. The editor renders template string fields (`schema.template`) with a highlighted
`@`/`{{`-autocomplete input (`TemplateInput.vue`; helper catalogue in `helpers.js`), fed by each plugin's declared `state` variables
(now surfaced in `PluginInfo.state`). Covered by `sequence.rs` tests
(`at_variable_and_handlebars_over_dotted_state`).
`cargo test -p quicktys-host` exercises the single-tier ABI (`tests/plugin_time.rs`), the
two-tier plugin↔module ABI (`tests/two_tier.rs`), and the system-capability path +
certification gating (`tests/net_cap.rs`) end to end. Gotcha: the cdylib wasm artifact
replaces `-` with `_` (package `counter-client` → `counter_client.wasm`).

Host system capabilities (`crates/quicktys-host`): `net.fetch` is real — a small HTTP client in
`src/http.rs` supporting both **`http://`** (plain `TcpStream`) and **`https://`** (rustls +
bundled webpki-roots, `ring` provider; e.g. Discord webhooks / Twitch). It frames the response by
**`Content-Length`/chunked** and does NOT half-close the request write side — a server running
an async handler (e.g. Pear's `/api/v1/queue`, which round-trips over IPC) abandons the response
if the client sends FIN first, and keep-alive servers never close, so reading-until-EOF hung/
returned empty (that's why the queue widget was blank; regression tests in `http.rs`). `net.send`
(WS write) is real too (`ws.rs::WsSender`, used by the obs Identify/requests). **`audio.play-file`
is real** — decodes wav/mp3/ogg/flac and plays host-side via `rodio` on a dedicated thread that
owns the (!Send) output stream (`src/audio.rs`; graceful no-op when there's no device). The
**`storage` cap persists** to `<data>/storage/<id>.json` per extension (`WasmHost::set_storage_dir`).
**`process`** is real — `spawn`/`kill`/`run-shell`/`open-path` cross-platform via `cfg!`
(`src/process.rs`; the `os` module/plugin). **`input`** is real — `type-text`/`key-combo`/
`mouse-click`/`move` via `enigo` (x11rb backend on Linux, no libxdo; `src/input.rs`; the `input`
module/plugin; needs a desktop session). OAuth is real (token store + transparent refresh in
`src/oauth.rs`). Remaining stubs: `audio.play-buffer`, the streaming socket calls, and the other
device caps (hid/files/timers) — wired when a driver needs them. A module reaches the system caps only if it is **certified**
(uncertified modules that declare capabilities are refused at load — but `storage`/`logging`/
`events` are safe and need no certification, e.g. the `webview` module). `wss`, streaming
`ws-connect` → `on-host-event`, and the interactive browser `authorize` flow remain.

Reference net-driver pattern: a **plugin can't touch the network** (safe caps only), so anything
needing HTTP goes through a **certified module**. The generic **`http` module** (`request
{method,url,headers,body}` → `{status,body}`) is reusable by any plugin that declares
`requires: http` — e.g. the **`discord` plugin** (no Discord-specific module) sends an incoming
webhook by POSTing JSON through it. The **`sound`** module+plugin is the two-tier audio example
(`sound` plugin → `sound` module holding the `audio` cap → `audio.play-file`).

Extensions are a different compile target and crate-type (`cdylib`); never add them to the host workspace `members`. The host *loads* the resulting `.wasm` component at runtime via wasmtime.

```bash
# Frontend (Vue 3 + PrimeVue + Vite, in frontend/) — its own npm project
(cd frontend && npm install && npm run build)   # -> frontend/dist (embedded by Tauri)

# Desktop app (Tauri shell) — embeds frontend/dist at build time
cargo run -p quicktys-app        # standalone; loads the built frontend, no dev server
```

`tauri.conf.json` deliberately has **no `devUrl`**, so the binary loads the embedded
`frontend/dist` and runs standalone — rebuild the frontend after UI changes. For HMR dev,
run `npm --prefix frontend run dev` and re-add `devUrl`/`beforeDevCommand` (or use
`cargo tauri dev`). The IPC layer (`quicktys-ipc::AppBackend`) is Tauri-free and unit-tested;
`quicktys-app` is only thin `#[tauri::command]` shims + boot. A GUI run needs a real desktop
session (GTK/webkit2gtk); it won't initialize under a headless/flaky display.

`tauri.conf.json` `security.csp` is set (was `null`): `img-src`/`media-src` allow the Pear
player (`http://127.0.0.1:26538`) + YouTube Music cover hosts (`*.googleusercontent.com`,
`*.ytimg.com`, `*.ggpht.com`); `frame-src` allows `widget:` (runtime widgets load from the
custom scheme, see Widgets above, so they're NOT inline in the app document); `script-src`/
`style-src` keep `'unsafe-inline'` for the Vue/PrimeVue app. **Untested under the native webview
here** (headless); if the webview's IPC breaks, verify/relax it in `cargo run -p quicktys-app`
(fallback: `"csp": null`).

## Architecture in one screen

**Trust model — the host is the only fully-trusted zone.** Three extension types:

| Type | Who writes it | Sandbox | Capabilities |
| --- | --- | --- | --- |
| **Plugin** | anyone | WASM | safe only: `logging`, `storage`, `reactivity`, `modules`. NO direct net/OS/device access. Provides actions/triggers/state; consumes module services. |
| **Module** | certified devs (signed) | WASM | system caps: `net`, `input`, `audio`, `hid`, `files`, `process`, `timers`. A *driver* ("how to talk to OBS/Twitch/audio"). Exposes typed services + events. |
| **Widget** | anyone | browser sandbox | frontend only, no WASM. Overlay/dashboard served over local HTTP, fed state via websocket. |

The core ships **no** modules/widgets; everything is resolved/downloaded on demand (a plugin pulls the certified modules it requires). A plugin reaching a module goes through the host (`modules.call`/`subscribe`), which validates the dependency, method, and payload.

**ABI rule:** the typed contract = WIT function signatures; all structured payloads (config, context, services, events, schemas, state values) are **JSON strings** validated by the host against schemas the extension declares in its manifest. This keeps the WIT ABI stable while the schema system evolves. The contracts live in [wit/host.wit](wit/host.wit) (host capabilities) and [wit/world.wit](wit/world.wit) (the `plugin` and `module` worlds), package `quicktys:api@0.2.0`. WIT conventions to respect when editing these: the `package` declaration must be the **first** line of each file (a leading comment block is parsed as a doc-comment on the package, and two files with one each is an error); same-package interfaces are imported by short name (`import logging;`, not `import quicktys:api/logging;`); and identifiers can't shadow WIT keywords (e.g. the directory-listing func is `list-dir`, since `list` is the `list<T>` type). Host-side bindings: `wasmtime::component::bindgen!` in `quicktys-host`; guest-side: `wit_bindgen::generate!` runs in the plugin crate via `define_plugin!`.

**Host crates** (`crates/`, host workspace):

- `quicktys-schema` — typed schemas (validation, YAML/JSON (de)ser, template regions, automation `Node` model)
- `quicktys-expr` — sandboxed **Rhai** expression engine for **conditions/values/iterables** (filters, activation, `if`/`loop`, `let`/`set`) — host-evaluated, deterministic. String **templating in action configs** is separate (see below), not Rhai.
- `quicktys-core` — domain engine: extensions registry, resources, profiles, **sequence** runner (tree-walking interpreter over the node model, structured concurrency + abort), queues, reactivity, bus, i18n, persistence
- `quicktys-host` — wasmtime runtime: load/run components, capabilities, certification gating, plugin↔module linker
- `quicktys-ipc` — host↔WebView command/event catalog (Tauri)
- `quicktys-app` — binary: boot, Tokio runtime, Tauri shell (Tauri itself added in M3)
- `quicktys-plugin-sdk` / `quicktys-module-sdk` — guest SDKs for writing plugins / modules

**Decentralized i18n (ARCHITECTURE §10):** every "thing" carries its own `lang/` folder of per-locale JSON — core (`lang/`, namespace `core.*`), each module (`modules/<id>/lang/`, `module.<id>.*`), each plugin (`plugins/<id>/lang/`, `plugin.<id>.*`), each widget (`widget.<id>.*`). Manifests emit **keys only**, never translated strings; the host discovers each `lang/` and namespaces it by id. `en.json` is the reference/fallback locale. Locales are data shipped beside the `.wasm`, never compiled in.

## Working conventions

- When you make a structural decision, record it in ARCHITECTURE.md (the doc says so explicitly) and tick/extend the relevant ROADMAP milestone.
- Match the existing doc language: the architecture/roadmap prose is French; code, identifiers, and code comments are English.
- Stay within the current milestone's scope (ROADMAP). The intended validation path is end-to-end: prove the plugin ABI on the simple `time` plugin (M2) before the two-tier model on `obs` module+plugin (M4), before porting CastMate's ~30 plugins (M8).
