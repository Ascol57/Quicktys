---
description: "Installer Quicktys sous Linux, macOS ou Windows : prérequis, cible wasm32-wasip2, clés de développement, et comment construire et lancer depuis un clone nu."
---

# Installation

Quicktys n'est pas encore distribué sous forme d'installeur signé : aujourd'hui,
on l'installe en le construisant. C'est moins pénible que ça en a l'air — un
script par OS installe tous les prérequis et génère les clés de développement, un
second construit et lance le tout.

::: danger Le dépôt n'est pas encore public
Les textes de licence sont chez un avocat avant l'ouverture du code : **le
`git clone` ci-dessous ne fonctionnera pas encore pour vous**, ni les liens vers
les fichiers source ailleurs sur ce site. Cette page est publiée dès maintenant
pour que la documentation soit prête le jour de l'ouverture ; les étapes
elles-mêmes sont exactes et testées.
:::

::: warning Pré-1.0
Il n'y a pas encore de binaires de release. Prévoyez d'avoir une chaîne Rust et
Node sur votre machine. Voir la [roadmap](/fr/contributing/roadmap) pour l'état de
l'empaquetage.
:::

## Depuis un clone nu

```bash
git clone https://github.com/Ascol57/Quicktys && cd Quicktys
./setup/setup-linux.sh      # ou setup-macos.sh
./setup/run.sh
```

Sous Windows :

```powershell
powershell -ExecutionPolicy Bypass -File setup\setup-windows.ps1
powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1
```

`./setup/setup.sh` détecte l'OS et appelle le bon script sur macOS et Linux.

Le premier `run.sh` est long — il construit tout le workspace Rust, plus un
composant WebAssembly par extension. Les suivants réutilisent `target/`.

## Ce qu'installe le script de setup

| | |
| --- | --- |
| **Outils natifs** | Xcode Command Line Tools (macOS), `build-essential` + GTK 3 + WebKit2GTK 4.1 (Linux), MSVC Build Tools + WebView2 (Windows). Tauri rend l'UI dans la **webview du système** : c'est elle qu'il faut, pas Chromium. |
| **Rust** | via rustup, plus la cible **`wasm32-wasip2`**. Rust **1.82+** est requis : c'est la version qui a introduit cette cible, et sans elle aucune extension ne se construit. |
| **Node 18+** | le frontend est une app Vue 3 + Vite. |
| **Paquets npm** | `npm install` dans `frontend/`. |
| **Clés de dev** | voir plus bas. |

::: details macOS : le script s'arrête en demandant les Xcode CLT
C'est normal — leur installeur est graphique. Laissez-le finir, puis relancez
`./setup/setup-macos.sh`.
:::

## Lancer

```bash
./setup/run.sh                # serveur (registry) + app — par défaut
./setup/run.sh --solo         # l'app seule, sans marketplace
./setup/run.sh --web          # le frontend seul, dans le navigateur (Vite)
./setup/run.sh --skip-build   # relancer sans reconstruire
./setup/run.sh --port 9000    # changer le port du registry
```

Deux processus sont en jeu :

- **Le client** est l'app de bureau Tauri (`quicktys-app`). Elle **embarque** le
  frontend construit (`frontend/dist`) — il n'y a pas de serveur web à lancer pour
  l'UI. Elle sert en revanche elle-même les overlays, pour que les sources
  navigateur d'OBS puissent les atteindre.
- **Le serveur** est `quicktys-regserve`, le registre depuis lequel l'app installe
  les extensions. Optionnel : sans lui (`--solo`), l'app charge les bundles
  construits localement dans `dist/bundles`.

Le mode par défaut délègue à `dev.sh`, qui empaquette et **signe** le catalogue
avant de servir le registry — c'est le chemin qui teste la vraie chaîne
marketplace de bout en bout.

::: tip `--web` sert uniquement à travailler l'UI
Il donne le rechargement à chaud de Vite, mais l'IPC est bouchonné : ni host, ni
extensions, ni état réel.
:::

## Les clés de développement

`dist/` est gitignoré : un clone frais n'a donc **aucune clé**.
`setup/gen-keys.sh` les crée (les scripts de setup l'appellent, et `run.sh` aussi
si elles manquent) :

| Fichier | Rôle |
| --- | --- |
| `dist/ca.key` / `ca.pub` | L'**autorité de certification** de dev. Elle signe les certificats d'éditeur, ce qui rend un module *certifié* — et donc autorisé à détenir les capacités système (`net`, `input`, `audio`…). L'app épingle la moitié publique via `QUICKTYS_CA`. |
| `dist/store.key` / `store.pub` | La **clé du store**. Elle signe les licences (entitlements) des extensions payantes. Épinglée via `QUICKTYS_STORE`. |
| `dist/secrets/<kind>-<id>.ck` | La clé de contenu d'un paquet chiffré, créée à l'empaquetage. |
| `dist/pear.license` | Une licence signée par la clé du store, à coller dans l'app pour débloquer `pear`. |

```bash
./setup/gen-keys.sh           # crée ce qui manque, n'écrase jamais
./setup/gen-keys.sh --show    # juste l'inventaire
./setup/gen-keys.sh --force   # tout régénérer (invalide les signatures existantes)
```

Ce sont des paires **ML-DSA** (post-quantiques), stockées en hexadécimal. Rien
là-dedans n'est un secret de production : perdre `dist/` est sans conséquence, on
le régénère. Et rien ne doit être commité.

## Reconstruire après une modification

| Vous avez modifié… | Lancez |
| --- | --- |
| le frontend (`frontend/`) | `npm --prefix frontend run build` — `tauri.conf.json` n'a volontairement pas de `devUrl`, le binaire charge `frontend/dist` embarqué. `run.sh` s'en charge sauf avec `--skip-build`. |
| une extension (`plugins/`, `modules/`, `widgets/`) | `scripts/build-bundles.sh`, puis relancer l'app. |
| un crate host (`crates/`) | `cargo run -p quicktys-app`. |

## Pièges connus

- **Le dossier de données est en dur.** `crates/quicktys-app/src/main.rs`
  construit `$HOME/.local/share/quicktys`. Ça marche sur macOS mais ce n'est pas
  la convention (`~/Library/Application Support`), et sous **Windows `HOME` n'est
  pas défini** — l'app retomberait sur `%TEMP%` et perdrait profils et réglages au
  redémarrage. `run-windows.ps1` fixe donc `HOME=%USERPROFILE%`.
- **Linux : il faut une vraie session graphique.** L'app ne démarrera pas en SSH
  sans affichage (GTK/WebKit).
- **Windows : `bash` est requis** pour `scripts/build-bundles.sh`. Celui de Git
  for Windows suffit, le script de setup l'installe.

## Et ensuite

[Première automatisation →](/fr/guide/getting-started)
