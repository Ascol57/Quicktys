# Quicktys widgets (overlays & dashboards)

A **widget** is the third extension type: an overlay/dashboard of **presentation**
shown in an OBS browser source or a window. Unlike plugins/modules it is **not** a
WASM component — it is frontend.

A widget package contains:

- **UI components** (Vue) rendered in the browser source;
- a **config schema** (drives the widget's edit form in the app);
- a **`lang/`** folder → namespace `widget.<id>.*`.

The host's local HTTP server serves the overlay page; the widget receives
plugin/module **state** in real time over a websocket and renders. Its JS runs
**isolated in the browser source** (browser sandbox), with no host access beyond
the authorised state stream — so third-party widgets are safe.

Overlay layouts (which widgets, their config + placement) are **resources**
(`overlays/*.yaml`), edited WYSIWYG in the app.

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §12. Anyone can author a widget
(no certification). M0 placeholder — no widget scaffolded yet.
