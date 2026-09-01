---
description: "Every integration shipped with Quicktys: Twitch, OBS Studio, Pear Desktop, Discord, sound and TTS, overlays, alerts, countdowns, input and shell commands."
---

# Integrations

What ships today, what each one needs, and what it gives you. Exhaustive lists of
actions, triggers, state and services are in
[Shipped extensions](/reference/extensions).

Remember the split: a **plugin** is what you see in the editor; a **module** is
the certified driver it talks to. You install the plugin; the module comes along
if it is needed.

## Twitch

**Plugin** `twitch` · **module** `twitch` (certified, needs `net`)

| | |
| --- | --- |
| **Do** | Send a chat message. Post an announcement (with a colour). Start watching a channel's chat. Start watching follows, subs and raids. |
| **React to** | Chat message. New follower. New subscriber. Raid received. |
| **Read** | `twitch.chat` (the last 200 messages), `twitch.chat-channel`, `twitch.badges` |
| **Needs** | An account for sending and for follow/sub/raid. Chat display needs nothing. |

Chat is the real IRC feed with IRCv3 tags: colours, display names, badges and
emote positions are all parsed, so the chat widget renders properly rather than
as plain text.

Follows, subs and raids come over EventSub. There are exact-shape requirements
behind that which the module handles for you — `channel.follow` must be v2, and
subscriptions have to be created within ten seconds of the socket opening or
Twitch hangs up.

See [Connecting accounts](/guide/accounts#twitch).

## OBS Studio

**Plugin** `obs` · **module** `obs` (certified, needs `net`)

| | |
| --- | --- |
| **Do** | Change scene. Show/hide a source. Move, scale and rotate a source. Toggle a filter. Mute. Set a text source. Set a browser URL. Refresh a browser. Screenshot a source. Media controls. Trigger a hotkey. Start/stop/toggle streaming, recording, replay buffer and virtual camera. Pause and resume recording. Save the replay buffer. Studio mode and studio transitions. Add a browser source. |
| **React to** | Scene changed. Stream started/stopped. Recording started/stopped. Replay buffer started/stopped/saved. Virtual camera started/stopped. Studio mode enabled/disabled. Media started/ended. |
| **Read** | `obs.scene`, `obs.scenes`, `obs.inputs`, `obs.filters`, `obs.hotkeys`, `obs.streaming`, `obs.recording`, `obs.replay-buffering`, `obs.virtual-cam`, `obs.studio-mode` |
| **Needs** | obs-websocket v5 (built into OBS 28+) enabled, with its URL and password. |

State is populated as soon as the connection is up, not lazily — which is why
scene and source fields **autocomplete** with your actual scenes and sources.

The plugin also contributes an **Add to OBS** button into the overlay editor's
toolbar: one click creates the browser source for the overlay you are editing.

## Pear Desktop

**Plugin** `pear` · **module** `pear` (certified, needs `net`)

| | |
| --- | --- |
| **Do** | Play, pause, toggle. Next and previous track. Like, dislike. Shuffle. Toggle mute. Set volume. Seek. |
| **React to** | Song changed. |
| **Read** | `pear.title`, `pear.artist`, `pear.artists`, `pear.cover`, `pear.url`, `pear.album`, `pear.video-id`, `pear.lyrics`, `pear.playlist`, `pear.playing`, `pear.queue` |
| **Needs** | Pear Desktop running, and a one-time Allow in the player. |

The three Pear widgets (Now Playing, Music, Queue) are driven from this state.

::: info The first observation never fires the trigger
Starting Quicktys while music is already playing syncs `pear.*` but does **not**
fire *Song changed*. Only a change from a known previous song does. Otherwise
every launch would fire your song alert.
:::

## Overlays

**Plugin** `overlays` · **module** `webview`

| | |
| --- | --- |
| **Do** | Create an overlay (or rename one). Delete an overlay. Refresh the list. |
| **Read** | `overlays.count`, `overlays.ids` |

Installing this is what gives you the Overlays tab and the `/overlay/<id>` pages.
See [Overlays](/guide/overlays).

## Alerts

**Plugin** `alert` · widget `alert`

One action, **Show alert**, taking a name, a title, content and an image URL. It
pushes into `alert.<name>`, and the alert widget subscribed to that name animates
in.

The name field autocompletes with the alert widgets you have actually placed, on
the dashboard **and** in overlays. Title, content and image are template fields,
so they can carry `@user` or `{{ message }}`.

Put alerts in an [action queue](/guide/automations#action-queues) so two
followers arriving at once do not overlap.

## Countdown

**Plugin** `countdown` · widget `countdown`

| | |
| --- | --- |
| **Do** | Start (or restart) a named countdown. Add time (negative takes it away). Pause, resume, stop. Signal the end early. |
| **React to** | Countdown finished. |
| **Read** | `countdown.<name>` — `running`, `endAt`, `remainingMs`, `remaining`, `display`, `label` |

Timers survive a restart. See [the note on who
ticks](/guide/dashboard#countdowns-who-does-the-ticking) — the *finished* trigger
needs the countdown to be displayed somewhere.

## Sound

**Plugin** `sound` · **module** `sound` (certified, needs `audio`)

Play a sound file (wav, mp3, ogg, flac) at a chosen volume, decoded and mixed by
the host. Also **text to speech**, with a voice or language picked from what your
OS has installed.

## Discord

**Plugin** `discord` · **module** `http` (certified, needs `net`)

Post a message to a Discord channel through an incoming webhook, with an optional
username and avatar override. No Discord account or bot needed.

Note there is no Discord-specific module: the plugin goes through the generic
**HTTP** module, which is reusable by any plugin that declares it.

## Operating system

**Plugin** `os` · **module** `os` (certified, needs `process`)

Run a shell command. Open a file or a URL with the system default handler.

::: warning This one really does run commands
`process` is the capability that lets a module start programs on your machine. It
is worth being deliberate about consenting to it, and about what you put in the
command field — especially if the command is built from a template that includes
chat input.
:::

## Input

**Plugin** `input` · **module** `input` (certified, needs `input`)

Type text, press a hotkey combination (`ctrl+shift+c`), click a mouse button.
Needs a real desktop session.

## Notify

**Plugin** `notify` — no module.

Write a log line, or show a toast in the app. Both take a template message and a
level. The simplest possible plugin, and useful for debugging a sequence.

## Random

**Plugin** `random` — no module.

Random number in a range (whole or not), roll dice, flip a coin, roll a
percentage chance, pick from a list. Bind the result and branch on it.

## Time

**Plugin** `time` — no module.

A delay action. Note that this one is **not** installed by default: its `delay` is
a pure ABI test fixture that does not actually wait. Use the **Wait** node
instead, which is the real thing.

## Next

- [Shipped extensions](/reference/extensions) — the exhaustive tables.
- [Write a plugin](/dev/plugin) — add your own.
