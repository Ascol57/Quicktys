# Environnement de dev

## Prérequis

Lancez une fois le script de setup de votre OS — il installe tout et génère les
clés de développement. Voir [Installation](/fr/guide/installation).

Ce qui compte pour le travail sur les extensions :

| | |
| --- | --- |
| **Rust 1.82+** | la version qui a introduit la cible `wasm32-wasip2` |
| **`wasm32-wasip2`** | `rustup target add wasm32-wasip2` — sans elle, rien ne se construit |
| **`bash`** | `scripts/build-bundles.sh` est un script shell, y compris sous Windows |

## Deux domaines de build

C'est la chose à intégrer : **les extensions ne sont pas dans le workspace
host.**

```toml
# Cargo.toml (racine)
[workspace]
members = ["crates/quicktys-app", "crates/quicktys-core", ...]
exclude = ["plugins", "modules"]
```

Les extensions sont des paquets autonomes avec leur propre `Cargo.lock`,
construits pour une autre cible et un autre type de crate (`cdylib`). En ajouter
une aux `members` du workspace casse le build.

```bash
# Workspace host — depuis la racine du dépôt
cargo check
cargo test -p quicktys-core
cargo test -p quicktys-host              # les tests d'intégration de l'ABI

# Une extension — depuis SON PROPRE dossier, pas avec -p depuis la racine
cd plugins/time
cargo build --target wasm32-wasip2 --release
```

::: warning Le nom de l'artefact n'est pas celui du paquet
Cargo remplace `-` par `_` dans l'artefact cdylib. Le paquet `counter-client`
produit `counter_client.wasm`. Tout le monde s'y fait prendre une fois.
:::

## Construire les bundles

L'app charge des **bundles**, pas des `.wasm` bruts. Construisez-les tous :

```bash
scripts/build-bundles.sh              # → dist/bundles/
cargo run -p quicktys-app
```

Le script compile chaque extension et assemble :

```
dist/bundles/
├── modules/<id>/{<id>.wasm, lang/, ui/, bundle.json}
├── plugins/<id>/{<id>.wasm, lang/, ui/, bundle.json}
└── widgets/<id>/{bundle.json, lang/, *.html}       ← copiés, pas compilés
```

**Pour ajouter votre extension au build de dev, ajoutez-la à la liste
`EXTENSIONS` ou `WIDGETS` en tête du script.** Le format est
`kind:dossier:id:artefact[:certified]` :

```bash
EXTENSIONS=(
  "module:modules/mondriver:mondriver:mondriver.wasm:certified"
  "plugin:plugins/montruc:montruc:montruc.wasm"
)
WIDGETS=(
  "widget:widgets/maboite:maboite"
)
```

Le drapeau `certified` final est ce qui autorise un module à détenir des capacités
système en développement. C'est un raccourci local — en production, la
certification est [une signature](/fr/dev/packaging#signer).

`time` est délibérément absent de cette liste : son action `delay` est une fixture
d'ABI pure qui n'attend pas réellement, donc la livrer induirait en erreur.

## La boucle de développement

| Vous avez modifié | Faites |
| --- | --- |
| une extension | `scripts/build-bundles.sh`, relancez l'app |
| un crate host | `cargo run -p quicktys-app` |
| le frontend | `npm --prefix frontend run build`, relancez |

Il n'y a pas encore de rechargement à chaud des extensions — le host WASM ne sait
pas décharger un composant en place. `setup/run.sh` fait toute la chaîne pour
vous.

## Lancer l'app

```bash
setup/run.sh            # registre + app, le chemin marketplace complet
setup/run.sh --solo     # app seule, chargeant dist/bundles directement
setup/run.sh --web      # frontend dans un navigateur, IPC bouchonné
cargo run -p quicktys-app
```

Utilisez `--solo` pendant que vous itérez sur une extension : il saute
l'empaquetage et la signature et charge votre bundle directement depuis
`dist/bundles`.

Un lancement graphique demande une vraie session de bureau — GTK et WebKit ne
s'initialisent pas en headless.

## Tests

Les suites de tests du host sont la meilleure spécification de l'ABI, et le moyen
le plus rapide de voir un contrat exercé de bout en bout.

```bash
cargo test -p quicktys-host             # tout ce qui suit
cargo test -p quicktys-host --test plugin_time      # ABI plugin mono-étage
cargo test -p quicktys-host --test two_tier         # plugin ↔ module
cargo test -p quicktys-host --test net_cap          # caps système + gating de certification
cargo test -p quicktys-host --test bundles          # chargement d'un vrai bundle depuis le disque
cargo test -p quicktys-host --test notify_plugin
cargo test -p quicktys-host --test twitch_chat
cargo test -p quicktys-host --test twitch_eventsub
cargo test -p quicktys-host --test pear_auth
cargo test -p quicktys-host --test pear_song_trigger
cargo test -p quicktys-host --test ws_close
```

Ils construisent les extensions de référence à la demande, donc la première
exécution est lente.

### Tester votre propre extension

Le motif utilisé partout : pointer le driver sur un **serveur bouchon** plutôt que
sur la vraie chose. Les modules qui parlent à un service réseau exposent un service
pour ça — `set-base-url` sur `pear`, `set-chat-url` et `set-eventsub-url` sur
`twitch` — donc aucun test ne touche un vrai lecteur ni Twitch.

Donnez la même échappatoire à votre module. Ça coûte une méthode de service et ça
rend tout l'ensemble testable hors-ligne.

## Lire les extensions de référence

Elles sont propriétaires et source-available : lisez-les, apprenez-en comment
elles appellent l'ABI, écrivez les vôtres. Ne copiez pas le code.

| Lisez | Si vous écrivez |
| --- | --- |
| `plugins/notify` | le plugin le plus simple possible |
| `plugins/random` | un plugin avec des résultats typés qu'on lie |
| `plugins/countdown` | un plugin qui utilise `storage`, plus le motif plugin↔widget |
| `plugins/discord` | un plugin qui atteint le réseau via un module générique |
| `modules/sound` | le module le plus simple possible |
| `modules/http` | un driver générique réutilisable |
| `modules/obs` | un driver WebSocket bidirectionnel avec corrélation requête/réponse |
| `modules/twitch` | WebSocket, OAuth, parsing IRC, EventSub |
| `modules/webview` | un module qui possède des données et contribue un onglet entier |
| `widgets/countdown` | un widget runtime avec un schéma de config riche |
| `widgets/alert` | un widget invisible au repos, avec une page d'aperçu |

## Et ensuite

[Écrire un plugin →](/fr/dev/plugin)
