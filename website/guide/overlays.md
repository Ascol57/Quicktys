---
description: "Build stream overlays in the Quicktys WYSIWYG editor, with real live previews and snapping guides, then display them through an OBS browser source."
---

# Overlays

An **overlay** is a page of widgets that OBS displays as a browser source. You
build it in the app, on a canvas, seeing the real widgets as you place them.

<Screenshot src="/screens/overlay-editor.png" caption="The overlay editor" />

## Install it first

Overlays arrive with the **Overlays** plugin. Install it from the
[Marketplace](/guide/marketplace); it pulls the `webview` module, which is what
actually stores your overlays and ships the editor tab.

::: info Why a plugin for a core feature
Modules are never browsable — they only arrive as a plugin's declared dependency.
The overlay module had no plugin requiring it, so it never installed through the
registry at all. The Overlays plugin is the user-facing face of the feature and
carries the dependency. Uninstalling it removes the module too, and with it the
editor and the `/overlay/<id>` pages.
:::

## Creating one

**Overlays** tab → **New**. An overlay is:

```
{ id, name, aspect: 16:9 | 9:16, widgets: [...] }
```

The aspect is a preview ratio, not a resolution. Widget positions and sizes are
stored as **percentages**, so the overlay renders identically at 720p, 1080p or
1440p.

## Placing widgets

Add a widget and it appears on the canvas — **rendered for real**, live, with
your colours and options applied. It is not a mockup: it is the widget itself in
its own sandboxed document, fed the same config message the live overlay gets,
made inert so it can never swallow a click.

| Gesture | Effect |
| --- | --- |
| Drag anywhere on the widget | Move it. |
| Drag one of the eight handles | Resize. |
| Arrow keys | Nudge. Hold **Shift** for ×10. |
| **Delete** | Remove. |
| **Escape** | Deselect. |
| **▴** / **▾** | Raise or lower. Paint order is list order, so raising moves it later. |

Edges snap to the canvas sides and centre, and to every other widget's sides and
centre, drawing a guide for whatever matched. **Hold Alt to place freely.**
Geometry is rounded on drop, so the stored percentages match the readout.

<Screenshot src="/screens/overlay-widget-config.png" caption="A widget's settings inside the overlay editor" />

::: warning Two places, two forms
A widget is configured in **two** separate UIs — the dashboard and the overlay
editor — with separate code. They render the same schema, but a setting added to
one is not automatically in the other. If an option is missing on one side, that
is why.
:::

### Widgets that are invisible at rest

An alert widget shows nothing until an alert fires, which makes it impossible to
position. Those widgets ship a **separate preview page**: the editor loads
`preview.html` instead of `index.html`, so it stages itself with placeholder
content while respecting your settings. The browser source always loads the real
`index.html`.

## Putting it in OBS

The app runs a local HTTP server on port **8787**. Add a **Browser source** in
OBS pointing at:

```
http://localhost:8787/overlay/<id>
```

Set the width and height to your canvas resolution (1920×1080 for a 16:9
overlay). The overlay is transparent where nothing is drawn.

<Screenshot src="/screens/obs-browser-source.png" caption="The OBS browser source properties" />

::: tip Let the OBS plugin do it
If you have the OBS plugin installed and connected, the overlay editor's toolbar
grows an **Add to OBS** button. It creates the browser source in your current
scene with the right URL and name — no copy-pasting.

That button is not hardcoded: it is a **UI contribution** the OBS plugin declares
into the editor's toolbar slot. See [UI contributions](/dev/ui).
:::

## How state reaches the browser

The local server pushes state to the overlay page over **SSE** (`/events`): a
snapshot first, then one event per changed key. The widget consumes exactly the
same message bridge it does inside the app, which is why rendering is identical
in both places.

An action a widget asks for comes back as `POST /action` and runs as a
single-node sequence.

The transport is one-way (host → browser) by design; SSE is enough and needs no
handshake.

## Troubleshooting

**The browser source is blank.** Check the app is running, and open the URL in a
normal browser — you should see the overlay. If the Overlays plugin is not
installed, `/overlay/<id>` will not exist.

**A widget shows but has no data.** The widget's plugin may not be installed, or
the value it reads may be empty. Check the [Variables](/guide/interface#variables)
view for the key it needs.

**Two copies of every chat message.** You have the same chat connected twice — a
dashboard box *and* an overlay both calling connect. Closing one now genuinely
closes its socket, so this is fixed, but a stale app instance can still cause it.

**Fonts look wrong.** Widget iframes have no network access, so a web font
silently falls back. Widgets use system font stacks only.

## Next

- [Dashboard](/guide/dashboard) — the same widgets, in the app.
- [Write a widget](/dev/widget) — build your own.
