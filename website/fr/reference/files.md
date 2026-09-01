---
description: "Où Quicktys range profils, automatisations, extensions installées, consentements, jetons et état, ainsi que les ports et schémas d'URI qu'il utilise."
---

# Fichiers & dossier de données

## Le dossier de données

```
<user-data>/quicktys/
├── settings/          réglages de l'app et des extensions
├── profiles/          un <id>.yaml par profil
├── automations/       un <id>.yaml par automatisation autonome
├── queues/            configuration des files d'action
├── accounts/          ressources de compte
├── secrets/           valeurs secrètes
├── state/             état réactif persisté
├── storage/           <id>.json — la capacité `storage`, par extension
├── logs/
├── installed/         extensions matérialisées par la marketplace
├── installed.json     le lockfile d'installation
├── consent.json       les capacités accordées par module
├── entitlements.json  les clés de licence saisies
├── oauth.json         jeux de jetons + envs de fournisseur (0600 sous unix)
└── registry-cache/    index, CRL et blobs téléchargés
```

::: warning L'emplacement est actuellement en dur
Il est construit comme `$HOME/.local/share/quicktys`. Ça marche sur macOS mais ce
n'est pas la convention (`~/Library/Application Support`), et sous **Windows
`HOME` n'est pas défini** — l'app retomberait sur `%TEMP%` et perdrait tout au
redémarrage. `run-windows.ps1` fixe `HOME=%USERPROFILE%` pour contourner ça.
:::

## Règles de persistance

Tout ce qui est visible par l'utilisateur est du **YAML**, un fichier par
ressource, écrit de façon **atomique** (fichier temporaire, puis renommage). Les
ids sont assainis en noms de fichiers.

Les automatisations stockent les expressions sous forme de **texte source** ;
l'arbre syntaxique est compilé au chargement, donc une expression cassée est une
erreur qu'on voit avant qu'elle ne devienne une surprise à l'exécution.

## `profiles/<id>.yaml`

```yaml
id: gaming
name: Gaming
activation: 'obs.scene == "Game"'
bindings:
  - trigger: twitch.chat-message
    filter: 'message.starts_with("!")'
    sequence:
      kind: block
      body:
        - kind: action
          plugin: notify
          action: log
          config: { message: "{{ user }} : {{ message }}" }
```

Un profil possède ses séquences en ligne plutôt que de référencer des
automatisations par id.

## `automations/<id>.yaml`

L'arbre de nœuds, étiqueté par `kind`. Voir [Nœuds
d'automatisation](/fr/reference/nodes).

## `installed/` et `installed.json`

```
installed/
├── modules/<id>/{<id>.wasm, lang/, bundle.json}
├── plugins/<id>/{<id>.wasm, lang/, ui/, bundle.json}
└── widgets/<id>/{bundle.json, lang/, *.html}
```

Exactement la forme que le scanner de démarrage lit déjà. L'installeur extrait un
`.qtpkg` de façon atomique (`.tmp/` puis renommage) et écrit un `bundle.json`
compatible depuis le `manifest.json` du paquet. Les widgets gardent leur
`bundle.json` authoré, qui est plus riche.

`installed.json` est la source de vérité : versions, empreintes, source, et
`required_by` — le **comptage de références** qui identifie les modules orphelins.
Un module installé uniquement comme dépendance est marqué non explicite et devient
purgeable quand plus rien n'en a besoin.

## `bundle.json`

Ce que le host lit au chargement :

```json
{ "id": "obs", "kind": "module", "wasm": "obs.wasm", "certified": true }
```

`certified` est **calculé à l'installation** depuis la chaîne de signature, et non
lu dans un fichier modifiable. Le `bundle.json` d'un widget est authoré et bien
plus riche — voir [Écrire un widget](/fr/dev/widget).

## `consent.json`

Les capacités accordées par module. Le modèle est **grâce jusqu'à revue** : un
module que vous n'avez jamais revu obtient ce qu'il a déclaré, pour que rien ne
casse avant que vous n'ouvriez l'écran de consentement ; ensuite, seules les
capacités accordées sont autorisées.

Une révocation prend effet immédiatement. Une mise à jour qui élargit les capacités
d'un module voit les nouvelles refusées jusqu'à re-revue.

## `oauth.json`

Les jeux de jetons **et** les environnements de fournisseur nécessaires au
rafraîchissement — les deux, parce que rafraîchir après un redémarrage exige aussi
l'environnement. `0600` sous Unix.

L'expiration est stockée en **secondes Unix absolues** : un `Instant` monotone est
dénué de sens précisément au moment du redémarrage où on le relit.

Un jeton mort et non rafraîchissable est jeté plutôt que rechargé en faux
« connecté ».

::: warning Encore du JSON en clair
Protégé des autres utilisateurs de la machine, pas de ce qui tourne sous votre
propre compte. Le trousseau de l'OS est l'étape suivante.
:::

## Ordre de recherche des bundles au démarrage

1. `dist/bundles/` — l'arborescence de développement (sautée si
   `QUICKTYS_NO_DEV_BUNDLES=1`)
2. `<user-data>/quicktys/installed/` — l'arborescence de la marketplace

**Les modules se chargent avant les plugins.** Un id déjà chargé est ignoré, donc
un bundle installé masque un bundle de développement de même id.

## Les ports

| Port | |
| --- | --- |
| **8787** | le serveur HTTP local de l'app — overlays (`/overlay/<id>`), assets de widgets (`/widget/<id>/…`), flux d'état SSE (`/events`), et `POST /action` |
| **8000** | le défaut de `quicktys-regserve`, en développement |

## Les schémas d'URI

Chaque surface isolée reçoit sa propre origine, donc sa propre CSP, et ne peut pas
lire le stockage de l'app :

| Schéma | Sert |
| --- | --- |
| `widget://localhost/<id>/` | les fichiers d'un widget runtime (`http://widget.localhost/<id>/` sous Windows) |
| `pluginui://<id>/<fichier>` | l'UI de slot custom d'un plugin, depuis `plugins/<id>/ui/` |
| `moduleui://<id>/<fichier>` | la vue contribuée par un module, depuis `modules/<id>/ui/` |

## Sauvegarder

Copiez `<user-data>/quicktys/`. Profils, automatisations et réglages sont du YAML
simple — diffable, portable d'une machine à l'autre, et relisable dans git si vous
le souhaitez.

La disposition du tableau de bord n'y est **pas** : elle vit dans le stockage local
du frontend, par machine.

## Et ensuite

- [Outils en ligne de commande](/fr/reference/cli)
- [Architecture](/fr/contributing/architecture)
