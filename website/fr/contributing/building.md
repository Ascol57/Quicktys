---
description: "Compiler Quicktys depuis les sources : les deux domaines de build, le frontend, les suites de tests d'intégration de l'ABI, les versions épinglées et les conventions."
---

# Compiler depuis les sources

## Démarrage rapide

```bash
git clone https://github.com/Ascol57/Quicktys && cd Quicktys
./setup/setup-linux.sh      # ou setup-macos.sh, ou setup\setup-windows.ps1
./setup/run.sh
```

Détails complets, pièges par OS et contenu des scripts :
[Installation](/fr/guide/installation).

## Les deux domaines de build

```bash
# Workspace host — 16 crates, depuis la racine du dépôt
cargo check                  # la porte d'acceptation
cargo build
cargo test -p quicktys-core
cargo test -p quicktys-core un_nom_de_test

# Extensions — EXCLUES du workspace, construites par crate pour wasm32-wasip2
cd plugins/time
cargo build --target wasm32-wasip2 --release
```

Les extensions sont des paquets autonomes avec leur propre `Cargo.lock`, une autre
cible et un autre type de crate. **N'en ajoutez jamais une aux `members` du
workspace.**

## Frontend

```bash
cd frontend
npm install
npm run build          # → frontend/dist, embarqué par Tauri à la compilation
```

`tauri.conf.json` n'a volontairement **pas de `devUrl`** : le binaire charge le
`frontend/dist` embarqué et tourne de façon autonome. Reconstruisez le frontend
après une modification d'UI.

Pour le rechargement à chaud : `npm --prefix frontend run dev`, puis remettez
`devUrl` et `beforeDevCommand` (ou utilisez `cargo tauri dev`).

## Lancer

```bash
cargo run -p quicktys-app     # autonome
setup/run.sh --solo           # tout construire, puis lancer sans registre
```

Un lancement graphique demande une vraie session de bureau — GTK et WebKit ne
s'initialisent pas en headless.

## Tests

```bash
cargo test                                        # workspace host
cargo test -p quicktys-host                       # les suites d'intégration de l'ABI
```

Les suites de `quicktys-host` sont la spécification exécutable de l'ABI :

| Test | Couvre |
| --- | --- |
| `plugin_time` | l'ABI plugin mono-étage de bout en bout |
| `two_tier` | les appels plugin ↔ module |
| `net_cap` | les capacités système et le gating de certification |
| `bundles` | le chargement d'un vrai bundle depuis le disque via son descripteur |
| `notify_plugin` | le chemin `reactivity.notify` vers l'UI |
| `twitch_chat` | le login IRC, avec un serveur bouchon qui décode les trames masquées |
| `twitch_eventsub` | la création des abonnements EventSub |
| `pear_auth` | refus persistant, relance explicite, rejeu après jeton périmé, client id |
| `pear_song_trigger` | poll → event → trigger → variables réactives |
| `ws_close` | qu'une fermeture de socket arrête vraiment la livraison |

Ils construisent les extensions de référence à la demande : la première exécution
est lente.

`quicktys-ipc` est sans Tauri et testé unitairement sans WebView.

## Versions épinglées

Épinglages exacts (`=`) pour des builds reproductibles :

| | |
| --- | --- |
| `wasmtime` / `wasmtime-wasi` | `=27.0.0` |
| `tauri` / `tauri-build` | `=2.11.5` / `=2.6.3` |
| `wit-bindgen` (invité, par extension) | `=0.36.0` |

Le host génère ses bindings avec le `bindgen!` Component Model de **wasmtime
lui-même** ; il n'utilise pas la crate `wit-bindgen`. `wit-bindgen` est le
générateur d'ABI **invité**, épinglé dans le manifeste de chaque extension — les
0.5x plus récentes ont besoin de la fonctionnalité std instable `wasip2`.

## Conventions

- **La prose de documentation est en français** ; le code, les identifiants et les
  commentaires de code sont en **anglais**. Alignez-vous sur ce qui vous entoure.
- **Consignez les décisions structurantes dans `ARCHITECTURE.md`.** Le document le
  dit explicitement, et c'est ce qui rend la base de code lisible : chaque choix
  non évident a sa justification écrite, y compris ceux qui étaient faux la
  première fois.
- **Cochez et étendez le jalon concerné de `ROADMAP.md`.**
- **Restez dans le périmètre du jalon courant.** Le chemin de validation est
  délibéré : prouver l'ABI plugin sur un plugin simple avant le modèle à deux
  étages, avant le portage de masse.
- **Ne copiez jamais de code CastMate.** Étudier ses concepts et ses APIs est
  licite ; réutiliser son code non, frontend compris.
- Le mainteneur gère lui-même tous les `git commit` et `git push`.

## Contribuer

Les contributions sont acceptées sous **CLA** : le contributeur cède ou licencie
ses droits au propriétaire, qui détient donc le projet et peut le
dual-licencier. Le texte du CLA n'est pas encore rédigé.

## Et ensuite

- [Architecture](/fr/contributing/architecture)
- [Licences](/fr/contributing/license)
