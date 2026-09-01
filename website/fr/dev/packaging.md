# Empaqueter & publier

Comment faire passer une extension d'un dossier sur votre disque à quelque chose
que quelqu'un d'autre peut installer.

## Le format `.qtpkg`

Un bundle est un dossier. Pour le transport, il est sérialisé en un unique
artefact **adressé par contenu** :

```
<id>-<version>.qtpkg          tar déterministe + zstd du dossier de bundle
├── manifest.json             le descripteur étendu
├── <id>.wasm                 absent pour un widget ou un paquet de contenu
├── lang/…                    les locales
├── index.html                widgets runtime uniquement
└── signature.json            obligatoire pour les modules, optionnelle sinon
```

L'archive est **déterministe** — entrées triées, permissions et dates fixes — donc
le même contenu produit les mêmes octets, et donc le même `sha256`. Cette
empreinte est l'identité du paquet, sa clé de cache et sa clé de CDN.

### `manifest.json`

Un sur-ensemble du `bundle.json` que le host lit au chargement :

```jsonc
{
  "id": "obs",
  "kind": "module",                 // plugin | module | widget | content
  "version": "1.4.0",               // semver
  "wasm": "obs.wasm",               // absent pour widget / content
  "requires": [                     // plugins : les modules dont ils ont besoin
    { "module": "http", "version": "^1" }
  ],
  "capabilities": ["net"],          // modules : capacités système demandées
  "min_host": "0.3.0",
  "publisher": "acme",
  "license": { "model": "free" },   // free | paid | subscription
  "hashes": { "obs.wasm": "sha256:…" }
}
```

Les empreintes par fichier sont de la défense en profondeur : la signature engage
le manifeste, et le manifeste engage chaque fichier.

::: info L'id n'est pas l'identité
Deux éditeurs peuvent publier `id: "obs"`. Ce qui les distingue est le champ
`publisher` et la chaîne de signature, pas l'id. L'installeur refuse d'écraser une
extension installée provenant d'un **éditeur différent** sans confirmation
explicite.
:::

## Empaqueter

```bash
quicktys-pack <dossier_bundles> [--out <dir>] [--sign] [--ca <fichier_clé>]
quicktys-pack <dossier_bundles> --paid <kind/id[:sku]>
quicktys-pack <dossier_bundles> --encrypt <kind/id[:sku]>
quicktys-pack <dossier_pkgs> --sign-only [--ca <fichier_clé>]
quicktys-pack --issue-license <kind/id[:sku]> [--store <fichier_clé>]
```

Le cycle de développement habituel :

```bash
scripts/build-bundles.sh                       # dossiers → dist/bundles/
quicktys-pack dist/bundles --out dist/pkgs --sign
quicktys-regserve dist/pkgs --port 8000
QUICKTYS_REGISTRY=http://127.0.0.1:8000 cargo run -p quicktys-app
```

`setup/run.sh` fait exactement ça par défaut — c'est pourquoi le chemin de
développement par défaut exerce la vraie chaîne marketplace plutôt que de la
contourner.

## Signer

La provenance est une chaîne : **autorité de certification → certificat d'éditeur
→ release**, chaque maillon signé. La paire est hybride post-quantique (Ed25519 +
ML-DSA).

```jsonc
// signature.json
{ "alg": "ed25519", "content_hash": "…", "sig": "…", "publisher_cert": { /* …, ca_sig */ } }
```

À l'installation, toute la chaîne est vérifiée contre des **clés CA épinglées**
plus une liste de révocation, ce qui produit l'un de trois verdicts :

| Verdict | Conséquence |
| --- | --- |
| **Certifié** | chaîne valide, empreinte de manifeste signée, capacités déclarées dans le plafond du certificat, non expiré, non révoqué |
| **Non signé** | s'installe, mais n'est **pas** certifié — les capacités système d'un module sont refusées au chargement |
| **Rejeté** | une signature invalide **refuse** l'installation |

`certified` est donc *calculé par la cryptographie*, jamais lu comme un booléen
dans un fichier modifiable.

::: info Pourquoi la signature couvre le manifeste, pas l'archive
`signature.json` vit *dans* l'archive : signer l'empreinte de l'archive complète
serait circulaire. La signature engage l'**empreinte du manifeste**, et le
manifeste couvre le payload par ses `hashes` par fichier.
:::

### Les clés de développement

`setup/gen-keys.sh` génère une CA locale (`dist/ca.key` / `.pub`) et une clé de
store (`dist/store.key` / `.pub`), de façon idempotente. Épinglez-les avec
`QUICKTYS_CA` et `QUICKTYS_STORE`. Rien là-dedans n'est un secret de production —
perdre `dist/` ne coûte rien.

