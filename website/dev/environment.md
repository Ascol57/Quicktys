---
description: "Set up a Quicktys extension toolchain: the wasm32-wasip2 target, why extensions sit outside the host workspace, building bundles, and the test suites."
---

# Dev environment

## Prerequisites

Run the setup script for your OS once — it installs everything and generates the
development keys. See [Installation](/guide/installation).

The parts that matter for extension work:

| | |
| --- | --- |
| **Rust 1.82+** | the release that introduced the `wasm32-wasip2` target |
| **`wasm32-wasip2`** | `rustup target add wasm32-wasip2` — without it nothing builds |
| **`bash`** | `scripts/build-bundles.sh` is a shell script, including on Windows |

## Two build domains

This is the thing to internalise: **extensions are not in the host workspace.**

```toml
# Cargo.toml (root)
[workspace]
members = ["crates/quicktys-app", "crates/quicktys-core", ...]
exclude = ["plugins", "modules"]
```

Extensions are standalone packages with their own `Cargo.lock`, built for a
different target and a different crate type (`cdylib`). Adding one to the
workspace `members` will break the workspace build.

```bash
# Host workspace — from the repo root
cargo check
cargo test -p quicktys-core
cargo test -p quicktys-host              # the ABI integration tests

# An extension — from ITS OWN directory, not with -p from the root
cd plugins/time
cargo build --target wasm32-wasip2 --release
```

::: warning The artifact name is not the package name
Cargo replaces `-` with `_` in the cdylib artifact. Package `counter-client`
produces `counter_client.wasm`. This bites everyone once.
:::

## Building bundles

The app loads **bundles**, not raw `.wasm` files. Build them all:

```bash
scripts/build-bundles.sh              # → dist/bundles/
cargo run -p quicktys-app
```

The script compiles each extension and assembles:

```
dist/bundles/
├── modules/<id>/{<id>.wasm, lang/, ui/, bundle.json}
├── plugins/<id>/{<id>.wasm, lang/, ui/, bundle.json}
└── widgets/<id>/{bundle.json, lang/, *.html}       ← copied, not compiled
```

**To add your extension to the dev build, add it to the `EXTENSIONS` or `WIDGETS`
list at the top of the script.** The format is
`kind:dir:id:artifact[:certified]`:

```bash
EXTENSIONS=(
  "module:modules/mydriver:mydriver:mydriver.wasm:certified"
  "plugin:plugins/mything:mything:mything.wasm"
)
WIDGETS=(
  "widget:widgets/mybox:mybox"
)
```

The trailing `certified` flag is what lets a module hold system capabilities in
development. It is a local development shortcut — in production, certification is
[a signature](/dev/packaging#signing).

`time` is deliberately absent from that list: its `delay` action is a pure ABI
fixture that does not actually wait, so shipping it would mislead.

## The development loop

| You changed | Do this |
| --- | --- |
| an extension | `scripts/build-bundles.sh`, restart the app |
| a host crate | `cargo run -p quicktys-app` |
| the frontend | `npm --prefix frontend run build`, restart |

There is no hot reload for extensions yet — the WASM host does not support
unloading a component in place. `setup/run.sh` does the whole chain for you.

## Running the app

```bash
setup/run.sh            # registry + app, the full marketplace path
setup/run.sh --solo     # app only, loading dist/bundles directly
setup/run.sh --web      # frontend in a browser, IPC stubbed
cargo run -p quicktys-app
```

Use `--solo` while iterating on an extension: it skips packing and signing and
loads your bundle straight from `dist/bundles`.

A GUI run needs a real desktop session — GTK and WebKit will not initialise
headless.

## Testing

The host test suites are the best specification of the ABI, and the fastest way
to see a contract exercised end to end.

```bash
cargo test -p quicktys-host             # everything below
cargo test -p quicktys-host --test plugin_time      # single-tier plugin ABI
cargo test -p quicktys-host --test two_tier         # plugin ↔ module
cargo test -p quicktys-host --test net_cap          # system caps + certification gating
cargo test -p quicktys-host --test bundles          # loading a real bundle from disk
cargo test -p quicktys-host --test notify_plugin
cargo test -p quicktys-host --test twitch_chat
cargo test -p quicktys-host --test twitch_eventsub
cargo test -p quicktys-host --test pear_auth
cargo test -p quicktys-host --test pear_song_trigger
cargo test -p quicktys-host --test ws_close
```

These build the reference extensions on demand, so the first run is slow.

### Testing your own extension

The pattern used throughout: point the driver at a **stand-in server** instead of
the real thing. Modules that speak to a network service expose a service for it —
`set-base-url` on `pear`, `set-chat-url` and `set-eventsub-url` on `twitch` — so a
test never touches a live player or Twitch.

Give your module the same escape hatch. It costs one service method and makes the
whole thing testable offline.

## Reading the reference extensions

They are proprietary and source-available: read them, learn how they call the
ABI, write your own. Do not copy the code.

| Read | If you are writing |
| --- | --- |
| `plugins/notify` | the smallest possible plugin |
| `plugins/random` | a plugin with typed results you bind |
| `plugins/countdown` | a plugin using `storage`, plus the plugin↔widget pattern |
| `plugins/discord` | a plugin reaching the network through a generic module |
| `modules/sound` | the smallest possible module |
| `modules/http` | a reusable generic driver |
| `modules/obs` | a bidirectional WebSocket driver with request/response correlation |
| `modules/twitch` | WebSocket, OAuth, IRC parsing, EventSub |
| `modules/webview` | a module that owns data and contributes a whole app tab |
| `widgets/countdown` | a runtime widget with a rich config schema |
| `widgets/alert` | a widget that is invisible at rest, with a preview page |

## Next

[Write a plugin →](/dev/plugin)
