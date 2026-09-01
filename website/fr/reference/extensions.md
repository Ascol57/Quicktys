---
description: "La surface complète de chaque plugin, module et widget livré avec Quicktys : actions, triggers, état, services, events et options de configuration."
---

# Extensions livrées

Tout ce que contient le dépôt Quicktys, avec sa surface complète. Pour une vue
orientée usage, voir [Intégrations](/fr/guide/integrations).

::: info Licence
Ces extensions sont **propriétaires et source-available**, © Ascol57. Lisez-les,
compilez-les, utilisez-les avec Quicktys, modifiez-les en privé. Ne les
redistribuez pas, ne les forkez pas publiquement, ne les vendez pas, ne les
réutilisez pas. L'**ABI** publique, elle, est libre : écrivez les vôtres contre
cette même ABI. Voir [Licences](/fr/contributing/license).
:::

## Plugins

### `alert`

Déclencher une boîte d'alerte nommée sur un overlay.

| Actions | Config |
| --- | --- |
| `show` | `name` (propose les widgets d'alerte), `title`, `content`, `image` — tous templatables |

Pousse `{ title, content, image, fired }` dans `alert.<nom>`. Le widget d'alerte
abonné à ce nom s'anime à chaque nouveau `fired`.

### `countdown`

Comptes à rebours nommés.

| Actions | Config |
| --- | --- |
| `start` | `name`, `duration`, `label` |
| `add` | `name`, `seconds` (négatif pour retirer du temps) |
| `pause` / `resume` / `stop` | `name` |
| `report-finished` | `name`, `seq` — déclenche le trigger ; le widget l'appelle tout seul |

| Triggers | Contexte |
| --- | --- |
| `finished` | `name`, `label` |

| État | Forme |
| --- | --- |
| `countdown.<nom>` | `running`, `endAt`, `remainingMs`, `remaining`, `totalMs`, `display`, `label`, `seq` |

`endAt` est un instant epoch-ms absolu, donc un widget en dérive le temps restant
localement. Les timers persistent via la capacité `storage` de l'étage plugin.

### `discord`

Poster via un webhook entrant. Requiert le module `http`.

| Actions | Config |
| --- | --- |
| `webhook` | `url` (retombe sur le réglage), `content`, `username`, `avatar` |

| Réglages | |
| --- | --- |
| `webhook` | URL de webhook par défaut |

### `input`

Requiert le module `input`.

| Actions | Config |
| --- | --- |
| `type-text` | `text` |
| `hotkey` | `keys` — par exemple `ctrl+shift+c` |
| `mouse-click` | `button` |

### `notify`

Pas de module.

| Actions | Config |
| --- | --- |
| `log` | `message` (template), `level` |
| `toast` | `message` (template), `level` |

`log` utilise la capacité sûre `logging` ; `toast` utilise `reactivity.notify`.

### `obs`

Requiert le module `obs`.

**Actions**

| Groupe | Actions |
| --- | --- |
| Scènes | `set-scene`, `set-studio-mode`, `studio-transition` |
| Sources | `set-source-visibility`, `set-source-transform`, `set-mute`, `toggle-mute`, `set-text`, `set-browser-url`, `refresh-browser`, `screenshot`, `add-browser-source` |
| Filtres | `set-filter-visibility` |
| Média | `media-control` |
| Streaming | `start-stream`, `stop-stream`, `toggle-stream` |
| Enregistrement | `start-record`, `stop-record`, `toggle-record`, `pause-record`, `resume-record` |
| Replay buffer | `start-replay`, `stop-replay`, `save-replay` |
| Caméra virtuelle | `start-vcam`, `stop-vcam`, `toggle-vcam` |
| Autre | `hotkey` |

**Triggers**

`scene-changed`, `stream-started`, `stream-stopped`, `record-started`,
`record-stopped`, `replay-started`, `replay-stopped`, `replay-saved`,
`vcam-started`, `vcam-stopped`, `studio-mode-enabled`, `studio-mode-disabled`,
`media-started`, `media-ended`

**État**

| Clé | |
| --- | --- |
| `obs.scene` | la scène programme courante |
| `obs.scenes` | la liste des scènes — alimente `suggest: "obs-scene"` |
| `obs.inputs` | la liste des sources — alimente `suggest: "obs-input"` |
| `obs.filters` | les filtres |
| `obs.hotkeys` | les raccourcis |
| `obs.streaming` / `obs.recording` | booléens |
| `obs.replay-buffering` / `obs.virtual-cam` / `obs.studio-mode` | booléens |

**Contributions d'UI** — un bouton **Ajouter à OBS** dans la barre d'outils de
l'éditeur d'overlays (`webview.toolbar`), qui lance `add-browser-source` avec le
`{ url, name }` du slot.

### `os`

Requiert le module `os`.

| Actions | Config |
| --- | --- |
| `shell` | `command` |
| `open` | `target` — fichier ou URL |

### `overlays`

Requiert le module `webview`. L'installer est ce qui rend le système d'overlays
atteignable.

| Actions | Config |
| --- | --- |
| `create` | `id`, `title` — crée, ou renomme un existant |
| `delete` | `id` |
| `refresh` | — relit et republie la liste |

| État | |
| --- | --- |
| `overlays.count`, `overlays.ids` | |

### `pear`

Requiert le module `pear`.

| Actions | Config |
| --- | --- |
| `play`, `pause`, `toggle`, `next`, `previous` | — |
| `like`, `dislike`, `shuffle`, `toggle-mute` | — |
| `set-volume` | `volume` |
| `seek-to` | `seconds` |

| Triggers | Contexte |
| --- | --- |
| `song-changed` | `title`, `artist`, `videoId` |

| État | |
| --- | --- |
| `pear.title`, `pear.artist`, `pear.artists`, `pear.cover`, `pear.url`, `pear.album`, `pear.video-id`, `pear.lyrics`, `pear.playlist` | mirrorés à chaque changement de musique |
| `pear.playing`, `pear.queue` | suivis par le poll du module |

### `random`

Pas de module.

| Actions | Config |
| --- | --- |
| `number` | `min`, `max`, `integer` |
| `dice` | `count`, `sides` |
| `coin` | — |
| `chance` | `percent` |
| `pick` | `items` |

Liez le résultat et branchez dessus.

### `sound`

Requiert le module `sound`.

| Actions | Config |
| --- | --- |
| `play` | `file`, `volume` (%) |
| `tts` | `text`, `voice` (propose `tts-voice`) |

### `time`

Pas de module. **Non livré par défaut** — son `delay` est une fixture d'ABI pure
qui n'attend pas réellement. Utilisez le nœud `wait-for`.

| Actions | Config |
| --- | --- |
| `delay` | `duration` |

### `twitch`

Requiert le module `twitch`.

| Actions | Config |
| --- | --- |
| `chat` | `message` |
| `announce` | `message`, `color` |
| `watch-chat` | `channel` |
| `watch-events` | `channel` |

| Triggers | Compte requis |
| --- | --- |
| `chat-message` | non |
| `follow` | **oui** |
| `subscribe` | **oui** |
| `raid` | **oui** |

| État | |
| --- | --- |
| `twitch.chat` | les 200 derniers messages |
| `twitch.chat-channel` | la chaîne suivie |
| `twitch.badges` | `set/version → URL d'image` |

## Modules

### `http` — certifié, `net`

| Services | Args → Résultat |
| --- | --- |
| `request` | `{ method, url, headers, body }` → `{ status, body }` |

Générique et réutilisable. Déclarez `requires("http", "^0")` plutôt que d'écrire
un module réseau pour une intégration REST.

### `input` — certifié, `input`

| Services | |
| --- | --- |
| `type`, `key-combo`, `mouse-click` | |

### `obs` — certifié, `net`

| Services | |
| --- | --- |
| `set-url`, `set-password` | réglages de connexion |
| `poll` | appelé par le host toutes les 3 s |
| `request` | requête obs-websocket brute |
| `set-source-enabled`, `set-source-transform` | actions à aller-retour, qui résolvent d'abord l'id de l'élément de scène |

| Events | |
| --- | --- |
| `scene-changed`, `media-started`, `media-ended`, `replay-saved` | vrais events → triggers |
| `stream-state`, `record-state`, `replay-state`, `vcam-state`, `studio-state` | changements de statut |
| `state` | synchronisation d'état en bloc — mirrorée dans `obs.*` **sans** déclencher de trigger |

### `os` — certifié, `process`

| Services | |
| --- | --- |
| `shell`, `open`, `run` | |

### `pear` — certifié, `net`

| Services | |
| --- | --- |
| `play`, `pause`, `toggle`, `next`, `previous`, `like`, `dislike`, `shuffle`, `toggle-mute`, `set-volume`, `seek-to`, `set-queue-index` | lecture |
| `current-song`, `get-volume` | requêtes |
| `poll` | appelé toutes les 3 s ; émet `song` et `song-changed` |
| `authorize`, `auth-status`, `clear-auth`, `set-client-id` | authentification |
| `set-base-url` | pointer vers un serveur bouchon pour les tests |

| Events | |
| --- | --- |
| `song` | tout changement, **y compris la première observation** |
| `song-changed` | uniquement un changement depuis une musique **connue** |

C'est cette distinction qui fait que lancer Quicktys synchronise l'état sans
déclencher votre alerte de musique.

### `sound` — certifié, `audio`

| Services | |
| --- | --- |
| `play` | `{ path, volume }` — le volume est un gain 0.0–1.0 |
| `speak` | `{ text, voice }` |
| `voices` | les voix installées |

### `twitch` — certifié, `net`

| Services | |
| --- | --- |
| `connect-chat` | rejoint `wss://irc-ws.chat.twitch.tv:443` **anonymement** |
| `connect-events` | ouvre EventSub et crée les abonnements |
| `send-chat`, `announce` | demandent un jeton |
| `get-stream`, `helix` | requêtes Helix |
| `chat-badges` | fusionne les images de badges globales et de la chaîne |
| `oauth-config` | ce qu'utilise le flux device code de l'app |
| `set-auth`, `set-client-id`, `auth-status` | |
| `set-base-url`, `set-chat-url`, `set-eventsub-url` | crochets de test |

| Events | |
| --- | --- |
| `chat`, `follow`, `subscribe`, `raid` | |

Le chat analyse les tags IRCv3 : couleur, nom d'affichage, badges (gardés en
`set/version`, puisque la version sélectionne l'image) et positions d'emotes. Les
positions d'emotes sont en **points de code** — découpez avec `Array.from`, sinon
un emoji antérieur décale toutes les emotes.

Exigences d'EventSub : `channel.follow` est en **v2** (la v1 est rejetée) et
demande à la fois `broadcaster_user_id` et `moderator_user_id`, plus
`moderator:read:followers` ; `channel.subscribe` est en v1 avec
`channel:read:subscriptions` ; `channel.raid` est en v1, sans scope, et prend
`to_` — `from_` et `to_` sont mutuellement exclusifs. Les abonnements doivent être
créés **dans les 10 s** suivant `session_welcome`, sinon Twitch ferme avec un
4003.

### `webview` — aucune capacité au-delà de `storage`, non certifié

| Services | |
| --- | --- |
| `list-overlays`, `get-overlay`, `upsert-overlay`, `delete-overlay` | |

Possède les données d'overlay dans son propre stockage. Contribue l'onglet
**Overlays**.

### `counter`, `netdemo` — fixtures de test

Utilisés par les tests d'intégration du host, non livrés.

## Widgets

| Widget | Nature | Requiert | Taille par défaut |
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

`source` : parmi `plugins`, `modules`, `profiles`, `automations`, `widgets`,
`webview`.

### `button`

`label`, `icon`, `color`. Émet `button.clicked` avec `{ id, label }`.

### `alert`

`name`, `duration` (500–60000 ms), `bgColor`, `titleColor`, `contentColor`,
`accentColor`, `radius`. Livre un `preview.html`, puisqu'il est invisible au
repos.

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

Retombe sur des pastilles de texte colorées quand la table de badges est vide — un
chat anonyme n'a pas de jeton, donc pas d'images.

### `pear-music`

`showCover`, `showTitle`, `showArtists`, `showProgress`, `showTime`, `bgColor`,
`titleColor`, `artistColor`, `accentColor`, `barColor`, `radius`, `coverRadius`,
`titleSize`.

### `pear-now-playing` / `pear-queue`

Aucune configuration.

## Et ensuite

- [Intégrations](/fr/guide/integrations) — à quoi sert chacune.
- [Écrire un plugin](/fr/dev/plugin) — ajouter la vôtre.
