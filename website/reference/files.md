# Files & data directory

## The data directory

```
<user-data>/quicktys/
├── settings/          app and extension settings
├── profiles/          one <id>.yaml per profile
├── automations/       one <id>.yaml per standalone automation
├── queues/            action queue configuration
├── accounts/          account resources
├── secrets/           secret values
├── state/             persisted reactive state
├── storage/           <id>.json — the `storage` capability, per extension
├── logs/
├── installed/         extensions materialised by the marketplace
├── installed.json     the install lockfile
├── consent.json       per-module capability grants
├── entitlements.json  licence keys you have entered
├── oauth.json         token sets + provider environments (0600 on unix)
└── registry-cache/    downloaded index, CRL and blobs
```

::: warning The location is currently hardcoded
It is built as `$HOME/.local/share/quicktys`. That works on macOS but is not the
convention there (`~/Library/Application Support`), and on **Windows `HOME` is not
set** — the app would fall back to `%TEMP%` and lose everything on restart.
`run-windows.ps1` sets `HOME=%USERPROFILE%` to work around it.
:::

## Persistence rules

Everything user-facing is **YAML**, one file per resource, written **atomically**
(temp file, then rename). Ids are sanitised into filenames.

Automations store expressions as their **source text**; the syntax tree is
compiled at load, so a broken expression is an error you can see before it is a
runtime surprise.

## `profiles/<id>.yaml`

```yaml
id: gaming
name: Gaming
activation: 'obs.scene == "Game"'
bindings:
  - trigger: twitch.chat-message
    filter: 'message.starts_with("!")'
    sequence:
      kind: block
      body:
        - kind: action
          plugin: notify
          action: log
          config: { message: "{{ user }}: {{ message }}" }
```

A profile owns its sequences inline rather than referring to automations by id.

## `automations/<id>.yaml`

The node tree, `kind`-tagged. See [Automation nodes](/reference/nodes).

## `installed/` and `installed.json`

```
installed/
├── modules/<id>/{<id>.wasm, lang/, bundle.json}
├── plugins/<id>/{<id>.wasm, lang/, ui/, bundle.json}
└── widgets/<id>/{bundle.json, lang/, *.html}
```

Exactly the shape the boot scanner already reads. The installer extracts a
`.qtpkg` atomically (`.tmp/` then rename) and writes a compatible `bundle.json`
from the package's `manifest.json`. Widgets keep their authored `bundle.json`,
which is richer.

`installed.json` is the source of truth: versions, hashes, source, and
`required_by` — the **reference count** that identifies orphaned modules. A module
installed only as a dependency is marked non-explicit and becomes purgeable when
nothing needs it.

## `bundle.json`

What the host reads at load time:

```json
{ "id": "obs", "kind": "module", "wasm": "obs.wasm", "certified": true }
```

`certified` is **computed at install** from the signature chain, not read from a
file someone can edit. A widget's `bundle.json` is authored and much richer — see
[Write a widget](/dev/widget).

## `consent.json`

Per-module capability grants. The model is **grace until review**: a module you
have never reviewed is allowed what it declared, so nothing breaks before you
first open the consent screen; afterwards, only granted capabilities are allowed.

Revoking takes effect immediately. An update that widens a module's capabilities
has the new ones refused until re-reviewed.

## `oauth.json`

Token sets **and** the provider environments a refresh needs — both, because
refreshing after a restart requires the environment too. `0600` on Unix.

Expiry is stored as absolute **Unix seconds**: a monotonic `Instant` is
meaningless across the restart at which it is read.

A token that is dead and cannot be refreshed is dropped rather than reloaded as a
fake "connected".

::: warning Still plain JSON
Protected from other users of the machine, not from anything running under your
own account. The OS keyring is the next step.
:::

## Bundle search order at boot

1. `dist/bundles/` — the development tree (skipped when
   `QUICKTYS_NO_DEV_BUNDLES=1`)
2. `<user-data>/quicktys/installed/` — the marketplace tree

**Modules load before plugins.** An id already loaded is skipped, so an installed
bundle shadows a development bundle with the same id.

## Ports

| Port | |
| --- | --- |
| **8787** | the app's local HTTP server — overlays (`/overlay/<id>`), widget assets (`/widget/<id>/…`), the SSE state stream (`/events`), and `POST /action` |
| **8000** | `quicktys-regserve`'s default, in development |

## URI schemes

Each sandboxed surface gets its own origin, so it has its own CSP and cannot read
the app's storage:

| Scheme | Serves |
| --- | --- |
| `widget://localhost/<id>/` | a runtime widget's files (`http://widget.localhost/<id>/` on Windows) |
| `pluginui://<id>/<file>` | a plugin's custom slot UI, from `plugins/<id>/ui/` |
| `moduleui://<id>/<file>` | a module's contributed view, from `modules/<id>/ui/` |

## Backing up

Copy `<user-data>/quicktys/`. Profiles, automations and settings are plain YAML —
diffable, portable between machines, and reviewable in git if you want.

The dashboard layout is **not** in there: it lives in the frontend's local
storage, per machine.

## Next

- [Command-line tools](/reference/cli)
- [Architecture](/contributing/architecture)
