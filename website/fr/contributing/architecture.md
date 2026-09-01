---
description: "L'architecture de Quicktys : seize crates host, le runtime wasmtime, le moteur d'expressions Rhai, et le raisonnement consigné derrière chaque décision."
---

# Architecture

La référence qui fait foi est
[`ARCHITECTURE.md`](https://github.com/Ascol57/Quicktys/blob/main/ARCHITECTURE.md)
dans le dépôt — en français, et où chaque décision structurante est consignée.
Cette page en est la carte.

## La forme générale

```
┌────────────────────────────────────────────────────────────────────┐
│                         Application Tauri                           │
│                                                                     │
│  ┌──────────────────────────┐        ┌────────────────────────────┐ │
│  │   WebView (frontend)     │  IPC   │        Host Rust           │ │
│  │   Vue 3 + PrimeVue       │◄──────►│  quicktys-app (boot+Tauri) │ │
│  │   - formulaires générés  │ cmds/  │  quicktys-ipc (pont)       │ │
│  │   - i18n (vue-i18n)      │ events │  quicktys-core (moteur)    │ │
│  └──────────────────────────┘        │  quicktys-schema           │ │
│                                      │  quicktys-host (wasmtime)  │ │
│                                      └──────────────┬─────────────┘ │
│                          capacités système          │ ABI WIT        │
│                        ┌────────────────────────────┼──────────────┐ │
│                        │   MODULES WASM (certifiés) ▼              │ │
│                        │   obs · twitch · pear · sound · input     │ │
│                        │   importent : net/input/audio/hid/…       │ │
│                        │   exportent : services + events           │ │
│                        └───────────────┬──────────────────────────┘ │
│                     services/events    │ (le host câble et valide)   │
│                        ┌───────────────▼──────────────────────────┐ │
│                        │   PLUGINS WASM (tout le monde, isolés)   │ │
│                        │   actions · triggers · état · UI         │ │
│                        │   importent : logging/storage/reactivity/│ │
│                        │               modules  (PAS de réseau/OS)│ │
│                        └──────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────┘
```

Le host est la **seule** zone de pleine confiance.

## Les crates

Seize crates dans le workspace host. Les extensions en sont **exclues** — elles se
construisent pour `wasm32-wasip2` en `cdylib`, avec leurs propres lockfiles.

### Moteur

| Crate | Rôle |
| --- | --- |
| `quicktys-schema` | Schémas typés : validation, (dé)sérialisation YAML/JSON, régions de template, et le **modèle de nœuds** d'automatisation (partagé avec l'UI). |
| `quicktys-expr` | Le moteur **Rhai** sandboxé : compilation, analyse statique (extraction des dépendances depuis l'AST), coercition par schéma. |
| `quicktys-core` | Le moteur de domaine : registre des extensions, ressources, profils, le **runner de séquences**, files d'action, réactivité, bus d'événements, i18n, persistance. |
| `quicktys-host` | Le runtime wasmtime : chargement des composants, capacités, gating de certification, linker plugin↔module. |

### Application

| Crate | Rôle |
| --- | --- |
| `quicktys-ipc` | Le catalogue de commandes et d'événements host ↔ WebView. **Sans dépendance à Tauri**, donc testable unitairement sans WebView. |
| `quicktys-app` | Le binaire : boot, runtime Tokio, coquille Tauri. De fines coquilles `#[tauri::command]` plus le câblage. |

### SDK invités — MIT

| Crate | Rôle |
| --- | --- |
| `quicktys-plugin-sdk` | SDK invité pour les plugins. |
| `quicktys-module-sdk` | SDK invité pour les modules. |

### Distribution

| Crate | Rôle |
| --- | --- |
| `quicktys-pkg` | Le format `.qtpkg` et son manifeste. |
| `quicktys-install` | Matérialiser un paquet dans `installed/`, atomiquement. Agnostique du host. |
| `quicktys-crypto` | La suite hybride post-quantique : signatures, scellement, watermarking. |
| `quicktys-cert` | Chaînes de signature, trust store, entitlements, clés de licence. |
| `quicktys-registry` | Récupérer, cacher, chercher et résoudre un registre. Agnostique du transport — le HTTP est injecté par un trait `Fetcher`, donc testable hors-ligne. |
| `quicktys-regserve` | Le serveur de registre. *Propriétaire.* |
| `quicktys-pack` | Empaquetage, signature, émission de licences. *Propriétaire.* |
| `quicktys-migrate` | L'outil de migration CastMate. |

## Décisions à connaître

### JSON plutôt que du WIT typé

La partie typée de l'ABI est la signature WIT ; les payloads sont du JSON validé
contre des schémas déclarés. Encoder les types de schéma riches en WIT figerait
l'ABI à chaque nouveau type.

### Rhai, côté host, sur un contexte matérialisé

Les templates ne sont pas du JavaScript arbitraire. Parce qu'une expression est un
arbre syntaxique, le host extrait ce qu'elle lit — et c'est ce qui achète la
réévaluation réactive ciblée, la validation à la saisie et l'autocomplétion.

L'évaluation est synchrone, sur un contexte déjà matérialisé : aucune I/O pendant
l'évaluation.

### Deux syntaxes de template, délibérément pas Rhai

Les champs de config d'action utilisent `@chemin` et Handlebars, pas Rhai. Ce sont
deux langages pour deux métiers : produire du texte, ou produire une décision.

Le seul endroit où ils convergent : `ExprEngine::eval` imbrique les clés pointées
via le même `nest_context` qu'utilise la couche Handlebars, et `compile()` retire
un `@` en tête. Donc `obs.scene == "Intro"` et `@obs.scene == "Intro"` marchent
tous les deux, et ce que l'autocomplétion insère dans un champ d'action se colle
tel quel dans une condition.

### La réactivité sur `tokio::sync`, pas un framework de signaux

Le suivi de dépendances est ici **explicite** — extrait de l'AST Rhai — donc un
framework de signaux auto-traçants serait le mauvais outil. Le `StateStore` diffuse
les clés modifiées sur un canal `broadcast` derrière un `RwLock`, et `wait_until`
ne réévalue que si une clé de l'ensemble de dépendances de l'expression change.

Comme les dépendances sont des **identifiants racines** (`obs`) alors que les clés
sont pointées (`obs.scene`), la comparaison se fait par segment racine. Une égalité
stricte aurait empêché tout `wait-for until` de se réveiller.

### 100 % WASM, aucune échappatoire native

Il n'y a pas de module natif hors-processus. Si un matériel exotique dépasse ce
qu'exposent les capacités du host, la réponse est d'**étendre les capacités du
host** en Rust, dans la zone de confiance — pas d'ouvrir une brèche dans le bac à
sable.

### Local-first

L'app est pleinement fonctionnelle hors-ligne, sans compte. Un compte ne débloque
que des suppléments optionnels : sync multi-machine, achats sur la marketplace,
relais distant. Aucune fonctionnalité locale n'est verrouillée derrière, et il n'y
a pas de télémétrie obligatoire.

### La contrainte clean-room

Quicktys ne contient **aucune ligne** de code
[CastMate](https://github.com/LordTocs/CastMate). Les concepts et les APIs ont été
étudiés ; le code, jamais. C'est pour ça que le frontend est une application
Vue 3 + PrimeVue écrite de zéro plutôt qu'une réutilisation de la bibliothèque
d'UI de CastMate — la stack est partagée, le code non.

Lire le YAML CastMate d'un utilisateur pour la migration est licite : c'est sa
donnée, pas le code de CastMate.

## Questions ouvertes

Consignées honnêtement dans `ARCHITECTURE.md` §16 :

- **Relecture juridique.** L'exception d'extension et le CLA demandent encore un
  juriste. Si la frontière d'extension est mal définie, un plugin pourrait être
  requalifié en dérivé — ou l'inverse.
- **`quicktys-schema` est lié dans chaque extension.** Les SDK ré-exportent
  `Schema`, donc ce crate AGPL est compilé dans les extensions. C'est le même
  problème que l'exception règle pour les SDK, mais le crate n'y est pas nommé. Il
  faut l'ajouter à la liste de l'ABI ou le passer en MIT.
- **Granularité des capacités** — assez fine pour la sécurité, assez simple pour
  les auteurs de modules.
- **Performance de l'ABI JSON** — les actions à haute fréquence pourraient exiger
  un chemin binaire ou un linking statique de composants.
- **Maturité du Component Model** — les versions de wasmtime et wit-bindgen sont
  épinglées exactement et suivies.
- **Infrastructure de certification** — l'autorité, le processus de revue, la
  révocation de clés.

## Ordre de lecture

1. [`ARCHITECTURE.md`](https://github.com/Ascol57/Quicktys/blob/main/ARCHITECTURE.md) — la référence qui fait foi.
2. [`docs/automation-engine.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/automation-engine.md) — le runner, le modèle de nœuds et les expressions.
3. [`docs/marketplace.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/marketplace.md) — paquets, registre, signature, résolution.
4. [`docs/drm.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/drm.md) — droits, chiffrement, watermarking, et un modèle de menace honnête.
5. [`docs/ROADMAP.md`](https://github.com/Ascol57/Quicktys/blob/main/docs/ROADMAP.md) — les jalons.

## Et ensuite

- [Compiler depuis les sources](/fr/contributing/building)
- [Roadmap](/fr/contributing/roadmap)
