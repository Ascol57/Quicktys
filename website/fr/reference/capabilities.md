---
description: "Chaque capacité du host Quicktys avec sa signature WIT et son état d'implémentation : logging, storage, reactivity, modules, net, audio, input, process et les autres."
---

# Capacités du host

Toutes les interfaces qu'une extension peut importer, depuis
[`wit/host.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/host.wit).

Légende de la colonne **État** :

| | |
| --- | --- |
| ✅ | implémenté et utilisé en production |
| 🟡 | partiellement implémenté |
| ⛔ | déclaré dans l'ABI, pas encore implémenté |

## Partagé — les deux étages

### `logging`

```wit
enum level { trace, debug, info, warn, error }
log: func(lvl: level, message: string);
```

✅ Redirigé vers le souscripteur `tracing` du host.

### `storage`

```wit
kv-get: func(key: string) -> option<string>;
kv-set: func(key: string, value: string);
get-secret: func(key: string) -> option<string>;
```

✅ Persisté par extension dans `<user-data>/quicktys/storage/<id>.json`. Les
valeurs sont des chaînes — sérialisez votre propre JSON.

Aucune certification requise. C'est ainsi qu'un module qui possède des données —
`webview` et ses overlays — survit à un redémarrage.

## Côté plugin

### `reactivity`

```wit
set-state: func(key: string, value-json: string);
emit-trigger: func(trigger-id: string, context-json: string);
notify: func(message: string, level: string);
```

| Fonction | Notes |
| --- | --- |
| `set-state` | ✅ La clé est **locale** ; le host la namespace en `<plugin-id>.<clé>` dans le store d'état et propage vers l'UI. |
| `emit-trigger` | ✅ Émet `<plugin-id>.<trigger-id>` sur le bus de triggers. Le JSON devient le contexte du trigger. |
| `notify` | ✅ Un toast transitoire dans l'UI. `level` vaut `info`, `success`, `warn` ou `error`. |

### `modules`

```wit
variant call-error { not-installed, no-such-method, denied, failed(string) }

call: func(module-id: string, method: string, args-json: string) -> result<string, call-error>;
subscribe: func(module-id: string, event-id: string) -> result<_, call-error>;
unsubscribe: func(module-id: string, event-id: string);
```

✅ Le host vérifie que le plugin a déclaré la dépendance, que la méthode ou
l'event existe, et que les payloads valident contre les schémas publiés par le
module. Les events souscrits arrivent à `plugin-guest.on-module-event`.

## Côté module

### `events`

```wit
emit-event: func(event-id: string, payload-json: string);
```

✅ Diffusé à tous les plugins abonnés à cet event.

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

| Fonction | État |
| --- | --- |
| `access-token` | ✅ Retourne un jeton valide, en le rafraîchissant de façon transparente. **C'est celle à appeler.** |
| `is-authorized` | ✅ |
| `revoke` | ✅ |
| `authorize` | ⛔ Un stub, volontairement. |

::: info Pourquoi `authorize` est un stub
C'est un appel **synchrone** qui n'a nulle part où rendre un code utilisateur à
afficher pendant qu'on attend l'approbation. La moitié interactive tourne donc
côté UI, via l'IPC : Réglages → `oauth_connect` / `oauth_poll` / `oauth_status` /
`oauth_disconnect`.

Le partage des rôles est préservé : le backend demande à votre module son service
**`oauth-config`**, donc aucune spécificité de fournisseur n'entre dans le cœur, et
votre module relit son jeton par `access-token`, comme prévu.
:::

Les jetons sont persistés dans `<user-data>/quicktys/oauth.json` (`0600` sous
Unix), y compris la configuration de fournisseur nécessaire au rafraîchissement.
L'expiration est stockée en secondes Unix absolues — un `Instant` monotone serait
dénué de sens précisément au moment du redémarrage où on le relit.

## Les capacités système

Accordées uniquement aux modules **certifiés**, uniquement si déclarées, et
uniquement si consenties. Les trois sont vérifiées à **chaque dispatch**.

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

| Fonction | État |
| --- | --- |
| `fetch` | ✅ Vrai client HTTP. `http://` sur TCP simple, `https://` sur rustls avec les racines webpki embarquées. |
| `ws-connect` | ✅ `ws://` et `wss://`. |
| `send` | ✅ |
| `close` | ✅ |
| `tcp-connect` | 🟡 Le TCP brut existe mais est moins exercé que le chemin WebSocket. |

Les données entrantes arrivent à `module-guest.on-host-event`, étiquetées avec
l'`event-id` que vous avez passé.

Trois détails d'implémentation qui relèvent du contrat, pas du hasard :

- **`fetch` délimite la réponse par `Content-Length` ou l'encodage chunked**, et ne
  ferme **pas** à moitié le côté requête. Un serveur qui exécute un handler
  asynchrone abandonne sa réponse si le client envoie FIN en premier, et un serveur
  keep-alive ne ferme jamais — lire jusqu'à EOF bloquerait ou renverrait du vide.
- **`ws-connect` connecte de façon synchrone** avant de rendre le handle, et
  enregistre d'abord la moitié écriture. Sinon, un protocole où **le client parle
  en premier** (l'IRC de Twitch) prendrait de vitesse la connexion et ses premiers
  `send` seraient perdus.
- **`close` coupe les deux moitiés.** Ne fermer que l'écriture laisse le lecteur
  garé sur un socket vivant, en train de livrer — c'est ainsi qu'un chat rejoint
  livre chaque message en double.

Pour `wss://`, une seule session TLS chiffre les deux sens et ne peut pas être
clonée. La session est partagée derrière un mutex, et le lecteur fait sa **lecture
socket bloquante hors du verrou**, pour qu'une lecture bloquée n'affame jamais un
envoi.

### `audio`

```wit
list-output-devices: func() -> list<string>;
play-buffer: func(device: option<string>, pcm: list<u8>, sample-rate: u32, channels: u8) -> result<_, string>;
play-file: func(path: string, volume: f32) -> result<_, string>;
speak: func(text: string, voice: string) -> result<_, string>;
list-voices: func() -> list<string>;
```

| Fonction | État |
| --- | --- |
| `play-file` | ✅ Décode wav / mp3 / ogg / flac et joue côté host sur un thread dédié. No-op gracieux en l'absence de périphérique. |
| `speak` / `list-voices` | ✅ TTS de la plateforme — `say`, SAPI, speech-dispatcher. `voice` est un nom de voix sur macOS et Windows, un code de langue sous Linux ; vide = défaut de l'OS. |
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

✅ Les helpers de haut niveau (`type-text`, `key-combo`, `mouse-click`) sont réels,
adossés à `enigo` — backend x11rb sous Linux, donc pas de libxdo. Demande une
vraie session graphique.

`key-combo` prend un accord comme `["ctrl", "shift", "c"]` ; les modificateurs sont
maintenus autour de la dernière touche. Boutons de souris : 0 gauche, 1 milieu,
2 droite.

### `process`

```wit
spawn: func(program: string, args: list<string>) -> result<u64, string>;
kill: func(handle: u64);
run-shell: func(command: string) -> result<u64, string>;
open-path: func(target: string) -> result<_, string>;
```

✅ Les quatre, multiplateformes. `run-shell` utilise `cmd /C` sous Windows et
`sh -c` ailleurs ; `open-path` utilise `start` / `open` / `xdg-open`.

::: danger La capacité la plus tranchante de la liste
Elle lance des programmes sur la machine de l'utilisateur. Ne la déclarez que si
vous en avez vraiment besoin, et méfiez-vous d'une commande construite depuis un
template pouvant inclure du texte de chat.
:::

### `files`

```wit
read: func(path: string) -> result<list<u8>, string>;
write: func(path: string, data: list<u8>) -> result<_, string>;
list-dir: func(path: string) -> result<list<string>, string>;
```

⛔ Déclarée ; conçue pour être limitée au dossier isolé du module. Câblée quand un
driver en aura besoin.

(`list` est le type WIT `list<T>`, d'où `list-dir`.)

### `hid`

```wit
enumerate: func(vendor-id: option<u16>, product-id: option<u16>) -> list<string>;
open: func(path: string, event-id: string) -> result<u64, string>;
write: func(handle: u64, data: list<u8>) -> result<_, string>;
close: func(handle: u64);
```

⛔ Conservée comme infrastructure générique — un driver Stream Deck ou Companion
l'utiliserait — mais **aucune extension livrée ne s'en sert aujourd'hui**. Le
protocole de surface distante qui le faisait a été retiré au profit du widget
Bouton, bien plus simple.

### `timers`

```wit
set-timer: func(ms: u64, repeat: bool, event-id: string) -> u64;
clear-timer: func(handle: u64);
```

⛔ Non implémentée. En attendant, l'app fait tourner un **poller** qui appelle le
service `poll` de chaque module qui en déclare un, toutes les trois secondes. Une
vraie capacité `timers` le remplacera.

C'est pour ça que tout ce qui est périodique appartient au service `poll` d'un
module, et qu'un plugin — jamais interrogé — ne peut rien faire battre de
lui-même.

## Les trois portes

Pour qu'un appel de capacité système aboutisse :

```
accordé = certifié && déclaré && consenti
```

| Porte | Décidée par | Modifiable |
| --- | --- | --- |
| **Certifié** | la cryptographie — la chaîne de signature contre des clés CA épinglées | non |
| **Déclaré** | le manifeste du module | à la publication |
| **Consenti** | l'utilisateur, dans les Réglages | à tout moment, avec effet immédiat |

Une mise à jour qui **élargit** les capacités d'un module voit les nouvelles
refusées jusqu'à re-revue. Un module jamais revu obtient ce qu'il a déclaré, pour
que rien ne casse avant que l'utilisateur n'ouvre l'écran de consentement ; après
revue, seules les capacités accordées sont autorisées.

## Et ensuite

- [Worlds & cycle de vie](/fr/reference/worlds)
- [Écrire un module](/fr/dev/module)
