# Quicktys plugins

Each plugin is an independent WebAssembly **component** implementing the
`quicktys:plugin` world from [`../wit/world.wit`](../wit/world.wit). Plugins are
sandboxed: they reach the outside world only through the host capability
interfaces (`host`, `http`, `streams`).

Build a plugin for the component-model target:

```sh
cargo build -p <plugin> --target wasm32-wasip2 --release
```

The host discovers compiled `.wasm` components, reads each manifest via
`guest.init`, and registers its actions / triggers / state / settings.

`time/` is the reference scaffold. The ~30 CastMate plugins (twitch, obs,
discord, philips-hue, ...) are ported here incrementally — see
[`../docs/ROADMAP.md`](../docs/ROADMAP.md).
