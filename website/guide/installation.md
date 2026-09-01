---
description: "Install Quicktys on Linux, macOS or Windows: prerequisites, the wasm32-wasip2 target, development keys, and how to build and run it from a bare clone."
---

# Installation

Quicktys is not yet distributed as a signed installer, so today you install it by
building it. That sounds worse than it is: one script per OS installs every
prerequisite and generates the development keys, and a second builds and runs
everything.

::: danger The repository is not public yet
The licence texts are with a lawyer before the code opens, so **the `git clone`
below will not work for you yet** — nor will the links to source files elsewhere
on this site. This page is published now so the documentation is ready the day
the repository opens; the steps themselves are accurate and tested.
:::

::: warning Pre-1.0
There are no release binaries yet. Expect to have a Rust toolchain and Node on
your machine. See the [roadmap](/contributing/roadmap) for where packaging sits.
:::

## From a bare clone

```bash
git clone https://github.com/Ascol57/Quicktys && cd Quicktys
./setup/setup-linux.sh      # or setup-macos.sh
./setup/run.sh
```

On Windows:

```powershell
powershell -ExecutionPolicy Bypass -File setup\setup-windows.ps1
powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1
```

`./setup/setup.sh` detects the OS and calls the right script on macOS and Linux.

The first `run.sh` is long — it builds the whole Rust workspace plus one
WebAssembly component per extension. Subsequent runs reuse `target/`.

## What the setup script installs

| | |
| --- | --- |
| **Native toolchain** | Xcode Command Line Tools (macOS), `build-essential` + GTK 3 + WebKit2GTK 4.1 (Linux), MSVC Build Tools + WebView2 (Windows). Tauri renders the UI in the **system webview**, so that is what must be present — not Chromium. |
| **Rust** | via rustup, plus the **`wasm32-wasip2`** target. Rust **1.82+** is required: that is the release that introduced the target, and without it no extension builds. |
| **Node 18+** | the frontend is a Vue 3 + Vite app. |
| **npm packages** | `npm install` inside `frontend/`. |
| **Dev keys** | see below. |

::: details macOS: the script stops asking for Xcode CLT
That is expected — the Command Line Tools installer is graphical. Let it finish,
then run `./setup/setup-macos.sh` again.
:::

## Running

```bash
./setup/run.sh                # registry server + app (default)
./setup/run.sh --solo         # the app alone, no marketplace
./setup/run.sh --web          # the frontend alone, in a browser (Vite)
./setup/run.sh --skip-build   # relaunch without rebuilding
./setup/run.sh --port 9000    # change the registry port
```

Two processes are involved:

- **The client** is the Tauri desktop app (`quicktys-app`). It *embeds* the built
  frontend (`frontend/dist`), so there is no web server to start for the UI. It
  does run its own local HTTP server for overlays, so OBS browser sources can
  reach them.
- **The server** is `quicktys-regserve`, the registry the app installs extensions
  from. It is optional: with `--solo`, the app loads the bundles built locally
  into `dist/bundles`.

The default mode delegates to `dev.sh`, which packs and **signs** the catalogue
before serving the registry — that is the path that exercises the real
marketplace chain end to end.

::: tip `--web` is for UI work only
It gives you Vite hot-reload, but the IPC is stubbed: no host, no extensions, no
real state.
:::

## The development keys

`dist/` is gitignored, so a fresh clone has **no keys at all**. `setup/gen-keys.sh`
creates them (the setup scripts call it, and `run.sh` calls it if they are
missing):

| File | Role |
| --- | --- |
| `dist/ca.key` / `ca.pub` | The development **certification authority**. It signs publisher certificates, which is what makes a module *certified* — and therefore allowed to hold system capabilities (`net`, `input`, `audio`…). The app pins the public half via `QUICKTYS_CA`. |
| `dist/store.key` / `store.pub` | The **store key**. It signs the licences (entitlements) of paid extensions. Pinned via `QUICKTYS_STORE`. |
| `dist/secrets/<kind>-<id>.ck` | The content key of an encrypted package (created at packing time). |
| `dist/pear.license` | A licence signed by the store key, to paste into the app to unlock `pear`. |

```bash
./setup/gen-keys.sh           # create what is missing, never overwrite
./setup/gen-keys.sh --show    # just the inventory
./setup/gen-keys.sh --force   # regenerate everything (invalidates existing signatures)
```

These are **ML-DSA** (post-quantum) keypairs stored as hex. Nothing here is a
production secret: losing `dist/` costs nothing, you regenerate it. Nothing in it
should ever be committed.

## Rebuilding after a change

| You changed… | Run |
| --- | --- |
| the frontend (`frontend/`) | `npm --prefix frontend run build` — `tauri.conf.json` deliberately has no `devUrl`, so the binary loads the embedded `frontend/dist`. `run.sh` does this unless you pass `--skip-build`. |
| an extension (`plugins/`, `modules/`, `widgets/`) | `scripts/build-bundles.sh`, then restart the app. |
| a host crate (`crates/`) | `cargo run -p quicktys-app`. |

## Known gotchas

- **The data directory is hardcoded.** `crates/quicktys-app/src/main.rs` builds
  `$HOME/.local/share/quicktys`. That works on macOS but is not the convention
  there (`~/Library/Application Support`), and on **Windows `HOME` is not set** —
  the app would fall back to `%TEMP%` and lose profiles and settings on restart.
  `run-windows.ps1` therefore sets `HOME=%USERPROFILE%`.
- **Linux needs a real graphical session.** The app will not start over SSH with
  no display (GTK/WebKit).
- **Windows needs `bash`** for `scripts/build-bundles.sh`. The one from Git for
  Windows is enough; the setup script installs it.

## Next

[Your first automation →](/guide/getting-started)
