# Outils en ligne de commande

## `scripts/build-bundles.sh`

Compile chaque extension et assemble les dossiers de bundle que le host charge.

```bash
scripts/build-bundles.sh [dossier-de-sortie]      # défaut : dist/bundles
```

Produit :

```
dist/bundles/
├── modules/<id>/{<id>.wasm, lang/, ui/, bundle.json}
├── plugins/<id>/{<id>.wasm, lang/, ui/, bundle.json}
└── widgets/<id>/{bundle.json, lang/, *.html}
```

**Ce qui est livré, ce sont les listes `EXTENSIONS` et `WIDGETS` en tête du
script.** Ajoutez-y la vôtre :

```bash
EXTENSIONS=(
  # kind:dossier:id:artefact[:certified]
  "module:modules/mondriver:mondriver:mondriver.wasm:certified"
  "plugin:plugins/montruc:montruc:montruc.wasm"
)
WIDGETS=(
  # kind:dossier:id
  "widget:widgets/maboite:maboite"
)
```

Le drapeau `certified` final est le substitut de développement à une signature —
c'est lui qui autorise un module à détenir des capacités système en local.

::: warning Le nom de l'artefact n'est pas celui du paquet
Cargo remplace `-` par `_` : le paquet `counter-client` produit
`counter_client.wasm`.
:::

## `setup/gen-keys.sh`

Génère les clés de développement. `dist/` est gitignoré : un clone frais n'en a
aucune.

```bash
setup/gen-keys.sh            # crée ce qui manque, n'écrase jamais
setup/gen-keys.sh --show     # inventaire seulement
setup/gen-keys.sh --force    # tout régénérer (invalide les signatures existantes)
```

| Fichier | Rôle |
| --- | --- |
| `dist/ca.key` / `.pub` | autorité de certification de dev — signe les certificats d'éditeur, ce qui rend un module certifié. Épinglée via `QUICKTYS_CA`. |
| `dist/store.key` / `.pub` | clé de store — signe les droits des extensions payantes. Épinglée via `QUICKTYS_STORE`. |
| `dist/secrets/<kind>-<id>.ck` | clé de contenu d'un paquet chiffré, créée à l'empaquetage |
| `dist/pear.license` | une licence signée par la clé de store, à coller dans l'app |

Paires ML-DSA (post-quantiques), stockées en hexadécimal. Rien n'est un secret de
production ; perdre `dist/` ne coûte rien. Ne le commitez jamais.

## `setup/run.sh`

Construit tout et lance.

```bash
setup/run.sh                  # serveur de registre + app (défaut)
setup/run.sh --solo           # l'app seule, chargeant dist/bundles
setup/run.sh --web            # le frontend seul, dans un navigateur (Vite, IPC bouchonné)
setup/run.sh --skip-build     # relancer sans reconstruire
setup/run.sh --port 9000      # port du registre
```

Windows : `powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1`.

Le mode par défaut délègue à `dev.sh`, qui empaquette et **signe** le catalogue
avant de servir le registre — le chemin par défaut exerce donc la vraie chaîne
marketplace.

## `quicktys-pack`

Transforme des dossiers de bundle en paquets `.qtpkg` distribuables, les signe, et
émet des licences.

```bash
quicktys-pack <dossier_bundles> [--out <dir>] [--sign] [--ca <clé>] [--paid <kind/id[:sku]>]
quicktys-pack <dossier_pkgs> --sign-only [--ca <clé>]
quicktys-pack --issue-license <kind/id[:sku]> [--store <clé>]
```

