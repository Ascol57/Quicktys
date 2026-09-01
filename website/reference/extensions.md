---
description: "The complete surface of every plugin, module and widget shipped with Quicktys: actions, triggers, state, services, events and configuration options."
---

# Shipped extensions

Everything in the Quicktys repository, with its complete surface. For a
task-oriented view, see [Integrations](/guide/integrations).

::: info Licence
These extensions are **proprietary and source-available**, © Ascol57. Read them,
build them, use them with Quicktys, modify them privately. Do not redistribute,
fork publicly, sell or reuse them. The public **ABI** is free for anyone to build
against — write your own against the same ABI. See
[Licensing](/contributing/license).
:::

## Plugins

### `alert`

Fire a named alert box on an overlay.

| Actions | Config |
| --- | --- |
| `show` | `name` (suggests alert widgets), `title`, `content`, `image` — all templatable |

Pushes `{ title, content, image, fired }` into `alert.<name>`. The alert widget
subscribed to that name animates on each new `fired`.

### `countdown`

Named countdown timers.

| Actions | Config |
| --- | --- |
| `start` | `name`, `duration`, `label` |
| `add` | `name`, `seconds` (negative removes time) |
| `pause` / `resume` / `stop` | `name` |
| `report-finished` | `name`, `seq` — fires the trigger; the widget calls it automatically |

| Triggers | Context |
| --- | --- |
| `finished` | `name`, `label` |

| State | Shape |
| --- | --- |
| `countdown.<name>` | `running`, `endAt`, `remainingMs`, `remaining`, `totalMs`, `display`, `label`, `seq` |

`endAt` is an absolute epoch-ms instant, so a widget derives the remaining time
locally. Timers persist through the plugin-tier `storage` capability.

### `discord`

Post through an incoming webhook. Requires the `http` module.

| Actions | Config |
| --- | --- |
| `webhook` | `url` (falls back to the setting), `content`, `username`, `avatar` |

| Settings | |
| --- | --- |
| `webhook` | default webhook URL |

### `input`

Requires the `input` module.

| Actions | Config |
| --- | --- |
| `type-text` | `text` |
| `hotkey` | `keys` — e.g. `ctrl+shift+c` |
| `mouse-click` | `button` |

### `notify`

No module.

| Actions | Config |
| --- | --- |
| `log` | `message` (template), `level` |
| `toast` | `message` (template), `level` |

`log` uses the safe `logging` capability; `toast` uses `reactivity.notify`.

### `obs`

Requires the `obs` module.

**Actions**

| Group | Actions |
| --- | --- |
| Scenes | `set-scene`, `set-studio-mode`, `studio-transition` |
| Sources | `set-source-visibility`, `set-source-transform`, `set-mute`, `toggle-mute`, `set-text`, `set-browser-url`, `refresh-browser`, `screenshot`, `add-browser-source` |
| Filters | `set-filter-visibility` |
| Media | `media-control` |
| Streaming | `start-stream`, `stop-stream`, `toggle-stream` |
| Recording | `start-record`, `stop-record`, `toggle-record`, `pause-record`, `resume-record` |
| Replay buffer | `start-replay`, `stop-replay`, `save-replay` |
| Virtual camera | `start-vcam`, `stop-vcam`, `toggle-vcam` |
| Other | `hotkey` |

**Triggers**

`scene-changed`, `stream-started`, `stream-stopped`, `record-started`,
`record-stopped`, `replay-started`, `replay-stopped`, `replay-saved`,
`vcam-started`, `vcam-stopped`, `studio-mode-enabled`, `studio-mode-disabled`,
`media-started`, `media-ended`

**State**

| Key | |
| --- | --- |
| `obs.scene` | current program scene |
| `obs.scenes` | list of scenes — feeds `suggest: "obs-scene"` |
| `obs.inputs` | list of sources — feeds `suggest: "obs-input"` |
| `obs.filters` | filters |
| `obs.hotkeys` | hotkeys |
| `obs.streaming` / `obs.recording` | booleans |
| `obs.replay-buffering` / `obs.virtual-cam` / `obs.studio-mode` | booleans |

**UI contributions** — an **Add to OBS** button in the overlay editor's toolbar
(`webview.toolbar`), running `add-browser-source` with the slot's `{ url, name }`.

### `os`

