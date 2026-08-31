# Marketplace & distribution des extensions

> Doc de conception (design de référence). Prose en français, identifiants/formats
> en anglais. Complète **ARCHITECTURE.md §11 (Marketplace, conception cible)** et
> **§11.1 (Économie & licence)** — celles-ci restent la source des *décisions* de
> haut niveau ; ce fichier détaille le *système* (formats, protocole, signature,
> résolution, installation à chaud, IPC). Cible le jalon **ROADMAP M6**.

## 0. But & périmètre

Aujourd'hui une extension n'existe que si elle a été **compilée localement** par
`scripts/build-bundles.sh` dans `dist/bundles/…`. Le boot sait déjà charger un
second emplacement — `<data>/installed/` — mais **rien ne le remplit** : il n'y a
ni téléchargement, ni vérification de signature, ni résolution de dépendances à
l'exécution. Ce document conçoit ce chaînon manquant : **découvrir → télécharger
→ vérifier → installer → charger à chaud**, pour les **quatre** types d'objets de
§11 (plugins, modules, widgets, contenus).

Principe directeur : **le marketplace n'invente aucun nouveau mode de chargement.**
Il ne fait que **matérialiser des bundles** dans `installed/<kind>/<id>/` — la
forme exacte que [`load_bundle_dir`](../crates/quicktys-app/src/main.rs) scanne
déjà. Toute la sécurité runtime (sandbox WASM, gating de certification) reste où
elle est ; le marketplace ajoute une **frontière de confiance amont** (signature,
provenance, consentement) avant que le `.wasm` ne touche le host.

## 1. État actuel (ce sur quoi on construit)

| Brique existante | Où | Ce qui manque pour le marketplace |
| --- | --- | --- |
| Scan de bundles 2 racines, modules avant plugins, dédup par id | `main.rs::load_extensions` / `load_bundle_dir` / `bundle_roots` | rien à changer — cible d'installation |
| Descripteur `bundle.json` = `{id, kind, wasm, certified}` | `main.rs::BundleManifest` | pas de `version`, `requires`, `hash`, `signature`, `publisher`, `license` |
| Gating certification (refus si module non certifié déclare une cap) | `WasmHost::load_module` (`lib.rs`) — `if !certified && !caps.is_empty()` | `certified` vient d'un **bool en clair** dans le JSON, **aucune signature vérifiée** |
| Check par-capability `certified && declared` | `ExtensionRegistry::may_grant` | **jamais appelé** par le host (un module certifié obtient de fait toutes les caps) |
| Dépendances plugin→module `requires: [{module, version}]` | `extensions.rs::ModuleRequirement`, `missing_dependencies`, `version_satisfies` | non appliqué au **load** ni à l'**install** ; semver minimal (`*`, exact, `^major`) |
| Racine de données `<data>/installed/` créée par `ensure_layout` | `persist.rs::DataDir` (`SUBDIRS`) | pas de `registry-cache/`, pas de lockfile, pas de store de consentement |
| IPC `list_plugins/modules/widgets`, `call_module` | `quicktys-ipc::backend.rs` | **aucune** commande install/download/uninstall/update ; `ModuleInfo` n'expose pas `certified` |

Écarts explicitement à combler (décisions actées plus bas) : **(a)** signature de
provenance sur les paquets, **(b)** install runtime dans `installed/`, **(c)** câbler
`may_grant` dans le dispatch réel des caps, **(d)** semver réel, **(e)** entitlements.

## 2. Modèle de confiance & signature (décision)

Le sandbox WASM protège l'utilisateur **à l'exécution**. La signature protège la
**chaîne d'approvisionnement** : garantir *qui* a publié un artefact et qu'il n'a
pas été altéré. On distingue nettement les deux frontières :

```
┌─ frontière amont (marketplace) ─┐        ┌─ frontière runtime (host) ─┐
provenance + intégrité + droit d'usage  →  sandbox + gating certification
   (signatures, entitlements)               (Store wasmtime, may_grant)
```

