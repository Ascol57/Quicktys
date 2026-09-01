---
description: "Write a certified Quicktys module: system capabilities, services and events, socket drivers, OAuth, and why you must never authorise inside a poll loop."
---

# Write a module

A module is a **driver**: how to talk to OBS, to Twitch, to the soundcard, to the
keyboard. It is the only tier that receives system capabilities, and the only one
that must be **certified** to use them.

Write a module when a plugin cannot do the job — which means when you need the
network, a device, the filesystem, or to start a process. Otherwise write a
[plugin](/dev/plugin).

::: tip Check whether a module already exists
The `http` module is a generic HTTP client. If your integration is REST-shaped, a
plugin declaring `requires("http", "^0")` is the whole answer, and you avoid the
certification process entirely.
:::

## The crate

```toml
# modules/mydriver/Cargo.toml
[package]
name = "mydriver"
version = "0.1.0"
edition = "2021"
license = "MIT"

[lib]
crate-type = ["cdylib"]

[dependencies]
quicktys-module-sdk = { path = "../../crates/quicktys-module-sdk" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wit-bindgen = "=0.36.0"
```

## The smallest module

```rust
use quicktys_module_sdk::{Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",       // note: "module", not "plugin"
});

use quicktys::api::audio::play_file;

struct Sound;

impl Module for Sound {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("sound", "Sound", "0.1.0")
            .capability(Capability::Audio)
            .service(ServiceDecl::new("play").args(Schema::object([
                ("path".to_string(), Schema::string()),
                ("volume".to_string(), Schema::number()),
            ])))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "play" => {
                let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
                if path.is_empty() {
                    return Err("sound path is required".to_string());
                }
                let volume = args.get("volume").and_then(Value::as_f64).unwrap_or(1.0) as f32;
                play_file(path, volume)?;
                Ok(Value::Null)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Sound);
```

## Services and events

A module exposes **services** (request/response methods plugins call) and
**events** (things it pushes to subscribers). It does *not* expose actions or
triggers — that is the plugin's job.

```rust
ModuleManifest::new("mydriver", "My Driver", "0.1.0")
    .capability(Capability::Net)
    .service(ServiceDecl::new("send").args(schema).result(schema))
    .event(EventDecl::new("message").payload(schema))
    .settings(schema)
    .view("pi pi-desktop", Some("module.mydriver.view".into()), "index.html")
```

One module can feed several plugins, which is the point of the split.

## Capabilities

Declare exactly what you need, and no more:

```rust
.capability(Capability::Net)
.capability(Capability::Audio)
```

Available: `Net`, `Input`, `Audio`, `Hid`, `Files`, `Process`, `Timers`. Full
signatures in [Host capabilities](/reference/capabilities).

Three gates stand between a declaration and an actual call:

1. **Certification** — an uncertified module that declares a capability is
   **refused at load**. In development, the `certified` flag in your bundle
   descriptor stands in for a signature.
2. **Declaration** — a capability you did not declare is denied.
3. **Consent** — the user must have granted it, and can revoke it at any time,
   with immediate effect.

All three are checked on **every dispatch**, not once at load.

::: info Some capabilities need no certification
`storage`, `logging` and `events` are safe and available to any module. A module
that only needs those — like `webview`, which owns overlay data — needs no
certification at all.
:::

## Lifecycle

| Method | When |
| --- | --- |
| `manifest()` | at load |
| `on_load()` | once after instantiation, with capabilities granted — open connections here |
| `call()` | a plugin called a service |
| `on_settings_changed()` | on settings change, and at load |
| `on_host_event()` | low-level data from a capability you hold |
| `on_unload()` | before the component is dropped |

## Host events: the driver loop

Capabilities that produce data asynchronously — a socket, a timer, an HID report
— deliver it to `on_host_event`, tagged with the `event-id` you passed when you
opened the handle. Your job is to turn low-level bytes into a **service event**:

```rust
use quicktys::api::events::emit_event;
use quicktys::api::net::{ws_connect, send, close};

fn call(method: &str, args: Value) -> Result<Value, String> {
    match method {
        "connect" => {
            let url = args.get("url").and_then(Value::as_str).unwrap_or_default();
            let handle = ws_connect(url, "socket").map_err(|e| format!("{e:?}"))?;
            // remember the handle in a thread_local
            Ok(Value::Null)
        }
        _ => Err("unknown method".into()),
    }
}

fn on_host_event(event_id: &str, payload: Value) {
    if event_id == "socket" {
        // parse, then publish something meaningful
        emit_event("message", &payload.to_string());
    }
}
```