| Option | |
| --- | --- |
| `--out <dir>` | où écrire les paquets |
| `--sign` | signer chaque paquet avec la clé CA |
| `--ca <clé>` | la clé CA de signature (défaut `dist/ca.key`) |
| `--sign-only` | signer sur place des paquets déjà construits |
| `--paid <kind/id[:sku]>` | marquer un paquet comme payant — l'installeur le refusera sans licence couvrante |
| `--encrypt <kind/id[:sku]>` | chiffrer le payload `.wasm` ; écrit la clé de contenu dans `dist/secrets/` |
| `--issue-license <kind/id[:sku]>` | frapper un droit signé par la clé de store et l'afficher |
| `--store <clé>` | la clé de store pour signer les licences |

```bash
scripts/build-bundles.sh
quicktys-pack dist/bundles --out dist/pkgs --sign
quicktys-pack --issue-license module/pear --store dist/store.key > dist/pear.license
```

## `quicktys-regserve`

Sert un dossier de paquets comme un registre.

```bash
quicktys-regserve <dossier_paquets> [--port N] [--addr IP] [--out DIR] [--sign] [--static]
```

| Option | Défaut | |
| --- | --- | --- |
| `--port` | 8000 | port d'écoute |
| `--addr` | 127.0.0.1 | adresse d'écoute |
| `--out` | un dossier temporaire | où écrire l'arborescence de registre générée |
| `--sign` | désactivé | signer l'index généré |
| `--static` | désactivé | générer l'arborescence et sortir, sans servir |

Pointez les clients dessus :

```bash
QUICKTYS_REGISTRY=http://127.0.0.1:8000 cargo run -p quicktys-app
```

Le registre est une arborescence statique — `index.json` (+ `.sig`), `crl.json`, et
des blobs adressés par contenu — donc la sortie de `--static` peut être déposée sur
n'importe quel hébergement de fichiers.

## `quicktys-migrate`

Migration unidirectionnelle best-effort depuis un dossier de données CastMate.

```bash
quicktys-migrate <dossier-données-castmate> [--out <dir>]
```

Il lit **vos données** — des fichiers YAML, pas du code — et écrit des ressources
Quicktys valides par construction, plus un **rapport Markdown**.

Ce qu'il fait :

- importe profils, automatisations et réglages ;
- transpose le modèle d'automatisation (instant / time / stack / flow → l'arbre de
  nœuds) ;
- convertit un template `{{ chemin.simple }}` **seul** en `@chemin.simple` ;
- extrait défensivement, en tolérant plusieurs orthographes de champ.

Ce qu'il ne fait **délibérément pas** :

- **aucune traduction automatique JavaScript → Rhai.** Les conditions et filtres
  CastMate sont du JS arbitraire ; les convertir serait peu fiable, donc ils vous
  sont laissés à réécrire et sont listés dans le rapport.
- tout ce qui est ambigu — actions non reconnues, expressions `{{ }}` non
  triviales — est **rapporté, jamais deviné**.

Le rapport est le point d'entrée : lisez-le en premier.

## Variables d'environnement

| Variable | |
| --- | --- |
| `QUICKTYS_REGISTRY` | URL de base du registre |
| `QUICKTYS_CA` | clé publique CA épinglée |
| `QUICKTYS_STORE` | clé publique de store épinglée |
| `QUICKTYS_NO_DEV_BUNDLES` | ne pas scanner `dist/bundles`, pour ne charger que les extensions installées — c'est ce que met `dev.sh` afin d'exercer le vrai chemin d'installation |
| `HOME` | le dossier de données en est dérivé ; **doit être défini sous Windows**, ce que fait `run-windows.ps1` |

## Cargo

```bash
cargo check                              # tout le workspace host
cargo build
cargo test -p quicktys-core
cargo test -p quicktys-core un_test
cargo test -p quicktys-host              # les suites d'intégration de l'ABI

# Une extension — depuis son propre dossier
cd plugins/time && cargo build --target wasm32-wasip2 --release
```

Les extensions sont **exclues** du workspace host. N'en ajoutez jamais une aux
`members`.

## Et ensuite

- [Empaqueter & publier](/fr/dev/packaging)
- [Fichiers & dossier de données](/fr/reference/files)
