---
description: "Write a Quicktys widget: bundle.json, the config schema, the postMessage bridge, preview pages, and the sandbox rules that govern fonts and network access."
---

# Write a widget

A widget is the third extension type and the easiest to write: **no Rust, no
WebAssembly, no build step**. It is an HTML file, a JSON descriptor and a folder
of translations. The bundle is authored, not compiled.

Widgets appear in two places — the dashboard grid and stream overlays — and
render identically in both.

## The bundle

```
widgets/mybox/
├── bundle.json      the descriptor + config schema
├── index.html       the widget itself
├── preview.html     optional: what the overlay editor shows
└── lang/
    ├── en.json
    └── fr.json
```

`scripts/build-bundles.sh` copies this directory as-is. Add it to the `WIDGETS`
list to have it ship:

```bash
WIDGETS=(
  "widget:widgets/mybox:mybox"
)
```

## `bundle.json`

```json
{
  "id": "mybox",
  "kind": "widget",
  "name": "My box",
  "icon": "pi pi-star",
  "runtime": "html",
  "entry": "index.html",
  "preview": "preview.html",
  "requires": ["countdown"],
  "defaultW": 2,
  "defaultH": 1,
  "config": {
    "type": "Object",
    "properties": {
      "name": { "type": "String", "default": "timer" },
      "format": { "type": "String", "enum": ["auto", "mm:ss"], "default": "auto" },
      "showLabel": { "type": "Boolean", "default": true },
      "textColor": { "type": "Color", "default": "#ffffff" },
      "radius": { "type": "Number", "default": 12, "min": 0, "max": 60 }
    }
  }
}
```

| Field | Meaning |
| --- | --- |
| `runtime: "html"` + `entry` | a **runtime** widget: your HTML in a sandboxed iframe |
| `template` | a **built-in** widget: drawn by a component inside the app |
| `preview` | a separate page the overlay editor loads instead of `entry` |
| `requires` | plugin ids this widget needs — shown to the user |
| `defaultW` / `defaultH` | initial size in dashboard grid cells |
| `config` | the settings schema, which generates the ⚙ form |

## The config form

The generated form understands four shapes:

| Schema | Control |
| --- | --- |
| `Boolean` | switch |
| `Color` | colour picker |
| `Number` with `min`/`max` | slider |
| `String` with `enum` | dropdown |

::: danger Name your booleans `showX`, never `hideX`
An unset boolean renders as **ON** and is read as `!== false`. A `hideFoo` option
would therefore show a switch that is on while the thing is visible — the switch
would lie about its own meaning. Every shipped widget uses `showX`.
:::

Labels are i18n keys, resolved from your `lang/` folder:

```
widget.mybox.name
widget.mybox.config.<key>
widget.mybox.options.<key>.<value>     ← enum option labels
```

::: warning Enum labels do not live under `config`
`widget.mybox.config.format` must stay a **string** — it is the field's own
label. Option labels go under `widget.mybox.options.format.auto`. A key that
resolves to an object breaks translation lookup.
:::

## The bridge

Your widget talks to the app with `postMessage`. Two messages in, two out.

**Host → widget**

```js
{ source: "quicktys", type: "config", config: { /* your settings */ } }
{ source: "quicktys", type: "state",  state:  { /* reactive state */ } }
```

**Widget → host**

```js
{ source: "quicktys-widget", type: "ready" }
{ source: "quicktys-widget", type: "action",
  plugin: "countdown", action: "report-finished", config: { name, seq } }
```

An action runs as a single-node sequence, exactly as if it were in an automation.

### A minimal widget

```html
<!doctype html>
<html>
  <body>
    <div id="title">—</div>
    <script>
      let config = {};
      let state = {};

      function render() {
        document.body.style.color = config.textColor || "#fff";
        document.getElementById("title").textContent = state["pear.title"] || "—";
      }

      window.addEventListener("message", (e) => {
        const d = e.data;
        if (!d || d.source !== "quicktys") return;
        if (d.type === "config") config = d.config || {};
        else if (d.type === "state") state = d.state || {};
        render();
      });

      render();
      // Tell the host you are listening — it replies with config and state.
      parent.postMessage({ source: "quicktys-widget", type: "ready" }, "*");
    </script>
  </body>
</html>
```

Send `ready` **last**, after your listener is attached. The host answers it with
the current config and state.

State keys arrive **flat and namespaced**: `state["pear.title"]`, not
`state.pear.title`.

## Where it runs

In the app, the iframe loads from a custom **`widget://localhost/<id>/`** URI
scheme (`http://widget.localhost/<id>/` on Windows), so each widget is its own
document with **no inherited CSP** — which is why an inline `<script>` works.

In an OBS browser source, the same file is served by the app's local HTTP server
and state arrives over SSE, converted into the same `postMessage` shape. That is
why rendering is identical in both places.

::: warning No network access
The iframe cannot fetch anything. A web font silently falls back, so **use system
font stacks only**. The same goes for images and scripts: inline them or take
them from state.
:::

## Preview pages

A widget that is invisible at rest — an alert box only appears when fired — cannot
be positioned in the overlay editor. Ship a `preview.html`:

- the **overlay editor** loads `preview.html`,
- an **OBS browser source** always loads `index.html`.

The preview receives the same `config` message, so it should stage itself with
placeholder content while honouring the user's colours and options. That keeps
`index.html` simple and idle, with the demo in a separate file.

## Interaction

Widgets in the overlay editor are made **inert** with `pointer-events: none` so
the editor keeps every click and a preview can never swallow a drag. On the
dashboard they are fully interactive.

## Naming and autocompletion

A widget that is addressed by name — an alert box, a countdown — reads that name
from its own config. The plugin side declares `.suggest("<widget type>")` on the
matching field so the action's name field autocompletes with the widgets actually
placed, across the dashboard **and** overlays.

For a new suggestion source to produce anything, it must also be wired into the
frontend's suggestion provider. An unwired source silently suggests nothing.

## Two places, two forms

Widgets are configured in **two separate UIs** with separate code: the dashboard's
config form and the overlay editor's ⚙ panel. They render the same schema, but a
setting added to one is not automatically in the other.

Because a sandboxed module page has no access to the app's i18n, the overlay
editor resolves labels host-side and ships them in the init message as `label`,
`configLabels` and `optionLabels`.

## Locales

```json
// widgets/mybox/lang/en.json
{
  "name": "My box",
  "config": {
    "name": "Countdown",
    "format": "Format",
    "showLabel": "Show label",
    "textColor": "Text colour"
  },
  "options": {
    "format": { "auto": "Automatic", "mm:ss": "Minutes and seconds" }
  }
}
```

## Built-in widgets

Setting `template` instead of `runtime`/`entry` makes the app draw the widget with
an internal Vue component. That is how the Info box and the Button work. It is
not available to third-party widgets — a component has to exist in the app — so
your widget will be a runtime one.

## Test it

```bash
scripts/build-bundles.sh
setup/run.sh --solo
```

Dashboard → Edit → Add widget → yours.

## Next

- [Translations](/dev/i18n)
- [Shipped extensions](/reference/extensions) — every shipped widget's schema.
