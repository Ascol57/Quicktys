# Host capabilities

Every interface an extension can import, from
[`wit/host.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/host.wit).

Legend for the **Status** column:

| | |
| --- | --- |
| ✅ | implemented and used in production |
| 🟡 | partially implemented |
| ⛔ | declared in the ABI, not implemented yet |

## Shared — both tiers

### `logging`

```wit
enum level { trace, debug, info, warn, error }
log: func(lvl: level, message: string);
```

✅ Routed to the host's `tracing` subscriber.

### `storage`

```wit
kv-get: func(key: string) -> option<string>;
kv-set: func(key: string, value: string);
get-secret: func(key: string) -> option<string>;
```

✅ Persisted per extension to `<user-data>/quicktys/storage/<id>.json`. Values are
strings — serialise your own JSON.

No certification required. This is how a module that owns data — `webview` and its
overlays — survives a restart.

## Plugin-facing

### `reactivity`

```wit
set-state: func(key: string, value-json: string);
emit-trigger: func(trigger-id: string, context-json: string);
notify: func(message: string, level: string);
```

| Function | Notes |
| --- | --- |
| `set-state` | ✅ The key is **local**; the host namespaces it `<plugin-id>.<key>` in the state store and propagates to the UI. |
| `emit-trigger` | ✅ Fires `<plugin-id>.<trigger-id>` on the trigger bus. The JSON becomes the trigger context. |
| `notify` | ✅ A transient toast in the UI. `level` is `info`, `success`, `warn` or `error`. |

### `modules`

```wit
variant call-error { not-installed, no-such-method, denied, failed(string) }

call: func(module-id: string, method: string, args-json: string) -> result<string, call-error>;
subscribe: func(module-id: string, event-id: string) -> result<_, call-error>;
unsubscribe: func(module-id: string, event-id: string);
```

✅ The host enforces that the plugin declared the dependency, that the method or
event exists, and that payloads validate against the module's published schemas.
Subscribed events arrive at `plugin-guest.on-module-event`.

## Module-facing

### `events`

```wit
emit-event: func(event-id: string, payload-json: string);
```

✅ Fans out to every plugin subscribed to that event.

### `oauth`

```wit
record provider-config {
    auth-url: string, token-url: string,
    client-id: string, client-secret: option<string>,
    scopes: list<string>, redirect: string, use-pkce: bool,
}
authorize: func(provider: string, config: provider-config) -> result<_, string>;
access-token: func(provider: string) -> result<string, string>;
is-authorized: func(provider: string) -> bool;
revoke: func(provider: string);
```

| Function | Status |
| --- | --- |
| `access-token` | ✅ Returns a valid token, refreshing transparently. **This is the one to call.** |
| `is-authorized` | ✅ |
| `revoke` | ✅ |
| `authorize` | ⛔ Deliberately a stub. |

::: info Why `authorize` is a stub
It is a **synchronous** call with nowhere to hand back a user code for the UI to
display while waiting for approval. The interactive half therefore runs UI-side
over IPC: Settings → `oauth_connect` / `oauth_poll` / `oauth_status` /
`oauth_disconnect`.

The division of responsibility is preserved: the backend asks your module for its
**`oauth-config`** service, so no provider specifics enter the core, and your
module reads its token through `access-token` as designed.
:::

Tokens are persisted to `<user-data>/quicktys/oauth.json` (`0600` on Unix),
including the provider environment a refresh needs. Expiry is stored as absolute
Unix seconds — a monotonic `Instant` would be meaningless across the restart at
which it is read.

## System capabilities

Granted only to **certified** modules, only when declared, and only when
consented to. All three are checked on **every dispatch**.

### `net`

```wit
variant net-error { refused, timeout, tls(string), other(string) }

tcp-connect: func(host: string, port: u16, event-id: string) -> result<u64, net-error>;
ws-connect:  func(url: string, event-id: string) -> result<u64, net-error>;
send:  func(handle: u64, data: list<u8>) -> result<_, net-error>;
close: func(handle: u64);

record http-request  { method: string, url: string,
                       headers: list<tuple<string, string>>, body: option<list<u8>> }
record http-response { status: u16, headers: list<tuple<string, string>>, body: list<u8> }
fetch: func(req: http-request) -> result<http-response, net-error>;
```

| Function | Status |
| --- | --- |
| `fetch` | ✅ Real HTTP client. `http://` over plain TCP, `https://` over rustls with bundled webpki roots. |
| `ws-connect` | ✅ Both `ws://` and `wss://`. |
| `send` | ✅ |
| `close` | ✅ |
| `tcp-connect` | 🟡 Raw TCP is present but less exercised than the WebSocket path. |

Inbound data arrives at `module-guest.on-host-event` tagged with the `event-id`
you passed.

Three implementation details that are contract, not accident:

- **`fetch` frames the response by `Content-Length` or chunked encoding**, and does
  **not** half-close the request side. A server running an async handler abandons
  the response if the client sends FIN first, and a keep-alive server never
  closes — so reading until EOF would hang or return empty.
- **`ws-connect` connects synchronously** before returning the handle, and
  registers the write half first. Otherwise a protocol where the **client speaks
  first** (Twitch IRC) would race the dial and have its first sends dropped.
- **`close` shuts down both halves.** Dropping only the write half leaves the
  reader parked on a live socket, still delivering — which is how a rejoined chat
  delivers every message twice.

For `wss://`, one TLS session encrypts both directions and cannot be cloned. The
session is shared behind a mutex, and the reader does its **blocking socket read
outside the lock** so a blocked read never starves a send.

### `audio`

```wit
list-output-devices: func() -> list<string>;
play-buffer: func(device: option<string>, pcm: list<u8>, sample-rate: u32, channels: u8) -> result<_, string>;
play-file: func(path: string, volume: f32) -> result<_, string>;
speak: func(text: string, voice: string) -> result<_, string>;
list-voices: func() -> list<string>;
```

| Function | Status |
| --- | --- |
| `play-file` | ✅ Decodes wav / mp3 / ogg / flac and plays host-side on a dedicated thread. A graceful no-op when there is no device. |
| `speak` / `list-voices` | ✅ Platform TTS — `say`, SAPI, speech-dispatcher. `voice` is a platform voice name on macOS and Windows, a language code on Linux; empty means the OS default. |
| `list-output-devices` | 🟡 |
| `play-buffer` | ⛔ |

### `input`

```wit
send-key: func(code: u32, down: bool);
send-mouse-move: func(x: s32, y: s32);
send-mouse-button: func(button: u8, down: bool);
type-text: func(text: string) -> result<_, string>;
key-combo: func(keys: list<string>) -> result<_, string>;
mouse-click: func(button: u8) -> result<_, string>;
```

✅ The high-level helpers (`type-text`, `key-combo`, `mouse-click`) are real,
backed by `enigo` — the x11rb backend on Linux, so no libxdo. Needs a real desktop
session.

`key-combo` takes a chord such as `["ctrl", "shift", "c"]`; modifiers are held
around the last key. Mouse buttons: 0 left, 1 middle, 2 right.

### `process`

```wit
spawn: func(program: string, args: list<string>) -> result<u64, string>;
kill: func(handle: u64);
run-shell: func(command: string) -> result<u64, string>;
open-path: func(target: string) -> result<_, string>;
```

✅ All four, cross-platform. `run-shell` uses `cmd /C` on Windows and `sh -c`
elsewhere; `open-path` uses `start` / `open` / `xdg-open`.

::: danger The sharpest capability in the list
This starts programs on the user's machine. Declare it only if you truly need it,
and be careful about building a command from a template that could include chat
input.
:::

### `files`

```wit
read: func(path: string) -> result<list<u8>, string>;
write: func(path: string, data: list<u8>) -> result<_, string>;
list-dir: func(path: string) -> result<list<string>, string>;
```

⛔ Declared; scoped to a module's own sandboxed directory by design. Wired when a
driver needs it.

(`list` is the WIT `list<T>` type, hence `list-dir`.)

### `hid`

```wit
enumerate: func(vendor-id: option<u16>, product-id: option<u16>) -> list<string>;
open: func(path: string, event-id: string) -> result<u64, string>;
write: func(handle: u64, data: list<u8>) -> result<_, string>;
close: func(handle: u64);
```

⛔ Kept as generic infrastructure — a Stream Deck or Companion driver would use it
— but **no shipped extension uses it today**. The remote-surface protocol that
did was removed in favour of the far simpler Button widget.

### `timers`

```wit
set-timer: func(ms: u64, repeat: bool, event-id: string) -> u64;
clear-timer: func(handle: u64);
```

⛔ Not implemented. In the meantime the app runs a **poller** that calls the
`poll` service of every module declaring one, every three seconds. A real
`timers` capability will replace it.

This is why anything periodic belongs in a module's `poll` service, and why a
plugin — which is never polled — cannot tick anything on its own.

## The three gates

For a system capability call to succeed:

```
granted = certified && declared && consented
```

| Gate | Decided by | Changeable |
| --- | --- | --- |
| **Certified** | cryptography — the signature chain against pinned CA keys | no |
| **Declared** | the module's manifest | at publish time |
| **Consented** | the user, in Settings | any time, effective immediately |

An update that **widens** a module's capabilities has the new ones refused until
reviewed again. A module never reviewed is granted what it declared, so nothing
breaks before the user first opens the consent screen; after review, only what was
granted is allowed.

## Next

- [Worlds & lifecycle](/reference/worlds)
- [Write a module](/dev/module)
