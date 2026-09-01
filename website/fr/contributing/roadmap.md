---
description: "L'état honnête de chaque jalon de Quicktys, du noyau host et des deux ABI d'extension jusqu'à la marketplace et au portage de masse restant."
---

# Roadmap

La migration depuis [CastMate](https://github.com/LordTocs/CastMate) est
**incrémentale** — chaque jalon est livrable et testable. La source de vérité est
[`docs/ROADMAP.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/ROADMAP.md).

::: info La checklist est en retard sur le code par endroits
M5 en est un bon exemple : ses cases ne sont pas cochées, alors que le système
d'overlays, le type d'extension widget et le serveur local sont implémentés et
utilisés quotidiennement. Là où cette page et la checklist divergent, cette page
reflète ce que le code fait réellement.
:::

## M0 — Scaffolding & architecture ✅

Arborescence du dépôt, workspace Cargo, contrat WIT à deux étages, document
d'architecture, épinglage exact des versions, `cargo check` vert sur tous les
crates host.

## M1 — Noyau host ✅

- `quicktys-schema` — schémas, validation, YAML/JSON, régions de template, modèle
  de nœuds.
- `quicktys-expr` — le moteur Rhai sandboxé avec limites d'opérations, de
  profondeur et de taille, analyse statique des dépendances, coercition par
  schéma.
- `quicktys-core` — ressources avec store mirroré en YAML, le **runner
  d'automatisation** sur tous les types de nœuds avec abort coopératif et
  concurrence structurée, la réactivité (`StateStore` à réévaluation ciblée par
  dépendances), le bus de triggers, les profils, le dispatcher, les files
  d'action, l'i18n, le registre des extensions avec résolution de dépendances et
  gating de certification.

Reste : les portées lexicales, un cache d'expressions compilées.

## M2 — ABI plugin de bout en bout ✅

Bindings host contre le world `plugin`, les quatre capacités sûres implémentées,
`quicktys-plugin-sdk` avec la macro `define_plugin!`, et un plugin compilé en
composant WebAssembly, chargé et exécuté de bout en bout avec son état observé
côté host.

## M3 — Coquille Tauri & UI ✅

`quicktys-ipc` comme façade sans Tauri et testée unitairement, plus le frontend
Vue 3 + PrimeVue écrit de zéro : tableau de bord, profils, éditeur
d'automatisation avec surlignage d'exécution en direct, variables, marketplace,
réglages.

## M4 — L'étage module ✅

Bindings host contre le world `module`, `quicktys-module-sdk`, le linker
plugin↔module validé, le gating de certification, et les capacités système câblées
pour de vrai : HTTP et WebSocket (TLS compris), lecture audio et synthèse vocale,
lancement de processus, injection d'entrées, stockage persistant, stockage de
jetons OAuth avec rafraîchissement transparent.

De vrais drivers ont suivi : `obs`, `twitch`, `pear`, `sound`, `os`, `input`,
`http`.

Reste : `audio.play-buffer`, `files`, `hid` et `timers` — déclarés dans l'ABI,
câblés quand un driver en aura besoin.

## M5 — Overlays & widgets ✅ *(checklist non mise à jour)*

Implémentés : le serveur HTTP local, le flux d'état SSE vers les sources
navigateur, le troisième type d'extension, l'éditeur d'overlays WYSIWYG avec de
vrais aperçus en direct et l'aimantation, l'isolation des widgets dans leur propre
schéma d'URI, et la grille du tableau de bord.

Huit widgets sont livrés. Voir [Overlays](/fr/guide/overlays) et [Tableau de
bord](/fr/guide/dashboard).

## M6 — Marketplace, DRM & distribution ✅

Les dix sous-jalons sont faits :

| | |
| --- | --- |
| M6.1 | le format `.qtpkg` et son manifeste |
| M6.2 | le runtime d'installation — matérialisation atomique, lockfile, chargement à chaud |
| M6.3 | semver et résolution de dépendances |
| M6.4 | `quicktys-crypto` — la suite hybride post-quantique |
| M6.5 | chaînes de signature, trust store, CRL, certification calculée |
| M6.6 | le registre statique signé, agnostique du transport |
| M6.7 | consentement par capacité, appliqué à chaque dispatch |
| M6.8 | droits et clés de licence vérifiables hors-ligne |
| M6.9 | payloads chiffrés, déchiffrés en mémoire au chargement |
| M6.10 | watermarking forensique |

Reste : le **service d'activation en ligne** — binding machine via le keystore de
l'OS, sièges, grâce d'abonnement. La cryptographie hybride `seal`/`open` est déjà
dans `quicktys-crypto` ; ce qui manque, c'est le serveur.

## M7 — Boutons ✅

Une conception **implémentée puis retirée** : un protocole de surface distante
générique (un deck web, l'appairage, un driver Stream Deck en `hid`) s'est révélé
trop de plomberie pour la valeur apportée. Il a été remplacé par un simple **widget
Bouton** sur le tableau de bord.

Le modèle event→profil est identique ; seule la surface a changé. La capacité
`hid` est conservée comme infrastructure générique, inutilisée par toute extension
livrée.

::: tip À lire comme une leçon de conception
Le retrait est documenté plutôt que discrètement annulé. Une surface distante
pourrait réapparaître plus tard comme widget runtime servi par le serveur
d'overlays — ce qui ne coûterait presque rien, l'infrastructure de widgets
existant déjà.
:::

## M8 — Migration & portage de masse 🔨

**Fait :** l'outil `quicktys-migrate`. Il lit le YAML CastMate d'un utilisateur,
écrit des ressources Quicktys valides par construction, et produit un rapport
Markdown. Délibérément, il ne tente **aucune** traduction automatique JavaScript →
Rhai, et rapporte tout ce qui est ambigu plutôt que de deviner.

**En cours :** le portage des ~30 plugins CastMate, par lots de complexité
croissante. Le lot 1 (`random`, `twitch`) est fait, et les extensions qui ont suivi
sont visibles dans `scripts/build-bundles.sh`.

## Ce qui n'est pas fait

Pour être clair :

- **Pas de binaires de release.** Aujourd'hui, vous le construisez vous-même.
- **Le programme de certification n'est pas ouvert.** La chaîne de signature
  fonctionne et est exercée par les clés de développement, mais l'autorité et le
  processus de revue sont encore en cours de mise en place.
- **Pas de service d'activation en ligne.**
- **Les jetons sont en JSON clair**, pas dans le trousseau de l'OS.
- **Le chemin du dossier de données est en dur** et non idiomatique sous macOS et
  Windows.
- **`files`, `hid`, `timers` et `audio.play-buffer`** sont déclarés mais non
  implémentés. Le poller de modules toutes les trois secondes tient lieu de
  `timers`.
- **Les formulaires de réglages de widget existent en double** — tableau de bord et
  éditeur d'overlays — avec du code séparé qui peut diverger.
- **Le CLA n'est pas rédigé** et les textes de licence n'ont pas eu de relecture
  juridique.

## Et ensuite

- [Architecture](/fr/contributing/architecture)
- [Licences](/fr/contributing/license)
