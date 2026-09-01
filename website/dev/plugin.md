# Write a plugin

A plugin is the user-facing half of an integration: it declares **actions**,
**triggers**, **state** and **settings**, and it composes modules to do anything
it cannot do itself — which is anything involving the outside world.

Anyone can publish one. No certification, no signature required. The sandbox is
the guarantee.

## The crate

Extensions live outside the host workspace. Create the directory and a
standalone package:

```toml
# plugins/greeter/Cargo.toml
[package]
name = "greeter"
version = "0.1.0"
edition = "2021"
license = "MIT"          # your extension, your licence

[lib]
crate-type = ["cdylib"]  # a WebAssembly component, not a Rust library

[dependencies]
quicktys-plugin-sdk = { path = "../../crates/quicktys-plugin-sdk" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wit-bindgen = "=0.36.0"
```

::: warning Pin `wit-bindgen` to `=0.36.0`
Newer 0.5x releases need the unstable `wasip2` std feature. The pin is
deliberate, not neglect.
:::

## The smallest plugin

```rust
// plugins/greeter/src/lib.rs
use quicktys_plugin_sdk::{ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

// Generates the bindings for the `plugin` world. Must come before define_plugin!.
wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};

struct Greeter;

impl Plugin for Greeter {
    fn manifest() -> Manifest {
        Manifest::new("greeter", "Greeter", "0.1.0")
            .icon("mdi mdi-hand-wave")
            .color("#8dc1a9")
            .action(
                ActionDecl::new("greet").config(Schema::object([(
                    "who".to_string(),
                    Schema::template_string().named("plugin.greeter.fields.who"),
                )])),
            )
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            "greet" => {
                let who = config.get("who").and_then(Value::as_str).unwrap_or("world");
                log(Level::Info, &format!("hello, {who}"));
                Ok(Value::Null)
            }
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }
}

quicktys_plugin_sdk::define_plugin!(Greeter);
```

That is a complete, loadable plugin.

`define_plugin!` wires your `Plugin` impl to the generated `plugin-guest` export,
handles the JSON marshalling both ways, and emits the component export shims. It
must be invoked **after** `wit_bindgen::generate!`, so the generated `exports::…`
paths and the `export!` macro are in scope.

## The manifest

`manifest()` is the whole contract. The host reads it to build the UI, validate
payloads and resolve dependencies.

```rust
Manifest::new("greeter", "Greeter", "0.1.0")
    .icon("mdi mdi-hand-wave")      // a Material Design Icons class
    .color("#8dc1a9")               // accent colour in the editor
    .action(ActionDecl::new("greet").config(schema).result(schema))
    .trigger(TriggerDecl::new("greeted").context(schema))
    .state_field(StateDecl::new("last").schema(schema))
    .settings(schema)               // a generated form in Settings
    .requires("http", "^0")         // a module dependency, semver
    .ui(UiDecl::button("webview.toolbar", "greet"))
```

::: danger Emit keys, not sentences
Every human-readable string is an **i18n key**: `.named("plugin.greeter.fields.who")`,
not `.named("Who")`. The text lives in `lang/en.json`. A manifest that emits
English will not translate, ever. See [Translations](/dev/i18n).
:::

### Schemas

Config, context, results and settings are all described with `Schema`, the same
type the host uses. The form in the editor is generated from it.

```rust
Schema::object([
    ("message".into(), Schema::template_string()),      // supports @ and {{ }}
    ("count".into(),   Schema::number()),
    ("loud".into(),    Schema::boolean()),
    ("level".into(),   Schema::string().options(["info", "warn", "error"])),
    ("scene".into(),   Schema::string().suggest("obs-scene")),
    ("tags".into(),    Schema::array(Schema::string())),
])
```

Two hints worth knowing:

- **`template_string()`** marks a field as templatable. The editor renders it
  with `@`/`{{` autocompletion, and the host resolves it before your action runs
  — so by the time you read it, `@pear.title` is already the title.
- **`.suggest("<source>")`** attaches an autocomplete source. The value stays a
  free string; the editor just offers suggestions. Existing sources: `alert`,
  `countdown`, `tts-voice`, `obs-scene`, `obs-input`.

  A new source must be **wired in the frontend's suggestion provider** to produce
  anything. An unwired source silently autocompletes nothing, which leaves users
  guessing.

Full type list in [Schema types](/reference/schema).

## Actions

`invoke_action` receives the action id, the **already-validated and
already-templated** config, and the trigger context.

```rust
fn invoke_action(action: &str, config: Value, context: Value) -> Result<Value, ActionError> {
    match action {
        "roll" => {
            let sides = config.get("sides").and_then(Value::as_u64).unwrap_or(6);
            let value = /* … */ 4;
            Ok(quicktys_plugin_sdk::json!({ "value": value }))
        }
        other => Err(ActionError::failed(format!("unknown action: {other}"))),
    }
}
```

