# Setup — lancer Quicktys de zéro

Scripts d'installation et de lancement, un par OS. Tout part d'un `git clone`
nu : aucune dépendance, aucune clé, rien de construit.

## macOS — de zéro

```bash
git clone <url> && cd FastMate
./setup/setup-macos.sh     # prérequis + dépendances + clés de dev
./setup/run.sh             # construit tout, puis lance serveur + client
```

C'est tout. Le premier `run.sh` est long (tout le workspace Rust, plus un
composant WASM par extension) ; les suivants réutilisent `target/`.

Si le script s'arrête en demandant d'installer les **Xcode Command Line
Tools**, c'est normal : leur installeur est graphique. Laisse-le finir, puis
relance `./setup/setup-macos.sh`.

## Les autres OS

| OS | Prérequis | Lancer |
| --- | --- | --- |
| **macOS** | `./setup/setup-macos.sh` | `./setup/run.sh` |
| **Linux** | `./setup/setup-linux.sh` | `./setup/run.sh` |
| **Windows** | `powershell -ExecutionPolicy Bypass -File setup\setup-windows.ps1` | `powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1` |

`./setup/setup.sh` détecte l'OS et appelle le bon script (macOS/Linux).

## Ce que font les scripts

### `setup-<os>` — les prérequis, une fois

1. **Outils de compilation natifs** — Xcode Command Line Tools (macOS), GTK 3 +
   WebKit2GTK 4.1 + build-essential (Linux), MSVC Build Tools + WebView2
   (Windows). Tauri rend l'UI dans la webview **du système** : c'est elle qu'il
   faut, pas Chromium.
2. **Rust** (rustup) + la cible **`wasm32-wasip2`**, sans laquelle aucune
   extension ne se construit. Rust **1.82+** est requis : c'est la version qui a
   introduit cette cible.
3. **Node 18+** — le frontend est une app Vue 3 + Vite.
4. **`npm install`** dans `frontend/`.
5. **Les clés de développement** (voir plus bas).

### `run.sh` — construire et lancer

```bash
./setup/run.sh                # serveur (registry) + client (app) — par défaut
./setup/run.sh --solo         # l'app seule, sans marketplace
./setup/run.sh --web          # le frontend seul, dans le navigateur (Vite)
./setup/run.sh --skip-build   # relancer sans reconstruire
./setup/run.sh --port 9000    # changer le port du registry
```

- **client** = l'app de bureau Tauri (`quicktys-app`). Elle **embarque** le
  frontend construit (`frontend/dist`) — il n'y a pas de serveur web à lancer
  pour l'UI. Elle sert par contre elle-même les overlays pour les *browser
  sources* OBS, sur son propre port local.
- **serveur** = `quicktys-regserve`, le registry/marketplace depuis lequel l'app
  installe les extensions. Optionnel : sans lui (`--solo`), l'app charge les
  bundles construits localement dans `dist/bundles`.

Le mode par défaut délègue à [`dev.sh`](../dev.sh), qui empaquette et **signe**
le catalogue avant de servir le registry — c'est le chemin qui teste la vraie
chaîne marketplace.

> `--web` sert à travailler l'UI vite (HMR) : l'IPC est bouchonné, il n'y a ni
> host, ni extensions, ni état réel.

## Les clés

`dist/` est **gitignoré** : un clone frais n'a donc **aucune clé**, et il faut
les générer localement. C'est ce que fait `./setup/gen-keys.sh` (appelé par les
scripts de setup, et par `run.sh` si elles manquent) :

| Fichier | Rôle |
| --- | --- |
| `dist/ca.key` / `ca.pub` | **Autorité de certification** de dev. Signe les certificats d'éditeur, ce qui rend un module *certifié* — donc autorisé à détenir les capacités système (`net`, `input`, `audio`…). L'app épingle la moitié publique via `QUICKTYS_CA`. |
| `dist/store.key` / `store.pub` | **Clé du store**. Signe les licences (entitlements) des extensions payantes. Épinglée via `QUICKTYS_STORE`. |
| `dist/secrets/<kind>-<id>.ck` | Clé de contenu d'un paquet chiffré (premium, ex. `module/pear`). Créée à l'empaquetage. |
| `dist/pear.license` | Une licence signée par la clé du store, à coller dans l'app pour débloquer `pear`. |

```bash
./setup/gen-keys.sh           # crée ce qui manque, n'écrase jamais
./setup/gen-keys.sh --show    # juste l'inventaire
./setup/gen-keys.sh --force   # tout régénérer (invalide les signatures existantes)
```

Ce sont des paires **ML-DSA** (post-quantique), stockées en hexadécimal. Rien
là-dedans n'est un secret de production : perdre `dist/` est sans conséquence,
on le régénère. Et rien ne doit être commité.

## Pièges connus

- **Le dossier de données est en dur** :
  [`main.rs:329`](../crates/quicktys-app/src/main.rs#L329) construit
  `$HOME/.local/share/quicktys`. Sur macOS ça marche mais ce n'est pas la
  convention (`~/Library/Application Support`), et sur **Windows `HOME` n'est
  pas défini** : l'app retomberait sur `%TEMP%` et perdrait profils et réglages
  au redémarrage. `run-windows.ps1` fixe donc `HOME=%USERPROFILE%`.
- **Linux : il faut une vraie session graphique.** L'app ne démarrera pas en
  SSH sans affichage (GTK/WebKit).
- **Windows : `bash` est requis** pour `scripts/build-bundles.sh` — celui de Git
  for Windows suffit, le script de setup l'installe.
- **Après avoir modifié le frontend**, il faut le reconstruire : `tauri.conf.json`
  n'a volontairement pas de `devUrl`, le binaire charge `frontend/dist` embarqué.
  `run.sh` s'en charge sauf avec `--skip-build`.
- **Après avoir modifié une extension**, relancer `scripts/build-bundles.sh`
  (ou `run.sh` sans `--skip-bundles`).
