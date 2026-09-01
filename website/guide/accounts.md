# Connecting accounts

Some things need to know who you are. Reading a Twitch chat does not; posting to
it does, and so does knowing about your followers.

<Screenshot src="/screens/settings-accounts.png" caption="Settings → Accounts" />

## Twitch

### What works without an account

**Live chat display works with no account at all.** The Twitch module joins as an
anonymous read-only user, which is everything a chat *display* needs — no token
crosses the wire and the chat widget works out of the box.

What needs an account:

| Feature | Needs a connected account |
| --- | --- |
| Show chat in a widget or overlay | no |
| The `twitch.chat-message` trigger | no |
| Send a chat message or announcement | **yes** |
| Follow / subscribe / raid triggers (EventSub) | **yes** |
| Badge images in the chat widget | **yes** (anonymous chat has no token, so badges fall back to coloured text pills) |

### You need your own Client ID

Quicktys does not ship a Twitch application ID. You register one — it takes two
minutes and is free.

1. Go to [dev.twitch.tv/console/apps](https://dev.twitch.tv/console/apps) and
   click **Register Your Application**.
2. **Name**: anything, but it must be unique across all Twitch apps. If it is
   rejected, add a suffix.
3. **OAuth Redirect URL**: `http://localhost`.
4. **Category**: pick anything sensible, e.g. *Application Integration*.
5. Create it, then copy the **Client ID**.

<Screenshot src="/screens/twitch-app-registration.png" caption="Registering an application on dev.twitch.tv" />

::: info Two things that look wrong but are not
The redirect URL is **never used**. Quicktys uses the Device Code flow, which has
no redirect. Twitch's form insists on the field, so put `http://localhost` in it
and forget it.

You do **not** need a client secret. Do not generate one, and if you already did,
Quicktys will not ask for it.
:::

### Connecting

**Settings → Accounts → Twitch**: paste your Client ID, click **Connect**.

A dialog shows a short **user code** and a URL. Open the URL — on this machine or
on your phone — sign in, enter the code, approve. The dialog closes on its own
once Twitch confirms.

<Screenshot src="/screens/twitch-device-code.png" caption="The device code dialog" />

### Why device code

Twitch's Authorization Code flow requires a **client secret**, which cannot
safely ship in a distributed binary — and Twitch does not support PKCE, which is
the normal way around that. The Implicit flow returns no refresh token, so you
would be reconnecting constantly.

Device Code needs only a **public** client ID, returns a refresh token, and needs
neither a loopback server nor a declared redirect URI. It is the only one of the
three that fits a desktop app that anyone can download.

### Scopes

Requested when you connect:

| Scope | For |
| --- | --- |
| `moderator:read:followers` | the follow trigger (EventSub `channel.follow` v2) |
| `channel:read:subscriptions` | the subscribe trigger |
| *(none)* | raids — `channel.raid` needs no scope |

### Tokens on disk

Tokens are stored in `<user-data>/quicktys/oauth.json`, `0600` on Unix. Both the
token set and the provider configuration a refresh needs are persisted, so a
restart does not log you out. A token that is dead and cannot be refreshed is
dropped rather than reloaded as a fake "connected".

::: warning Plain JSON, for now
The file is protected from other users of the machine, not from anything running
under your own account. Moving to the OS keyring is the next step. Treat it like
any other credential file.
:::

**Disconnect** in Settings forgets the token set.

## OBS Studio

OBS uses a password, not OAuth.

1. In OBS: **Tools → WebSocket Server Settings**. Enable the server, note the port
   (4455 by default) and the password.
2. In Quicktys: **Settings → OBS**, fill in the URL (`ws://127.0.0.1:4455`) and
   the password.

Once connected, the OBS plugin's state fills in immediately — the current scene,
the list of scenes and sources, streaming and recording status, the profile,
resolution, FPS and performance counters. All of it is available to conditions
and templates, and the scene and source pickers autocomplete from it.

## Pear Desktop

Pear Desktop (a YouTube Music player) authenticates with a token minted by the
player itself.

The first time Quicktys asks, **the player shows an Allow / Deny dialog on your
screen**. Approve it once and re-authentication is silent afterwards.

::: warning If you click Deny
The denial is **sticky**. Quicktys will not ask again on its own — deliberately,
because it polls the player every three seconds and a naive retry would pop a
dialog at you forever. Use the **Connect** action to ask again when you are
ready.
:::

If you point Quicktys at a different player instance, or change its client id,
the token is dropped and consent is asked again — a different instance is a
different identity.

## Discord

No account needed. The Discord plugin posts through an **incoming webhook**:
create one in your server's channel settings, and paste the URL into the plugin's
settings (or into an individual action).

## Next

- [Integrations](/guide/integrations) — what each plugin can do once connected.
- [Troubleshooting](/guide/troubleshooting) — when it does not.
