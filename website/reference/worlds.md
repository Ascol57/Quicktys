---
description: "The Quicktys WIT worlds: what a plugin and a module import and export, the lifecycle order at load, and how calls cross the sandbox boundary."
---

# Worlds & lifecycle

The extension contracts are WIT interfaces in package **`quicktys:api@0.2.0`**,
defined in [`wit/world.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/world.wit).

## The ABI rule

> The typed part of the contract is the **WIT function signature**. Every
> structured payload — config, context, service arguments, events, schemas, state
> values — is a **JSON string**, validated by the host against the schema the
> extension declared in its manifest.

This keeps the ABI stable while the schema system evolves. Adding a new rich
schema type does not change a single WIT signature.

## `world plugin`

```wit
world plugin {
    import logging;
    import storage;
    import reactivity;
    import modules;
    export plugin-guest;
}
```

That is the complete list of what a plugin can reach. System capabilities are not
in this world, so a plugin cannot request them — it cannot even name them.

### `interface plugin-guest`

```wit
variant invoke-error { aborted, failed(string) }

init: func() -> string;
on-load: func() -> result<_, string>;
on-unload: func();
invoke-action: func(action-id: string, config-json: string, context-json: string)
    -> result<string, invoke-error>;
on-settings-changed: func(settings-json: string);
on-module-event: func(module-id: string, event-id: string, payload-json: string);
```

| Export | Contract |
| --- | --- |
| `init` | Returns the manifest JSON: id, name, version, icon, colour, the declared actions / triggers / state / settings / secrets **with their schemas**, and the list of **required modules** with semver ranges. |
| `on-load` | Called once after instantiation. Subscribe to module events here. |
| `on-unload` | Called before the component is dropped. |
| `invoke-action` | Run an action. The host has already validated `config` and `context` against the declared schemas, **and resolved templates**. |
| `on-settings-changed` | The plugin's settings changed. Also called at load. |
| `on-module-event` | A module event this plugin subscribed to. This is where a trigger like "OBS scene changed" is realised: receive the module event, transform it, call `emit-trigger`. |

`invoke-error::aborted` means cancellation, and the host does not report it as an
error. `failed(string)` is a real failure.

## `world module`

```wit
world module {
    import logging;
    import storage;
    import events;
    import oauth;
    // System capabilities — certified modules only.
    import net;
    import input;
    import audio;
    import hid;
    import files;
    import process;
    import timers;
    export module-guest;
}
```

### `interface module-guest`

```wit
init: func() -> string;
on-load: func() -> result<_, string>;
on-unload: func();
call: func(method: string, args-json: string) -> result<string, string>;
on-settings-changed: func(settings-json: string);
on-host-event: func(event-id: string, payload-json: string);
```

| Export | Contract |
| --- | --- |
| `init` | Returns the manifest JSON: id, name, version, the **services** (methods + schemas), the **events** (+ schemas), settings/secrets, the **required system capabilities**, and an optional contributed `view`. |
| `on-load` | Open connections here — capabilities have been granted by now. |
| `call` | A plugin called one of your services. `args` was validated by the host. |
| `on-settings-changed` | Settings changed. Also called at load. |
| `on-host-event` | Low-level data from a capability you hold: socket data, a timer tick, an HID report. Turn it into a service event with `events.emit-event`. |

A module exposes **services and events**, never actions and triggers. One module
can feed several plugins.

::: warning `on-host-event` payloads are not always JSON
A JSON socket parses; a text protocol like IRC does not. The SDK hands a non-JSON
payload through as a string rather than losing the line to `null`.
:::

## Lifecycle order

At boot, and at hot-load after an install:

1. **Modules load first**, then plugins — plugins may depend on modules.
2. For each bundle: read `bundle.json`, load the `.wasm`, discover `lang/`.
3. A module's declared capabilities are checked against its `certified` flag. An
   uncertified module declaring a capability is **refused**.
4. `init()` → the manifest is decoded into the extension registry.
5. `on-load()`.

An id already loaded is skipped, so an installed bundle shadows a development
bundle with the same id.

## Calls across the boundary

### Plugin → module

```
plugin: modules.call("obs", "set-scene", "{\"scene\":\"Intro\"}")
   │
   ├─ host: is "obs" declared in the plugin's `requires`?
   ├─ host: does "obs" export a service "set-scene"?
   ├─ host: do the args validate against its published schema?
   ▼
module: call("set-scene", args) -> result<string, string>
```

Failures come back as `call-error`:

```wit
variant call-error { not-installed, no-such-method, denied, failed(string) }
```

### Module → plugin

```
module: events.emit-event("scene-changed", payload)
   │
   ▼ host fans out to every subscribed plugin
plugin: on-module-event("obs", "scene-changed", payload)
   │
   ▼ typically
plugin: reactivity.emit-trigger("scene-changed", context)
   │
   ▼ host fires `obs.scene-changed` on the trigger bus
```

## Isolation

Each extension runs in its **own wasmtime `Store`** with only its own
capabilities. A plugin cannot read the disk, open a socket, or reach secrets it
was not granted. It can only act through the services of installed modules, each
of which is itself mediated by the host.

## WIT conventions

If you edit the contracts:

- the `package` declaration must be the **first line** of each file — a leading
  comment block is parsed as a doc comment on the package, and two files each
  carrying one is an error;
- same-package interfaces are imported by short name: `import logging;`, not
  `import quicktys:api/logging;`;
- identifiers cannot shadow WIT keywords — hence `list-dir`, since `list` is the
  `list<T>` type.

Host-side bindings are generated by wasmtime's own Component Model `bindgen!` in
`quicktys-host`; guest-side by `wit_bindgen::generate!` inside each extension
crate.

## Next

- [Host capabilities](/reference/capabilities) — every imported function.
- [Schema types](/reference/schema) — the JSON payload vocabulary.
