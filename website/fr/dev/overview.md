---
description: "Le modèle d'extension de Quicktys : plugin, module et widget, ce que chacun peut importer, la règle d'ABI JSON sur WIT, et la structure des bundles d'extension."
---

# Le modèle d'extension

Tout ce qui, dans Quicktys, parle au monde extérieur est une extension. Il en
existe trois sortes, et laquelle vous écrivez est décidé par ce que vous devez
toucher.

## Choisir son étage

| | Plugin | Module | Widget |
| --- | --- | --- | --- |
| **Qui peut publier** | n'importe qui | développeurs certifiés, signé | n'importe qui |
| **Langage** | Rust → composant WebAssembly | Rust → composant WebAssembly | HTML/CSS/JS |
| **Bac à sable** | wasmtime, `Store` dédié | wasmtime, `Store` dédié | iframe navigateur |
| **Fournit** | actions, triggers, état, réglages | services, events | une surface rendue |
| **Peut importer** | `logging`, `storage`, `reactivity`, `modules` | les précédents plus `events`, `oauth`, et les capacités système | rien — un pont de messages |
| **Certification** | non requise | **requise** pour toute capacité système | non requise |

La décision est mécanique :

- **Besoin du réseau, du disque, d'un périphérique, ou de lancer un processus ?**
  C'est un **module**, et il doit être certifié.
- **Juste de la logique utilisateur qui compose d'autres choses ?** C'est un
  **plugin**.
- **Juste des pixels ?** C'est un **widget** — aucun WebAssembly.

Un plugin ne peut pas tricher pour monter d'un étage. Les interfaces de capacités
système ne sont pas dans son world WIT : il ne peut même pas les nommer.

## Comment les deux étages WASM s'articulent

```
Host (noyau Rust de confiance)
  expose : logging, storage                          (les deux étages)
           reactivity, modules                       (→ plugins)
           events, oauth,                            (→ modules)
           net, input, audio, hid, files,
           process, timers                           (→ modules certifiés uniquement)
   │
   ├─ MODULES (WASM, certifiés) — drivers / adaptateurs de protocole
   │    importent : les capacités système
   │    exportent : services + events
   │
   └─ PLUGINS (WASM, tout le monde) — logique utilisateur
        importent : logging / storage / reactivity / modules
        exportent : actions / triggers / état / réglages
```

Quand un plugin appelle un module, le host route et valide : le plugin doit avoir
déclaré la dépendance, la méthode doit exister, et le payload doit correspondre au
schéma de service publié par le module. Les events émis par un module sont
diffusés aux plugins abonnés.

## La règle de l'ABI

Une seule règle gouverne tous les contrats du système :

> **La partie typée du contrat est la signature de fonction WIT. Tout payload
> structuré — config, contexte, arguments de service, events, schémas, valeurs
> d'état — est une chaîne JSON, validée par le host contre le schéma que
> l'extension a déclaré dans son manifeste.**

Pourquoi ne pas tout typer en WIT ? Parce que le système de schémas est riche et
évolue encore — `Duration`, `Color`, `Resource`, `Range`, `MediaFile`. Les encoder
en types WIT figerait l'ABI à chaque nouveau type. JSON + validation par schéma
garde l'ABI **stable** pendant que le modèle de schémas grandit.

Les contrats vivent dans
[`wit/host.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/host.wit)
(capacités) et
[`wit/world.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/world.wit)
(les worlds `plugin` et `module`), paquet `quicktys:api@0.2.0`.

## Les manifestes déclarent, ils ne traduisent pas

L'`init()` d'une extension retourne un manifeste décrivant tout ce qu'elle offre :
actions avec leurs schémas de config, triggers avec leurs schémas de contexte,
clés d'état, réglages, modules requis.

Le manifeste émet des **clés i18n, jamais des chaînes traduites**. Le texte humain
vit dans le dossier `lang/` de l'extension, qui voyage en données à côté du
`.wasm`. Voir [Traductions](/fr/dev/i18n).

## Les bundles

Une extension n'est pas chargée depuis un `.wasm` brut à un chemin en dur. Elle
est chargée depuis un **bundle** — un dossier auto-descriptif :

```
plugins/<id>/
├── <id>.wasm        le composant
├── lang/            les locales (en.json, fr.json, …)
├── ui/              optionnel : HTML pour les contributions d'UI
└── bundle.json      { id, kind: "plugin", wasm, certified }
```

Les widgets n'ont pas de `.wasm` — leur bundle est authoré, pas compilé.

Le host scanne deux racines : `dist/bundles` (développement) puis
`<user-data>/quicktys/installed/` (marketplace). **Les modules se chargent avant
les plugins**, puisque les plugins en dépendent. Un id déjà chargé est ignoré, donc
un bundle installé masque un bundle de développement de même id.

## Quoi lire ensuite

| Vous voulez | Allez à |
| --- | --- |
| Monter une chaîne d'outils et construire quelque chose | [Environnement de dev](/fr/dev/environment) |
| Écrire de la logique utilisateur | [Écrire un plugin](/fr/dev/plugin) |
| Écrire un driver qui parle un protocole ou pilote un périphérique | [Écrire un module](/fr/dev/module) |
| Dessiner quelque chose sur le tableau de bord ou à l'antenne | [Écrire un widget](/fr/dev/widget) |
| Ajouter un bouton ou un onglet entier à l'app | [Contributions d'UI](/fr/dev/ui) |
| Le distribuer | [Empaqueter & publier](/fr/dev/packaging) |

## Les licences, en bref

Les SDK invités (`quicktys-plugin-sdk`, `quicktys-module-sdk`) sont sous **MIT** :
aucune obligation de copyleft ne peut atteindre votre extension par leur
intermédiaire. Le cœur est en AGPL-3.0 avec une **exception d'extension** : une
extension qui parle au cœur uniquement via l'ABI officielle n'est pas une œuvre
dérivée et peut être licenciée comme vous le souhaitez, y compris fermée et
commerciale.

Les extensions du dépôt Quicktys sont propriétaires et source-available —
lisez-les, apprenez-en comment elles appellent l'ABI, mais écrivez les vôtres
plutôt que de les copier. Détails dans [Licences](/fr/contributing/license).