Requires the `os` module.

| Actions | Config |
| --- | --- |
| `shell` | `command` |
| `open` | `target` — file or URL |

### `overlays`

Requires the `webview` module. Installing this is what makes the overlay system
reachable.

| Actions | Config |
| --- | --- |
| `create` | `id`, `title` — creates, or renames an existing one |
| `delete` | `id` |
| `refresh` | — re-reads and republishes the list |

| State | |
| --- | --- |
| `overlays.count`, `overlays.ids` | |

### `pear`

Requires the `pear` module.

| Actions | Config |
| --- | --- |
| `play`, `pause`, `toggle`, `next`, `previous` | — |
| `like`, `dislike`, `shuffle`, `toggle-mute` | — |
| `set-volume` | `volume` |
| `seek-to` | `seconds` |

| Triggers | Context |
| --- | --- |
| `song-changed` | `title`, `artist`, `videoId` |

| State | |
| --- | --- |
| `pear.title`, `pear.artist`, `pear.artists`, `pear.cover`, `pear.url`, `pear.album`, `pear.video-id`, `pear.lyrics`, `pear.playlist` | mirrored on each song change |
| `pear.playing`, `pear.queue` | tracked by the module's poll |

### `random`

No module.

| Actions | Config |
| --- | --- |
| `number` | `min`, `max`, `integer` |
| `dice` | `count`, `sides` |
| `coin` | — |
| `chance` | `percent` |
| `pick` | `items` |

Bind the result and branch on it.

### `sound`

Requires the `sound` module.

| Actions | Config |
| --- | --- |
| `play` | `file`, `volume` (%) |
| `tts` | `text`, `voice` (suggests `tts-voice`) |

### `time`

No module. **Not shipped by default** — its `delay` is a pure ABI fixture that
does not actually wait. Use the `wait-for` node.

| Actions | Config |
| --- | --- |
| `delay` | `duration` |

### `twitch`

Requires the `twitch` module.

| Actions | Config |
| --- | --- |
| `chat` | `message` |
| `announce` | `message`, `color` |
| `watch-chat` | `channel` |
| `watch-events` | `channel` |

| Triggers | Needs an account |
| --- | --- |
| `chat-message` | no |
| `follow` | **yes** |
| `subscribe` | **yes** |
| `raid` | **yes** |

| State | |
| --- | --- |
| `twitch.chat` | the last 200 messages |
| `twitch.chat-channel` | the watched channel |
| `twitch.badges` | `set/version → image URL` |

## Modules

### `http` — certified, `net`

| Services | Args → Result |
| --- | --- |
| `request` | `{ method, url, headers, body }` → `{ status, body }` |

Generic and reusable. Declare `requires("http", "^0")` rather than writing a
network module for a REST integration.

### `input` — certified, `input`

| Services | |
| --- | --- |
| `type`, `key-combo`, `mouse-click` | |

### `obs` — certified, `net`

| Services | |
| --- | --- |
| `set-url`, `set-password` | connection settings |
| `poll` | called by the host every 3 s |
| `request` | raw obs-websocket request |
| `set-source-enabled`, `set-source-transform` | round-trip actions resolving the scene item id first |

| Events | |
| --- | --- |
| `scene-changed`, `media-started`, `media-ended`, `replay-saved` | real events → triggers |
| `stream-state`, `record-state`, `replay-state`, `vcam-state`, `studio-state` | status changes |
| `state` | bulk state sync — mirrored into `obs.*` **without** firing a trigger |

### `os` — certified, `process`

| Services | |
| --- | --- |
| `shell`, `open`, `run` | |

### `pear` — certified, `net`

| Services | |
| --- | --- |
| `play`, `pause`, `toggle`, `next`, `previous`, `like`, `dislike`, `shuffle`, `toggle-mute`, `set-volume`, `seek-to`, `set-queue-index` | playback |
| `current-song`, `get-volume` | queries |
| `poll` | called every 3 s; emits `song` and `song-changed` |
| `authorize`, `auth-status`, `clear-auth`, `set-client-id` | authentication |
| `set-base-url` | point at a stand-in server for tests |

| Events | |
| --- | --- |
| `song` | any change, **including the first observation** |
| `song-changed` | only a change from a **known** previous song |

That split is why launching Quicktys syncs the state without firing your song
alert.