::: warning Non-JSON payloads
Host events carry whatever the transport produced. A JSON socket parses; a text
protocol like IRC does not. The SDK hands a non-JSON payload through as a
`Value::String` rather than losing the line to `null` — so match on both shapes.
:::

### `ws-connect` connects before it returns

The handle you get back is a live, registered connection. That matters for
protocols where the **client speaks first** — Twitch IRC stays silent until
`CAP`/`NICK`/`JOIN` arrive. If the connection were still being dialled in the
background, your first `send` would be dropped and you would have an open socket
that never produced a message.

`net.close` really closes: it shuts down both halves, so the reader thread ends.
Half-closing would leave it parked on a live socket, still delivering — which is
how a rejoined chat ends up delivering every message twice.

## Request/response over a socket

A WebSocket driver usually needs to correlate a reply with the request that
caused it. The pattern used by the OBS module:

1. Send a request with a generated request id, and remember it in a pending map.
2. When the reply arrives in `on_host_event`, look the id up.
3. Continue the operation with what came back.

That lets a service like "set a source's visibility" internally resolve the scene
item id first and then act on it — invisible to the plugin and to the user.

## Two event flows

Keep them separate. The OBS module is the reference:

- **Real events** (a scene changed, a stream started) become module events that a
  plugin bridges to **triggers**.
- **State synchronisation** (the current scene, the list of sources, performance
  counters) becomes one bulk `state` event the plugin **mirrors into state
  without firing anything**.

Conflating them fires a trigger every time you refresh a value.

## Never authorise implicitly in a loop

Any module that is polled — the host calls the `poll` service of every module
declaring one, every three seconds — must never re-authenticate on its own.

The Pear module learned this the hard way: the player shows a **native dialog**
when an unknown client asks for a token, so a naive "no token? authenticate" pops
a modal on the user's screen every three seconds, forever.

The rules that came out of it, and that any authenticating driver should copy:

- a **denial is sticky** — only an explicit `authorize` service asks again;
- a **failed** attempt is throttled to one per minute; a **successful** one clears
  the throttle;
- a 401 clears the token, re-authorises **once**, and replays the call.

## OAuth

The core drives the interactive flow; the module owns the provider.

Expose an **`oauth-config`** service returning
`{ provider, device_url, token_url, client_id, scopes }`. The app's Settings
screen drives the device-code flow from that, stores the tokens, and refreshes
them. Your module then reads a fresh token:

```rust
use quicktys::api::oauth::access_token;

let token = access_token("twitch")?;
```

No provider specifics leak into the core, and you never write OAuth mechanics.

::: info `oauth.authorize` is still a stub, on purpose
It is a synchronous call with nowhere to hand back a user code for the UI to
display while waiting. The interactive half therefore runs UI-side over IPC.
`oauth.access-token` is real and is what you should call.
:::

## Make it testable offline

Expose a service to point your driver somewhere else:

```rust
.service(ServiceDecl::new("set-base-url").args(Schema::object([
    ("url".to_string(), Schema::string()),
])))
```

Every reference module does this — `set-base-url` on `pear`, `set-chat-url` and
`set-eventsub-url` on `twitch`. It costs one method and it means the whole
integration can be tested against a stand-in server, in CI, with no credentials.

## Contributing a tab

A module can contribute a whole app page:

```rust
.view("pi pi-desktop", Some("module.mydriver.view".into()), "index.html")
```

The HTML lives in `modules/<id>/ui/`, is copied to the bundle root, and is served
over `moduleui://<id>/`. The page gets a scoped request/response bridge and can
call **only its own module**. See [UI contributions](/dev/ui).

## Certification

Publishing a module that uses system capabilities means getting a publisher
certificate and signing your releases. In development, the `certified` flag in
`scripts/build-bundles.sh` stands in.

See [Package & publish](/dev/packaging#certification).

## Next

- [Host capabilities](/reference/capabilities) — every function you can import.
- [Package & publish](/dev/packaging) — signing and distribution.