Avec un **trust store vide** (aucune opération de CA pour l'instant), les bundles
de développement sont de confiance par localité, ce qui fait marcher `--solo`.

## Certification

Seuls les modules certifiés peuvent détenir des capacités système. Être certifié,
c'est :

1. une revue du module,
2. un certificat d'éditeur émis par l'autorité Quicktys,
3. un **plafond de capacités** sur ce certificat — vous ne pourrez pas signer plus
   tard une release qui en demande davantage,
4. la signature de chaque release avec votre clé d'éditeur.

Les plugins et les widgets n'ont besoin de rien de tout ça. Leur bac à sable est
la garantie, et un plugin ne peut de toute façon pas demander de capacité système.

::: warning Le programme de certification n'est pas encore ouvert
L'infrastructure de signature existe et est exercée de bout en bout par les clés
de développement, mais l'autorité, le processus de revue et les opérations de
révocation sont encore en cours de mise en place. Voir la
[roadmap](/fr/contributing/roadmap).
:::

## Le registre

Un registre est **statique** — pas de serveur applicatif, rien de dynamique :

```
registry/
├── index.json            catalogue signé, une entrée par version publiée
├── index.sig
├── crl.json (+ .sig)     liste de révocation
└── blobs/
    └── sha256/<hash>.qtpkg
```

Servez-le depuis un bucket, un CDN, ou `quicktys-regserve`. Trois raisons à cette
forme : un serveur minimal est une surface d'attaque minimale ; c'est trivialement
auto-hébergeable ; et la confiance repose sur les **signatures des paquets**, pas
sur le serveur.

Un index compromis peut au pire cacher des paquets ou mentir sur des métadonnées.
Il ne peut pas fabriquer un module valablement certifié.

```bash
quicktys-regserve <dossier_paquets> --port 8000 --addr 127.0.0.1 --sign
```

Pointez les clients dessus avec `QUICKTYS_REGISTRY`.

## Versions et dépendances

Une contrainte structurelle façonne tout le reste : **une seule version d'un id de
module donné peut être chargée à la fois.** Le loader dédoublonne par id, donc deux
plugins qui exigent des versions incompatibles d'`obs` ne peuvent pas coexister.

Le résolveur produit donc **une** version résolue par id, satisfaisant **toutes**
les contraintes, en parcourant les dépendances en profondeur, dépendances avant
dépendants, avec détection de cycles.

Conséquence pratique pour les auteurs de modules : **la discipline semver n'est
pas optionnelle.** Un changement cassant des arguments d'un service est une version
majeure, et en livrer un discrètement cassera des plugins qui ne sont pas les
vôtres.

## Les extensions payantes

Un paquet déclare son modèle de licence dans le manifeste. Les paquets `paid` et
`subscription` ne s'installent pas sans une **licence** qui les couvre.

Une licence est une clé `QTLIC-…` : un droit **signé par le store** et vérifié
**hors-ligne** contre des clés de store épinglées. Une fois que l'utilisateur l'a,
aucun réseau n'est nécessaire.

```bash
quicktys-pack --issue-license module/pear:pro --store dist/store.key
```

### Payloads chiffrés

`--encrypt` produit `<wasm>.enc` — le composant chiffré en XChaCha20-Poly1305 — et
le déclare dans `manifest.protection`. La clé de contenu est livrée **dans la
licence**, donc il n'y a ni compte ni serveur dans le chemin.

Le déchiffrement se fait **au chargement, en mémoire**. Le clair n'est jamais
réécrit sur le disque, ni au boot ni au chargement à chaud.

Signature et chiffrement restent **deux couches indépendantes**. Un paquet premium
est signé en clair — pour rester catalogable et sa certification calculable — **et**
chiffré.

### Watermarking

Un paquet peut être marqué `watermark: true`. À l'installation, un id opaque par
acheteur est tissé invisiblement dans les **assets texte** du paquet à l'aide de
caractères de largeur nulle, répété pour survivre à une troncature.

Il est appliqué **après** la vérification de provenance (une marque ne peut pas
faire partie des octets signés) et **jamais** sur les `*.json` (ça casserait la
signature et le manifeste). Si une copie fuit, l'id peut être extrait et rattaché
à un acheteur côté serveur.

::: warning C'est de la dissuasion, pas de la protection
Le design le dit franchement : un watermark client se retire, et un DRM client
parfait n'existe pas. L'objectif est de protéger la majorité honnête et de pouvoir
tracer et révoquer — pas d'être inviolable. Le déchiffreur est ouvert par
conception ; le secret est dans les clés.
:::

## Sideload

Un `.qtpkg` peut être installé depuis le disque. Il passe par une vérification
identique : un paquet non signé s'installe comme non certifié, et un module non
certifié qui déclare des capacités est refusé au chargement. Le sideload n'achète
aucun privilège.

## Checklist avant publication

- [ ] Le manifeste émet des **clés i18n**, pas des chaînes en anglais.
- [ ] `lang/en.json` couvre toutes les clés, et est la locale de référence.
- [ ] La version est en semver, et un changement cassant de service a incrémenté
      le majeur.
- [ ] Un module ne déclare que les capacités qu'il utilise réellement.
- [ ] Un driver réseau expose un service à la `set-base-url` pour être testable
      hors-ligne.
- [ ] Les booléens de widget s'appellent `showX`, jamais `hideX`.
- [ ] Les libellés d'énumération vivent sous `options.<clé>.<valeur>`.
- [ ] Votre licence est déclarée dans `Cargo.toml` et dans le manifeste du paquet.

## Et ensuite

- [Outils en ligne de commande](/fr/reference/cli) — toutes les options.
- [Marketplace](/fr/guide/marketplace) — le côté installation.
