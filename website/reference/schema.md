---
description: "The Quicktys schema types that generate forms and validate ABI payloads: strings, numbers, durations, colours, ranges, toggles and resource references."
---

# Schema types

A **schema** is the self-contained description of one field. It drives three
things at once:

1. **UI** — the frontend generates a form from it;
2. **Validation** — every JSON payload crossing the WebAssembly ABI is checked
   against it;
3. **Serialisation** — YAML ↔ typed values for persistence, round-trip safe.

Schemas serialise with an internally-tagged `type` discriminator:
`{ "type": "Number", "min": 0, "max": 100 }`.

## Shared metadata

Every variant carries:

| Field | Meaning |
| --- | --- |
| `name` | The label — **an i18n key**, not a translated string. |
| `required` | Whether the field must be present (checked when it is an object property). |
| `suggest` | An autocomplete source name. Does not constrain the value; the editor just offers suggestions. |

## Base types

### `String`

```json
{ "type": "String", "default": "", "enum": ["a", "b"],
  "minLength": 0, "maxLength": 200, "multiline": false, "template": false }
```

`enum` renders a dropdown. `multiline` renders a textarea. `template` opts the
field into `@`/`{{ }}` templating.

```rust
Schema::string()
Schema::string().options(["info", "warn", "error"])
Schema::template_string()
```

### `Number`

```json
{ "type": "Number", "default": 0, "min": 0, "max": 100,
  "step": 1, "slider": false, "unit": "%", "template": false }
```

`min` + `max` in a widget config renders a slider.

### `Boolean`

```json
{ "type": "Boolean", "default": false }
```

::: warning Unset reads as ON in widget configs
The widget config form reads an unset boolean as `!== false`, so it shows ON.
Name options `showX`, never `hideX`.
:::

### `Object`

```json
{ "type": "Object", "properties": { "a": { "type": "String" } } }
```

Properties are **ordered** — the order is the order of the generated form.

```rust
Schema::object([
    ("message".into(), Schema::template_string()),
    ("level".into(),   Schema::string().options(["info", "warn"])),
])
```

### `Array`

```json
{ "type": "Array", "items": { "type": "String" }, "minItems": 0, "maxItems": 10 }
```

## Rich UI types

These exist so a form can be expressive without the extension shipping any UI
code. Adding one means adding a variant and a validation arm — **the ABI does not
change**, because payloads stay JSON validated against the schema.

### `Duration`

```json
{ "type": "Duration", "default": 5, "min": 0, "max": 3600, "template": false }
```

A length of time **in seconds**. Rendered as a duration control rather than a
bare number.

### `Color`

```json
{ "type": "Color", "default": "#7c5cff", "template": false }
```

An RGBA/hex colour string. Rendered as a colour picker.

### `FilePath`

```json
{ "type": "FilePath", "default": "", "template": false }
```

A filesystem path — file or media. Stored as a string, rendered with a file
picker.

### `Range`

```json
{ "type": "Range", "default": { "min": 0, "max": 10 }, "template": false }
```

A numeric range. Both bounds are optional and independently templatable.

### `Toggle`

```json
{ "type": "Toggle", "default": "toggle" }
```

Tri-state: `true`, `false`, or the string `"toggle"` meaning *flip whatever it
currently is*. This is what makes a single "Mute" action able to mute, unmute or
toggle.

### `Resource`

```json
{ "type": "Resource", "resourceType": "queue", "default": null }
```

A reference to a host resource by id.

## Templating and coercion

A field with `template: true` may contain `@` and `{{ }}`. The host resolves it
**before** the action is invoked, then coerces the result to the field's type:

| Type | Coercion |
| --- | --- |
| String, Color, FilePath | → string, honouring `maxLength` |
| Number, Duration | → number, clamped to `min`/`max` |
| Boolean | `"false"` → `false`, anything else → true |
| Toggle | `"toggle"` or a boolean |
| Range | templating applied recursively to `min` and `max` |

A templatable field skips strict type checking when its value is itself a
template string — the type is checked after resolution.

## Autocomplete sources

`suggest` attaches a named source to a string field:

```rust
Schema::string().suggest("obs-scene")
```

| Source | Suggests |
| --- | --- |
| `alert` | names of alert widgets currently placed |
| `countdown` | names of countdown widgets currently placed |
| `tts-voice` | installed TTS voices |
| `obs-scene` | `obs.scenes` |
| `obs-input` | `obs.inputs` |

::: warning A new source must be wired frontend-side
The suggestion resolver lives in the app. An unwired source silently suggests
nothing, which just leaves the user guessing — the worst of both worlds.
:::

Widget-name sources are collected across the dashboard **and** the overlays, so a
name is suggested wherever the widget is placed.

## Where schemas appear

| Declared by | For |
| --- | --- |
| `ActionDecl::config` | an action's form |
| `ActionDecl::result` | the shape an action returns (what `bind` captures) |
| `TriggerDecl::context` | the shape an occurrence carries |
| `StateDecl::schema` | a published state value |
| `Manifest::settings` | the plugin's settings form |
| `ServiceDecl::args` / `result` | a module service |
| `EventDecl::payload` | a module event |
| widget `bundle.json` `config` | a widget's settings form |

## Next

- [Templating helpers](/reference/templating)
- [Write a plugin](/dev/plugin)