**Signatures hybrides post-quantiques.** Toutes les signatures de cette chaîne sont
**hybrides classique + PQ** (Ed25519 + ML-DSA ; racine CA en Ed25519 + SLH-DSA),
avec `suite_id` versionné pour la crypto-agilité — décision et justification en
[drm.md §2.1](drm.md). Les racines vivant des décennies, on les rend PQ dès le
départ pour éviter une migration future.

**Chaîne à deux niveaux.** On n'auto-signe pas au niveau CA chaque
release (le CA resterait en ligne en permanence). À la place :

1. **Racine de certification (CA).** Une clé racine hors-ligne. Sa **clé publique
   est épinglée dans le host** (embarquée à la compilation, dans un *keyring* qui
   supporte plusieurs clés → **rotation** sans casser les anciens paquets).
2. **Certificat d'éditeur.** Le CA signe un `publisher-cert` :
   `{ publisher_id, publisher_pubkey, caps_ceiling, not_after }`. `caps_ceiling`
   est le **plafond** des capacités système que cet éditeur a le droit de demander
   (p. ex. un éditeur audio n'a pas `input`). Un plugin (jamais de cap système)
   n'a pas besoin d'être certifié → **pas de certificat requis**.
3. **Signature de release.** L'éditeur signe chaque paquet avec `publisher_pubkey` :
   `{ content_hash, id, version, kind, caps, not_after }`.

**Vérification au host** (avant de matérialiser un bundle module) :
`content_hash` recalculé == signé → `publisher_pubkey` valide la release →
`publisher-cert` valide par le CA épinglé → `caps ⊆ caps_ceiling` → non révoqué.
La sortie de cette vérification est le **`certified: bool`** que
`WasmHost::load_module` consomme déjà — on ne change pas le runtime, on **calcule
honnêtement** ce booléen au lieu de le lire en clair.

**Ce qui est signé, par type :**

| Type | Signature | Pourquoi |
| --- | --- | --- |
| **Module** | **obligatoire** (chaîne CA→éditeur→release) | seul type à toucher les caps système |
| **Plugin** | **facultative** (intégrité éditeur, pas CA) | sandboxé ; la signature ne sert qu'à la réputation/anti-tamper |
| **Widget** | **facultative** | frontend isolé ; idem |
| **Contenu** | aucune (validé par schéma) | pas de code exécutable |

**Révocation.** Le registre publie une **CRL** signée (`revoked: [content_hash | publisher_id]`),
téléchargée avec l'index et **mise en cache**. Un module révoqué déjà installé est
refusé au prochain boot (le host revérifie au chargement, pas seulement à l'install).

**Sideload de module non signé.** Interdit par défaut. Un mode développeur explicite
(réglage + avertissement) autorise un module local non signé → il est chargé
**non certifié**, donc le gating `!certified && !caps.is_empty()` le refuse s'il
demande une cap. C'est cohérent avec le comportement actuel.

## 3. Format de paquet distribuable (`.qtpkg`) (décision)

Un bundle est aujourd'hui un **dossier**. Pour le transport on le sérialise en un
seul artefact **content-addressé** :

```
<id>-<version>.qtpkg      # tar déterministe + zstd du dossier bundle
  ├── manifest.json       # descripteur étendu (voir ci-dessous)
  ├── <id>.wasm           # absent pour un widget/contenu
  ├── lang/…              # locales (données, §10)
  ├── index.html          # widgets runtime uniquement
  └── signature.json      # modules (oblig.) / plugins-widgets (fac.)
```

- **Déterministe** : entrées triées, permissions/mtime fixes → même contenu ⇒ même
  octets ⇒ même `sha256`. Le **`content_hash` = sha256 de l'archive** est l'identité
  vérifiable et la clé de cache/CDN.
- **`manifest.json` = sur-ensemble du `bundle.json` actuel.** Il **reste
  rétrocompatible** : `load_bundle_dir` ne lit que `{id, kind, wasm, certified}` et
  ignore le reste ; l'installeur écrit ces 4 champs (avec `certified` = résultat de
  la vérif) et conserve le manifest complet à côté.

```jsonc
{
  "id": "obs",
  "kind": "module",                 // plugin | module | widget | content
  "version": "1.4.0",               // semver
  "wasm": "obs.wasm",               // absent si widget/content
  "requires": [                     // plugins : modules requis (repris de la manifest guest)
    { "module": "http", "version": "^1" }
  ],
  "capabilities": ["net"],          // modules : caps système demandées (⊆ caps_ceiling)
  "min_host": "0.3.0",              // version host minimale
  "publisher": "acme",              // id d'éditeur (certifié) ou pseudonyme
  "license": { "model": "free" },   // free | paid | subscription (+ sku, price) — §7
  "hashes": { "obs.wasm": "sha256:…" } // intégrité par fichier (défense en profondeur)
}
```

`signature.json` (détaché) : `{ alg:"ed25519", content_hash, sig, publisher_cert }`,
où `publisher_cert` est lui-même `{ fields…, ca_sig }`. Absent = non signé.

> Décision : **le paquet est content-addressé, pas nommé par confiance.** Deux
> éditeurs peuvent publier `id:"obs"` ; c'est le `publisher` + la chaîne de
> signature (et le namespace de registre) qui les distinguent, pas le seul `id`.
> L'installeur refuse d'écraser un `installed/module/obs` d'un autre `publisher`
> sans confirmation explicite (anti-hijack d'id).

## 4. Le registre (registry) (décision)

**Approche static-first**, choisie pour trois raisons : (1) surface serveur
minimale = surface d'attaque minimale ; (2) **auto-hébergeable trivialement** (un
bucket + CDN) — cohérent avec le cœur AGPL et un marketplace dérivable ; (3) la
confiance ne repose pas sur le serveur mais sur les **signatures** des paquets.

```
registry/
├── index.json            # catalogue signé (liste + métadonnées + dernières versions)
├── index.sig             # signature du registre sur index.json
├── crl.json (+ .sig)     # liste de révocation
└── blobs/
    └── sha256/<hash>.qtpkg   # paquets content-addressés (immuables)
```

`index.json` (extrait) — une entrée **par version** publiée :

```jsonc
{
  "schema": 1,
  "generated": "2026-08-01T00:00:00Z",
  "packages": [
    {
      "id": "obs", "kind": "module", "version": "1.4.0",
      "publisher": "acme", "certified": true,
      "content_hash": "sha256:…", "size": 812345,
      "requires": [{ "module": "http", "version": "^1" }],
      "capabilities": ["net"],
      "license": { "model": "free" },
      "min_host": "0.3.0",
      "i18n": { "name": "…", "summary": "…" }   // texte marketing, localisé (données)
    }
  ]
}
```

- `index.sig` (clé de registre, distincte du CA) authentifie *la liste*. La
  confiance ultime pour **charger** un module reste sa propre chaîne de signature —
  l'index compromis peut au pire cacher/mentir sur des métadonnées, pas fabriquer
  un module certifié valide.
- **Recherche** : d'abord côté client sur l'index téléchargé (petits catalogues) ;
  un service de recherche dédié pourra arriver plus tard sans changer le modèle de
  confiance.
- **Miroir / hors-ligne** : copier `registry/` suffit (voir §11).

## 5. Résolution de dépendances & versions (décision)

**Contrainte structurante : un seul `id` de module chargé à la fois.** Le loader
dédupe par id (`host.has_module(&id)`), donc deux plugins qui requièrent `obs` à
des versions incompatibles ne peuvent pas coexister. Le résolveur doit donc
produire **une** version résolue par id, satisfaisant **toutes** les contraintes.

Algorithme (à l'installation d'un plugin `P`) :

1. Lire `P.requires` → pour chaque module requis, interroger l'index pour les
   versions disponibles.
2. Choisir la **plus haute version compatible** avec l'union des contraintes déjà
   en vigueur (les `requires` des plugins **déjà installés** + celui de `P`).
   Conflit irréconciliable → **échec explicite** (message : quel plugin impose
   quelle borne), pas de double-install.
3. Télécharger récursivement les modules manquants (les modules peuvent, à terme,
   dépendre d'autres modules — même algo).
4. Rien d'inutile : un module déjà présent et compatible n'est pas retéléchargé.

**Semver réel (écart à combler).** L'actuel `version_satisfies` ne gère que `*`,
égalité, et `^major`. Décision : adopter la crate **`semver`** pour les *ranges*
(`>=1.2, <2`, `~1.4`, `^1`) et les *précédences*. `version_satisfies` devient un
mince adaptateur (ou est remplacé) ; ses tests existants restent valides comme
sous-ensemble. À câbler là où ça manque : **au load** (`load_plugin` doit vérifier
`is_satisfied` et refuser/avertir si un requis est absent) — aujourd'hui seul
`module_call` vérifie *la déclaration* de la dépendance, pas *la version*.

**Lockfile `installed.json`** (racine `<data>/`) — l'état résolu, source de vérité
de l'installeur :

```jsonc
{
  "schema": 1,
  "packages": {
    "module/obs":   { "version": "1.4.0", "content_hash": "sha256:…",
                      "publisher": "acme", "source": "registry",
                      "required_by": ["plugin/obs"], "caps_granted": ["net"],
                      "entitlement": null },
    "plugin/obs":   { "version": "2.1.0", "content_hash": "sha256:…",
                      "source": "registry", "required_by": ["user"] }
  }
}
```

`required_by` = **comptage de références** (déjà anticipé par `is_orphan_module`).
Désinstaller `plugin/obs` retire `obs` du `required_by` du module ; si la liste se
vide **et** que rien d'autre ne le requiert → le module devient orphelin et peut
être supprimé (proposé à l'utilisateur, pas forcé). `source ∈ {registry, sideload, dev}`.

## 6. Parcours d'installation (machine à états)

```
        resolve            fetch           verify            entitle
Requested ──────► Resolved ──────► Fetched ──────► Verified ──────► Entitled
   │  (deps+versions) (blob→cache) (hash+sig+CRL)  (droit d'usage)     │
   │                                                                   ▼
   │                                                     consent (caps système)
   │                                                                   │
   ▼                                                                   ▼
 Failed ◄───────────────── (toute étape) ──────────────────────► Installing
                                                                        │
                                                        extract → installed/<kind>/<id>/
                                                        (atomique : tmp/ → rename)
                                                                        │
                                                                        ▼
                                                            HotLoad (§7) → Active
```

- **Verify** : recalcul `content_hash`, chaîne de signature (modules), CRL. Échec
  ⇒ abort, rien n'est écrit hors du cache.
- **Entitle** : pour un paquet payant, vérifier le droit d'usage (§8) **avant**
  extraction. Gratuit ⇒ passe.
- **Consent** : pour un module, présenter l'**union des caps** (`capabilities`) à
  accorder ; persister le consentement (`caps_granted` du lockfile). Refus ⇒ abort.
- **Extraction atomique** : écrire dans `installed/.tmp/<id>/` puis `rename` vers
  `installed/<kind>/<id>/`. Une install interrompue ne laisse pas de bundle à
  moitié écrit que le boot chargerait.
- **Rollback** : toute erreur après extraction supprime le dossier et restaure le
  lockfile.

## 7. Chargement à chaud & déchargement (décision)

Aujourd'hui les extensions ne sont chargées **qu'au boot**. Le marketplace impose
un **load/unload runtime**.

- **Load à chaud** : après extraction, appeler le même chemin que le boot
  (`host.load_module`/`load_plugin` + `locales.load_lang_dir`), puis pousser un
  event IPC (`extensions-changed`) pour que l'UI rafraîchisse
  `list_plugins/modules/widgets`. Respecter l'ordre : un module avant les plugins
  qui le requièrent.
- **Unload** : chaque extension a son **`Store` wasmtime** ; décharger = retirer du
  registre host + drop du Store. Refuser de décharger un module encore requis
  (`required_by` non vide). Les widgets se déchargent côté catalogue (pas de Store).
- **Câbler `may_grant` (écart).** Le gating actuel est binaire (`certified &&
  caps.is_empty()`). Décision : au dispatch réel d'une cap système, consulter
  `ExtensionRegistry::may_grant(module_id, cap)` **et** le consentement utilisateur
  (`caps_granted`). Un module certifié qui déclare `net` mais dont l'utilisateur
  n'a accordé que… (le consentement est tout-ou-rien aujourd'hui, mais l'infra
  permet le par-cap). Cela ferme le trou « un module certifié obtient de fait
  toutes les caps ».

## 8. Licences & entitlements (décision, cf. §11.1)

> **Protection (DRM)** — activation, chiffrement du payload, binding machine,
> watermark, révocation : [drm.md](drm.md). Cette section pose les *droits*
> (entitlements) ; le doc DRM pose leur *application* (comment on empêche/trace la
> copie de ce qui est payant).

Chaque objet porte `license.model ∈ {free, paid, subscription}`. L'infra est prévue
dès le départ pour le payant même si le lancement est gratuit.

- **Entitlement** : un **jeton signé** par le service de comptes (clé distincte du
  CA), `{ account_id, sku, kind, id, not_after, machine_binding? }`, **vérifiable
  hors-ligne** contre la clé publique du store épinglée. Le host le contrôle à
  l'étape *Entitle* (install) **et** au chargement (un abonnement expiré désactive
  proprement l'extension au boot suivant, avec message — pas de crash).
- **Clé de licence hors-ligne** : variante sans compte — une chaîne =
  statement Ed25519-signé `{ sku, id, not_after }` que l'utilisateur colle ; même
  vérification.
- **Revenue share** : purement **comptable côté serveur** (le client ne calcule
  rien) ; réservé aux objets dont la provenance est prouvée → **modules certifiés**,
  plugins/widgets signés. Motive la certification (§11.1).
- **Modules fermés/payants autorisés** : le `.wasm` reste sandboxé, donc sûr même
  fermé. Lien avec **§15 (Cloud & comptes, local-first)** : le compte est
  **optionnel** — le gratuit et les clés hors-ligne marchent sans compte.

## 9. Disposition sur disque (décision)

```
<data>/  (= $HOME/.local/share/quicktys, persist.rs::data_dir)
├── installed/                 # DÉJÀ scanné au boot (bundle_roots)
│   ├── modules/<id>/{<id>.wasm, lang/, bundle.json, manifest.json, signature.json}
│   ├── plugins/<id>/…
│   └── widgets/<id>/…
├── installed.json             # lockfile (§5) — état résolu + ref counts + entitlements
├── registry-cache/            # index.json, crl.json, blobs/sha256/<hash>.qtpkg
├── consent.json               # caps accordées par extension (peut fusionner dans le lockfile)
└── storage/<id>.json          # DÉJÀ là (cap storage) — données propres aux extensions
```

`SUBDIRS` de `ensure_layout` (`persist.rs`) gagne `registry-cache` ; `installed`
existe déjà. `manifest.json`/`signature.json` voyagent **dans** le bundle (le boot
les ignore, seul `bundle.json` compte pour le chargement — rétrocompat).

## 10. Surface IPC & UI

Nouvelles commandes (`quicktys-ipc::AppBackend` + shims `#[tauri::command]`),
alignées sur le style existant (`call_module`, `list_*`) :

| Commande | Rôle |
| --- | --- |
| `marketplace_refresh` | (re)télécharger `index.json`/`crl.json` signés dans le cache |
| `marketplace_search { query, kind?, filters }` | interroger l'index en cache → entrées catalogue |
| `marketplace_details { id }` | fiche : versions, éditeur, caps, licence, deps |
| `install_package { id, version? }` | machine à états §6 (résout, télécharge, vérifie, consent, extrait, hot-load) |
| `uninstall_package { kind, id }` | retire, met à jour ref counts, propose la purge des orphelins |
| `update_package { id? }` | met à jour un/tous (respecte semver + épinglage) |
| `list_installed` | état local : version, source (registry/sideload/dev), maj dispo, entitlement |
| `sideload_package { path }` | installer un `.qtpkg` local (§11) |
| `enter_license_key { key }` | enregistrer une clé de licence hors-ligne (§8) |
| `grant_capabilities { id, caps }` | écran de consentement (§6/§7) |

Events poussés : `download-progress {id, pct}`, `extensions-changed` (l'UI
rafraîchit les listes), `install-result {id, ok, error?}`.

Écarts UI à combler : `ModuleInfo`/`PluginInfo` doivent exposer `certified`,
`version`, `publisher`, `source`, `update_available` pour l'écran « Extensions
installées ». Écrans : **Parcourir** (recherche/filtre), **Fiche** (install +
consentement + licence), **Installées** (maj/désinstall), **Consentement** (caps).

## 11. Sideload, hors-ligne, miroir (décision)

- **Sideload** : installer un `.qtpkg` du disque. Un **module** sideloadé est
  vérifié contre le CA épinglé exactement comme s'il venait du registre (une
  signature valide ne dépend pas de la provenance). Un module **non signé** exige
  le mode développeur (§2). Plugins/widgets sideloadés : autorisés, marqués
  `source:"sideload"`, avec avertissement si non signés.
- **Air-gapped / entreprise** : le registre étant statique, un **miroir** = copie
  de `registry/` sur un partage local ; l'URL de registre est un réglage. Plusieurs
  registres possibles (officiel + interne).
- **Dev** : `dist/bundles` reste prioritaire (`bundle_roots` d'abord) → un bundle
  en cours de dev masque sa version installée, sans toucher au marketplace.

## 12. Sécurité — modèle de menace (synthèse)

| Menace | Mitigation |
| --- | --- |
| Module malveillant tente net/clavier sans droit | sandbox WASM + gating certification + `may_grant` par-cap + consentement |
| Paquet altéré en transit / CDN compromis | `content_hash` recalculé + signature de release |
| Éditeur usurpé | chaîne CA→éditeur ; `publisher_cert` signé racine, clé CA épinglée |
| Éditeur qui déborde ses caps | `caps ⊆ caps_ceiling` du certificat |
| Module compromis a posteriori | CRL signée, revérifiée **au chargement** (pas qu'à l'install) |
| Index mensonger | `index.sig` ; et de toute façon la confiance de *chargement* vient des sig. de paquet, pas de l'index |
| Hijack d'id (`obs` d'un autre éditeur) | refus d'écraser un id d'un autre `publisher` sans confirmation |
| Downgrade vers version vulnérable | `min_host`, versions immuables content-addressées, épinglage lockfile |
| Contournement d'entitlement | jeton signé vérifié hors-ligne à l'install **et** au load |
| Install interrompue → bundle corrompu chargé au boot | extraction atomique `tmp/→rename` |

## 13. Mise en œuvre incrémentale (jalons M6)

Chaque étape est utilisable seule ; la confiance se durcit progressivement.

1. **Install runtime (sans registre).** `sideload_package` + machine à états §6
   (hash uniquement, pas encore signature), extraction atomique dans `installed/`,
   hot-load + `extensions-changed`, lockfile + ref counts. Débloque tout le reste.
2. **Semver & résolution.** Crate `semver`, résolution des `requires`, `is_satisfied`
   câblé au load, purge des orphelins.
3. **Signature & certification.** Chaîne CA→éditeur→release, keyring CA épinglé,
   CRL ; `certified` calculé (plus lu en clair) ; `may_grant` câblé au dispatch.
4. **Registre statique.** `index.json`/`crl.json` signés + blobs content-addressés,
   `marketplace_refresh/search/details/install/update`, UI Parcourir/Installées.
5. **Consentement par-cap.** Écran de consentement, `caps_granted`, re-prompt si une
   maj élargit les caps.
6. **Entitlements & payant.** Jetons signés + clés hors-ligne, `enter_license_key`,
   hooks revenue-share, lien comptes §15. (Lancement possible tout-gratuit avant.)

## 14. Décisions à acter / questions ouvertes

- **Opérations du CA** : qui détient la clé racine, cérémonie de signature,
  cadence de rotation, HSM ? (bloquant pour l'étape 3).
- **Hébergement du registre** : bucket+CDN officiel, coûts, politique de rétention
  des blobs immuables.
- **Système de comptes** (§15) : identité pour entitlements/revenue-share ;
  jusqu'où rester local-first (le gratuit + clés hors-ligne ne doivent **jamais**
  exiger de compte).
- **Processeur de paiement** et fiscalité du revenue-share.
- **Granularité du consentement** : tout-ou-rien vs par-cap au lancement (l'infra
  vise par-cap ; l'UX peut démarrer tout-ou-rien).
- **Format d'archive** : tar+zstd (retenu) vs zip ; figer les options de
  déterminisme (tri, mtime=0, mode fixe).
- **Multi-registres** : priorité/override entre officiel et registres internes.
