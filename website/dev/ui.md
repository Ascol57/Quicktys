# UI contributions

Extensions can add to the app's interface in two ways: a **slot contribution**
(something small dropped into a named place) and a **contributed view** (a whole
tab).

Both are declarative. Neither requires a change to the app.

## Slots — a plugin adds a button

Each extensible place in the interface has a **slot id** and renders whatever
plugins have contributed to it, with a context object describing the situation.

Declare a contribution in the manifest:

```rust
use quicktys_plugin_sdk::UiDecl;

Manifest::new("obs", "OBS Studio", "0.1.0")
    .ui(
        UiDecl::button("webview.toolbar", "add-browser-source")
            .label("plugin.obs.ui.add-to-obs")
            .icon("pi pi-plus"),
    )
```

That is the real **Add to OBS** button in the overlay editor's toolbar. Clicking
it runs the plugin's `add-browser-source` action with the **slot's context merged
into the action config** — for that slot, `{ url, name }` of the overlay being
edited. The action calls the OBS module, which creates the browser source in the
current scene.

Two shapes are available:

| | |
| --- | --- |
| `UiDecl::button(slot, action)` | a native button. `.label(key)`, `.icon(class)`, `.config(json)` for extra fixed config. |
| `UiDecl::custom(slot, entry)` | an HTML file your plugin ships, rendered in a sandboxed iframe and fed the same context. |

Custom UI files live in `plugins/<id>/ui/`, are copied into the bundle by the
build script, and are served over `pluginui://<id>/<file>` — its own origin, its
own CSP.

### Known slots

| Slot id | Where | Context |
| --- | --- | --- |
| `webview.toolbar` | the overlay editor's toolbar | `{ url, name }` of the current overlay |

The list grows as places are made extensible; a slot is one `<Slot id="…">` in a
Vue component.

## Views — a module adds a tab

A **module** can contribute an entire page:

```rust
ModuleManifest::new("webview", "Webview", "0.1.0")
    .view("pi pi-images", Some("module.webview.view".into()), "index.html")
```

The app reads this from `list_modules`, adds an entry to the sidebar, and renders
`entry` in a sandboxed iframe served over `moduleui://<id>/`. The HTML lives in
`modules/<id>/ui/` and is copied to the bundle root, so `entry` is just
`"index.html"`.

The **Overlays** tab is exactly this — contributed by the `webview` module, not
hardcoded into the app.

### The scoped bridge

The key difference from a slot: a contributed view can **drive its own module**,
over a request/response `postMessage` bridge.

```js
// page → host
{ type: "call", reqId: 7, service: "list-overlays", args: {} }

// host → page
{ type: "result", reqId: 7, ok: true, value: [ /* … */ ] }
```

The bridge is **scoped**: the page can only call *its own* module, never another
one. That is what makes it safe to render third-party HTML with a live channel to
the host.

## Serialisation: the one real trap

::: danger Never hand a Vue reactive value to `postMessage`
`postMessage` serialises with the structured clone algorithm, which **throws
`DataCloneError` on a Proxy** — and props, store values and anything reactive
*are* Proxies.

WebKit (Tauri's webview on Linux and macOS) rejects them outright, so the symptom
is a widget or view that silently never receives its settings.

`toRaw()` is not enough: it only unwraps the top level. Every bridge payload goes
through a JSON round-trip (`plain()` in the frontend) — which is correct, because
the bridges are JSON by contract.
:::

## Why sandboxed iframes and not components

A contributed page is third-party HTML. Rendering it inline would give it the
app's origin, the app's CSP and the app's DOM. Serving it from its own URI scheme
gives it:

- its own origin, so it cannot read the app's storage;
- its own CSP, so inline scripts work without loosening the app's;
- a single, auditable channel to the host.

The same reasoning applies to widgets. See [Write a widget](/dev/widget).

## Localisation

A sandboxed page has no access to the app's i18n. The host resolves the labels
and ships them in the page's init message. Read them from there rather than
hardcoding text.

## Next

- [Write a module](/dev/module)
- [Write a widget](/dev/widget)
