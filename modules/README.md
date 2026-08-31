# Quicktys modules (certified drivers)

A **module** encapsulates *how to talk to* an external system — OBS, Twitch, the
system audio, the keyboard, a Stream Deck. Modules are the only extensions
granted **system capabilities** (`net`, `input`, `audio`, `hid`, `files`,
`process`, `timers`), so publishing one requires **certification** (a signing
key issued by the Quicktys certified-developer program).

A module is a WebAssembly **component** implementing the `module` world from
[`../wit/world.wit`](../wit/world.wit). It exposes:

- **services** — typed request/response methods plugins can call;
- **events** — high-level notifications plugins can subscribe to;
- its own **settings/secrets** + UI (e.g. the OBS connection panel).

It does **not** define user-facing actions/triggers — that's the plugin's job.
A single module can back many plugins.

```sh
cargo build -p <module> --target wasm32-wasip2 --release
```

Modules are **not** bundled with the core. They are resolved and downloaded on
demand: installing a plugin pulls the certified modules it requires; a module
nobody uses can be removed. `obs/` is the reference scaffold.