### `sound` — certified, `audio`

| Services | |
| --- | --- |
| `play` | `{ path, volume }` — volume is a 0.0–1.0 gain |
| `speak` | `{ text, voice }` |
| `voices` | installed voices |

### `twitch` — certified, `net`

| Services | |
| --- | --- |
| `connect-chat` | joins `wss://irc-ws.chat.twitch.tv:443` **anonymously** |
| `connect-events` | opens EventSub and creates the subscriptions |
| `send-chat`, `announce` | need a token |
| `get-stream`, `helix` | Helix queries |
| `chat-badges` | merges global + channel badge images |
| `oauth-config` | what the app's device-code flow uses |
| `set-auth`, `set-client-id`, `auth-status` | |
| `set-base-url`, `set-chat-url`, `set-eventsub-url` | test hooks |

| Events | |
| --- | --- |
| `chat`, `follow`, `subscribe`, `raid` | |

Chat parses IRCv3 tags: colour, display name, badges (kept as `set/version`,
since the version selects the image) and emote positions. Emote positions are in
**code points** — split with `Array.from`, or an earlier emoji shifts every
emote.

EventSub exactness: `channel.follow` is **v2** (v1 is rejected) and needs both
`broadcaster_user_id` and `moderator_user_id` plus `moderator:read:followers`;
`channel.subscribe` is v1 with `channel:read:subscriptions`; `channel.raid` is v1,
no scope, and takes `to_` — `from_` and `to_` are mutually exclusive.
Subscriptions must be created **within 10 s** of `session_welcome` or Twitch
closes with 4003.

### `webview` — no capability beyond `storage`, uncertified

| Services | |
| --- | --- |
| `list-overlays`, `get-overlay`, `upsert-overlay`, `delete-overlay` | |

Owns overlay data in its own storage. Contributes the **Overlays** tab.

### `counter`, `netdemo` — test fixtures

Used by the host integration tests, not shipped.

## Widgets

| Widget | Kind | Requires | Default size |
| --- | --- | --- | --- |
| `info` | built-in | — | |
| `button` | built-in | — | |
| `alert` | runtime | `alert` | 2 × 2 |
| `countdown` | runtime | `countdown` | 2 × 1 |
| `twitch-chat` | runtime | `twitch` | 2 × 3 |
| `pear-now-playing` | runtime | `pear` | |
| `pear-music` | runtime | `pear` | 2 × 1 |
| `pear-queue` | runtime | `pear` | 2 × 3 |

### `info`

`source`: one of `plugins`, `modules`, `profiles`, `automations`, `widgets`,
`webview`.

### `button`

`label`, `icon`, `color`. Emits `button.clicked` with `{ id, label }`.

### `alert`

`name`, `duration` (500–60000 ms), `bgColor`, `titleColor`, `contentColor`,
`accentColor`, `radius`. Ships a `preview.html`, because it is invisible at rest.

### `countdown`

`name`, `format` (`auto` / `mm:ss` / `h:mm:ss` / `seconds`), `fontFamily`,
`fontWeight`, `fontScale`, `labelPosition`, `showLabel`, `showProgress`,
`showWhenIdle`, `showBackground`, `warnAt`, `textColor`, `labelColor`,
`warnColor`, `accentColor`, `bgColor`, `radius`.

### `twitch-chat`

`channel`, `maxMessages`, `newestAt`, `rowStyle` (`plain` / `striped` / `bubble` /
`card`), `fadeAfter`, `fontFamily`, `fontWeight`, `fontScale`, `nameColors`
(`twitch` / `auto` / `fixed`), `fixedNameColor`, `textColor`, `bgColor`,
`accentColor`, `radius`, `showBadges`, `showEmotes`, `showTimestamps`,
`showSeparators`, `showBackground`, `animate`.

Falls back to coloured text pills when the badge map is empty — an anonymous chat
has no token, so no images.

### `pear-music`

`showCover`, `showTitle`, `showArtists`, `showProgress`, `showTime`, `bgColor`,
`titleColor`, `artistColor`, `accentColor`, `barColor`, `radius`, `coverRadius`,
`titleSize`.

### `pear-now-playing` / `pear-queue`

No configuration.

## Next

- [Integrations](/guide/integrations) — what to use each one for.
- [Write a plugin](/dev/plugin) — add your own.