The returned `Value` is what an action node's **bind** captures, so declare a
`result` schema if you return something structured.

Two failure modes:

| | |
| --- | --- |
| `ActionError::failed(msg)` | a real failure — ends the run unless caught by a `try` node |
| `ActionError::Aborted` | cancelled — the host does not report it as an error |

## State

Publish a value and it appears in the Variables view, in autocompletion, and in
every template and condition:

```rust
use quicktys::api::reactivity::set_state;

set_state("last", &serde_json::json!("world").to_string());
```

The key is **local**; the host namespaces it as `greeter.last`. Declare it in the
manifest with `state_field(StateDecl::new("last"))` so the editor knows about it
before your plugin has ever run.

## Triggers

```rust
use quicktys::api::reactivity::emit_trigger;

emit_trigger("greeted", &serde_json::json!({ "who": who }).to_string());
```

Again the id is local — the host fires `greeter.greeted` on the bus, and profiles
bound to it run. The JSON you pass becomes the **trigger context**: what filters
evaluate against and what `{{ }}` reads.

Declare the context shape with `TriggerDecl::new("greeted").context(schema)` so
the editor can autocomplete it.

## Storage

Per-extension key/value, persisted to `<user-data>/quicktys/storage/<id>.json`:

```rust
use quicktys::api::storage::{kv_get, kv_set};

kv_set("timers", &dump);
let restored = kv_get("timers");
```

Values are strings — serialise your own JSON. This is a safe capability: no
certification needed.

## Calling a module

This is how a plugin reaches anything real. Declare the dependency in the
manifest, then call:

```rust
use quicktys::api::modules::call as module_call;

Manifest::new("discord", "Discord", "0.1.0").requires("http", "^0")

let req = json!({
    "method": "POST",
    "url": url,
    "headers": { "Content-Type": "application/json" },
    "body": payload.to_string(),
});
let response = module_call("http", "request", &req.to_string())
    .map_err(|e| ActionError::failed(format!("{e:?}")))?;
```

The host checks the dependency was declared, that the method exists, and that the
arguments match the module's published schema. Errors come back as
`CallError::{NotInstalled, NoSuchMethod, Denied, Failed(String)}`.

::: tip Reuse a generic module before writing one
The `http` module is a general-purpose HTTP client. If your integration is "POST
some JSON to a URL", you need no module of your own — declare `requires("http",
"^0")` and you are done. That is exactly how the Discord plugin works.
:::

## Subscribing to module events

```rust
use quicktys::api::modules::subscribe as module_subscribe;

fn on_load() -> Result<(), String> {
    module_subscribe("pear", "song-changed").map_err(|e| format!("{e:?}"))?;
    Ok(())
}

fn on_module_event(module_id: &str, event_id: &str, payload: Value) {
    if (module_id, event_id) == ("pear", "song-changed") {
        emit_trigger("song-changed", &payload.to_string());
    }
}
```

This is the shape of most real plugins: a module does the protocol work and emits
events; the plugin mirrors some into state and bridges others onto the trigger
bus.

::: warning Do not fire a trigger on your first observation
When a driver first learns the current state — the song already playing when the
app started — mirror it into state but do **not** fire the trigger. Otherwise
every launch fires a song alert. Fire only on a change from a *known* previous
value.
:::

## Settings

```rust
.settings(Schema::object([(
    "webhook".to_string(),
    Schema::string().named("plugin.discord.settings.webhook"),
)]))
```

The host renders the form, persists the values, and delivers them:

```rust
fn on_settings_changed(settings: Value) {
    // store them — a thread_local RefCell is the usual idiom
}
```

## Lifecycle

| Method | When |
| --- | --- |
| `manifest()` | at load, via `init` |
| `on_load()` | once after instantiation — subscribe here |
| `invoke_action()` | per action |
| `on_settings_changed()` | on settings change, and at load |
| `on_module_event()` | when a subscribed module event fires |
| `on_unload()` | before the component is dropped |

## Locales

```json
// plugins/greeter/lang/en.json
{
  "name": "Greeter",
  "description": "Says hello.",
  "fields": { "who": "Who" },
  "actions": { "greet": { "name": "Say hello" } },
  "triggers": {},
  "state": {},
  "settings": {}
}
```

The host discovers `lang/` and namespaces it `plugin.greeter.*`. `en.json` is the
reference locale; missing keys in other locales fall back to it. See
[Translations](/dev/i18n).

## Build and load it

Add it to `scripts/build-bundles.sh`:

```bash
EXTENSIONS=(
  # …
  "plugin:plugins/greeter:greeter:greeter.wasm"
)
```

Then:

```bash
scripts/build-bundles.sh
setup/run.sh --solo
```

Your plugin appears in the action picker.

## Next

- [Write a module](/dev/module) — when a plugin is not enough.
- [UI contributions](/dev/ui) — add a button or a tab to the app.
- [Package & publish](/dev/packaging) — ship it.
