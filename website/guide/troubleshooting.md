---
description: "Why a Quicktys trigger never fires, an action does nothing, a browser source stays blank or an extension change has no effect — and how to fix each one."
---

# Troubleshooting

## A trigger never fires

Work down this list in order — the first two account for most cases.

**1. Check the filter and the activation condition.** Rhai coerces every compile
and evaluation error to `false`, silently. A typo — `=` instead of `==`, a
variable spelled slightly wrong — disables the binding with no message anywhere.

**2. Check the variable name.** Open the **Variables** view and read the exact
key. State is namespaced flat, so it is `obs.scene`, not `scene`. If the key is
not listed at all, the plugin that publishes it is not loaded.

**3. Check the profile is active.** An activation condition that is currently
false switches off every binding in that profile.

**4. Check the plugin is installed and loaded.** The trigger picker only lists
triggers from loaded plugins.

**5. For Twitch follows/subs/raids**, check an account is connected. Those come
over EventSub and need a user token; chat does not.

**6. For a countdown's *finished* trigger**, check the countdown is displayed by
a widget somewhere. Nothing ticks host-side, so an undisplayed countdown never
reports crossing zero.

## An action does nothing

**Check the app's log** — a failing action reports a message. Without a `try`
node, a failure ends the whole run, so anything after it will not have executed
either.

**Check the module is connected.** OBS actions do nothing if obs-websocket is not
reachable; Pear actions do nothing if the player is not running or consent was
denied.

**Check the capability is granted.** In Settings, a revoked capability makes every
call using it fail immediately. This is also what happens to a **sideloaded
unsigned module**: it installs, but its system capabilities are refused at load.

## Nothing appears in a browser source

- Is the app running? The overlay server lives inside it.
- Open `http://localhost:8787/overlay/<id>` in a normal browser. If that is blank
  too, the problem is the overlay, not OBS.
- Is the **Overlays** plugin installed? Without it, `/overlay/<id>` does not
  exist.
- Set the browser source's width and height to your canvas resolution. Widget
  geometry is in percentages, so a mis-sized source scales everything oddly.

## Every chat message appears twice

The same chat is connected twice — typically a dashboard chat box *and* an
overlay both calling connect. Closing one genuinely closes its socket now, so
this should not persist; if it does, restart the app and connect once.

## A widget shows the wrong fonts

Widget iframes have **no network access**, so a web font silently falls back.
Widgets are built with system font stacks only. If you wrote the widget, that is
the fix.

## A setting exists in one place but not the other

Widgets are configured in two separate UIs — the dashboard and the overlay editor
— with separate code. A setting added to one is not automatically in the other.
This is a known rough edge.

## The app will not start

**On Linux**, it needs a real graphical session. It will not start over SSH with
no display.

**Anywhere**, check the frontend is built: `tauri.conf.json` has no `devUrl` on
purpose, so the binary loads the embedded `frontend/dist`. After changing the
frontend, rebuild it.

## Profiles and settings vanish on restart (Windows)

The data directory is currently built from `$HOME`, which Windows does not set. It
falls back to `%TEMP%`, which is wiped. `run-windows.ps1` sets
`HOME=%USERPROFILE%` to work around it — if you are launching the binary another
way, set it yourself.

## An extension fails to build

- `wasm32-wasip2` target missing: `rustup target add wasm32-wasip2`. Rust 1.82+
  is required.
- Extensions are **not** in the host workspace. Build from the crate directory,
  not with `-p` from the root.
- `wit-bindgen` is pinned to `=0.36.0` on purpose. Newer 0.5x needs an unstable
  std feature.
- The artifact name replaces `-` with `_`: package `counter-client` produces
  `counter_client.wasm`.

## An extension change has no effect

Run `scripts/build-bundles.sh` and restart the app. The host loads **bundles**,
not raw `.wasm` files from `target/`.

Also check the extension is in the script's `EXTENSIONS` or `WIDGETS` list — that
list is what ships.

## Still stuck

Open an issue at
[github.com/Ascol57/Quicktys](https://github.com/Ascol57/Quicktys/issues) with
what you did, what happened, and the relevant log lines.
