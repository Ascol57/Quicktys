# Architecture de Quicktys

> Réécriture **clean-room** de CastMate en Rust + Tauri + plugins WebAssembly.
> Ce document est la référence d'architecture du projet. Il est volontairement
> détaillé : il fige les décisions structurantes avant d'écrire le code.

## 0bis. Licence & propriété (contrainte fondatrice)

- **Réécriture clean-room** : aucune ligne de code de CastMate (MPL-2.0) n'est
  copiée. Voir « ⚠️ Réécriture clean-room » en §1.
- **Licence (décision) : AGPL-3.0 + exception d'extension + CLA.** Le but : un
  dérivé fermé ne peut pas être revendu, et un **marketplace alternatif doit
  publier son code**, tout en **autorisant la vente de plugins/modules/widgets**.
  - **AGPL-3.0** (copyleft *réseau*) — toute version modifiée **distribuée ou
    exposée comme service** doit publier ses sources. ⇒ pas de fork fermé
    payant ; un marketplace dérivé de notre code est **forcé à l'open source**.
  - **Exception d'extension** (style Classpath/GCC-runtime) — une extension qui
    communique **uniquement via l'ABI officielle** (WIT/IPC : plugins, modules,
    widgets) **n'est pas une œuvre dérivée** du cœur et peut être licenciée/
    **vendue** librement, y compris **fermée**. C'est l'isolation WASM + l'ABE
    stable qui rend ce découplage défendable.
  - **CLA** (Contributor License Agreement) — les contributeurs cèdent/licencient
    leurs droits au **propriétaire**, qui détient ainsi le projet et peut
    **dual-licencier** (vendre des licences commerciales exemptées d'AGPL).
  - *Option plus stricte (si l'on veut interdire toute revente du cœur, pas
    seulement la fermer) : ajouter une **Commons Clause** → mais cela rend le
    cœur « source-available », non-OSI. À trancher avec le/la juriste.*
  - ⚠️ *Pas un avis juridique ; le texte de l'exception et le CLA doivent être
    rédigés/validés par un·e juriste, idem pour les CGU du marketplace.*
- **Textes rédigés (décision, août 2026)** — les fichiers de licence existent
  désormais à la racine, et [`COPYING.md`](COPYING.md) en donne la carte :
  - [`LICENSE`](LICENSE) — texte AGPL-3.0 verbatim (FSF, 661 lignes) ;
  - [`LICENSE-EXCEPTION`](LICENSE-EXCEPTION) — *Quicktys Extension Exception
    v1.0*, permission additionnelle au titre de l'**AGPL §7**. Elle définit
    l'**« Official Extension ABI »** de façon limitative : contrats WIT de
    `wit/`, **SDK invités** (`quicktys-plugin-sdk`, `quicktys-module-sdk`),
    catalogue IPC host ↔ WebView, pont widget (`widget://` + `postMessage`),
    formats manifeste/bundle/schéma/`lang/`. ⚠️ Point non trivial : les SDK
    invités sont **liés statiquement** dans chaque extension — sans leur
    inscription explicite dans l'ABI, toute extension serait dérivée d'un crate
    AGPL et l'exception ne servirait à rien (même problème que la *GCC Runtime
    Library Exception*). Toute évolution des SDK doit donc rester couverte par
    le §0(b) de l'exception.
  - [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS) — **décision** : les extensions de
    référence livrées dans ce dépôt (`plugins/`, `modules/`, `widgets/`) ne sont
    **pas** permissives comme envisagé initialement, mais **propriétaires
    source-available** © Ascol57 (lecture/compilation/usage local autorisés ;
    redistribution, fork public, revente et réutilisation interdits). Leurs
    `Cargo.toml` déclarent `license = "LicenseRef-Ascol57-Proprietary"`. L'ABI
    publique reste, elle, libre d'accès : on peut écrire sa propre extension
    contre la même ABI, pas réutiliser ce code-ci.
  - SPDX du cœur : `AGPL-3.0-only WITH quicktys-extension-exception`
    (identifiant d'exception **personnalisé**, non enregistré auprès de SPDX).
  - **Reste à faire** : le texte du **CLA**, et la relecture juridique de
    l'ensemble.

## 1. Objectifs et contraintes

CastMate est un outil d'automatisation pour streamers : des **triggers**
(événements Twitch, chat, IoT…) déclenchent des **automations** (séquences
d'**actions**), organisées en **profils**. Toute l'UI est générée à partir de
**schémas** typés, et l'état des plugins est propagé de façon réactive vers
l'interface.

La réécriture vise trois capacités que l'architecture Electron actuelle rend
difficiles :

1. **Traductions** — i18n cohérent du host jusqu'aux plugins.
2. **Marketplace** — installer des extensions/contenus tiers **en sécurité**.
3. **Remote buttons** — piloter les automations depuis des télécommandes /
   satellites.

Le point (2) est la raison profonde du choix WASM : une extension tierce ne doit
**jamais** pouvoir exécuter du code natif arbitraire sur la machine de
l'utilisateur. WebAssembly + Component Model donne un **bac à sable** avec des
**capabilities** explicites.

### Types d'extension (décision structurante)

Quicktys distingue **trois** types d'extension. Deux sont des composants WASM
sandboxés avec des niveaux de confiance distincts ; le troisième est de la
présentation (frontend) :

- **Plugin** *(WASM, tout le monde)* — le *lien backend ↔ utilisateur* : actions,
  triggers, UI. Sandbox stricte, **aucun** accès externe direct. Consomme des
  modules.
- **Module** *(WASM, dev certifié)* — un *driver* : « comment parler à OBS /
  Twitch / l'audio système / le clavier ». Reçoit les **capacités système**
  (réseau, injection clavier, audio, USB/HID). Expose des **services** typés +
  des **événements**.
- **Widget** *(frontend, tout le monde)* — un *overlay/dashboard* affiché dans une
  browser source OBS ou une fenêtre : composants UI + schéma de config + `lang/`.
  Servi par le serveur HTTP local du host, alimenté en **state** via websocket.
  Ce n'est **pas** un composant WASM (présentation pure) ; son JS tourne isolé
  dans la browser source. Voir §12.

Le **cœur ne contient aucun module ni widget** : si l'utilisateur n'en a pas
besoin, il ne les télécharge même pas. Tout est résolu **à la demande** (cf.
§4.4). Voir §4 pour le modèle WASM complet et §12 pour les widgets.

### ⚠️ Réécriture clean-room (contrainte de licence)

**Quicktys ne réutilise AUCUNE ligne de code de CastMate.** C'est une
**réimplémentation clean-room** : on étudie CastMate pour comprendre le *problème*
et l'*architecture* (les concepts/APIs ne sont pas protégés), mais **tout le code
est écrit de zéro**. Objectif : une licence **détenue/contrôlée par le
propriétaire** (cf. §0bis), adaptée au marketplace, sans l'encombrement de la
MPL de CastMate.

Conséquences :

- Le **frontend est réécrit** (pas de `castmate-ui-core` vendorisé). On garde la
  *stack* (Vue 3, PrimeVue) — libs tierces indépendantes — mais pas le code.
- Les mentions « équivalent / port de CastMate » dans ce document désignent une
  **filiation conceptuelle**, jamais une reprise de code.
- Lire les **fichiers de données** de l'utilisateur (ses YAML CastMate) pour la
  migration est OK — c'est sa donnée, pas du code CastMate.
- Quicktys est **volontairement gitignoré** par le repo CastMate (`/Quicktys`
  dans `.gitignore`) : il n'est pas suivi par l'historique git MPL de CastMate,
  la frontière de licence est donc déjà nette. **Décidé : Quicktys a désormais
  son propre dépôt git** (`git init` à la racine `Quicktys/`, remote
  `github.com/Ascol57/Quicktys`), versionné indépendamment de CastMate.

### Concepts repris (filiation, réimplémentés de zéro)

- Le **contrat de plugin** : actions / triggers / state / settings / secrets /
  resources.
- Le **système de schémas** qui génère l'UI et valide les données.
- Le **modèle d'exécution des séquences** (refondu, cf. §5bis).
- La **propagation réactive** de l'état vers l'UI.
- La **persistance YAML** (compatibilité des fichiers utilisateur visée).

### Ce qui change fondamentalement

- **Electron → Tauri** (host Rust, WebView pour l'UI).
- **Plugins JS monolithiques → deux étages WASM** : on **scinde** chaque plugin
  CastMate (qui mélangeait protocole + UI) en un **module** certifié (le
  protocole/connexion) et un **plugin** (les actions/triggers/UI). Chargés
  dynamiquement, sandboxés.
- **Réactivité async maison (TS) → équivalent Rust** côté host.
- Frontier de sécurité explicite entre host (confiance) et plugins (zéro
  confiance).

## 2. Vue d'ensemble

```
┌────────────────────────────────────────────────────────────────────┐
│                         Application Tauri                           │
│                                                                      │
│  ┌──────────────────────────┐        ┌────────────────────────────┐ │
│  │   WebView (frontend)      │  IPC   │        Host Rust            │ │
│  │   Vue 3 + PrimeVue        │◄──────►│  quicktys-app (boot+Tauri)  │ │
│  │   (réécrit, clean-room)   │ cmds/  │  quicktys-ipc (pont)        │ │
│  │   - formulaires (schémas) │ events │  quicktys-core (moteur)     │ │
│  │   - i18n (vue-i18n)       │        │  quicktys-schema            │ │
│  └──────────────────────────┘        │  quicktys-host (wasmtime)   │ │
│                                       └──────────────┬─────────────┘ │
│                            capacités système         │ WIT ABI        │
│                          ┌───────────────────────────┼──────────────┐│
│                          │   MODULES WASM (certifiés) ▼              ││
│                          │   obs · twitch · audio · input            ││
│                          │   import: net/input/audio/hid/…           ││
│                          │   export: services + events               ││
│                          └───────────────┬───────────────────────────┘│
│                            services/events│ (host wire + valide)        │
│                          ┌────────────────▼──────────────────────────┐│
│                          │   PLUGINS WASM (tout le monde, sandbox)    ││
│                          │   actions · triggers · state · UI          ││
│                          │   import: logging/storage/reactivity/      ││
│                          │           modules  (PAS de réseau/OS)      ││
│                          │   export: plugin-guest (invoke-action, …)  ││
│                          └────────────────────────────────────────────┘│
└────────────────────────────────────────────────────────────────────┘
```

Le host est **la seule zone de pleine confiance**. Les **plugins** n'ont aucun
accès au réseau/disque/OS ; ils passent par les **services des modules**. Les
**modules** (certifiés) sont la seule couche à recevoir les capacités système,
et chaque appel plugin→module est routé et validé par le host.

## 3. Découpage en crates

> La colonne « Analogie » indique la **filiation conceptuelle** (code écrit de
> zéro, jamais repris de CastMate).

| Crate                 | Rôle | Analogie (concept) |
| --------------------- | ---- | ------------------- |
| `quicktys-schema`     | Schémas typés, validation, (dé)sérialisation YAML/JSON, régions de template, modèle de nœuds d'automatisation | `castmate-schema` |
| `quicktys-expr`       | Moteur d'expressions sandboxé (Rhai) : templating + contrôle de flux, stdlib, analyse statique, coercition | (nouveau) |
| `quicktys-core`       | Extensions (plugins+modules), resources, profils, runner d'automatisation, réactivité, bus, persistance | `castmate-core` (partie non-plugin) |
| `quicktys-host`       | Chargement/exécution des composants WASM, capabilities, gating, certification, linker plugin↔module | (nouveau) |
| `quicktys-ipc`        | Catalogue de commands/events host↔WebView | `util/electron.ts` (IPC) |
| `quicktys-app`        | Binaire : boot, runtime Tokio, shell Tauri | `packages/castmate` (main) |
| `quicktys-plugin-sdk` | SDK guest pour écrire des **plugins** (tout le monde) | `definePlugin` & co |
| `quicktys-module-sdk` | SDK guest pour écrire des **modules** (devs certifiés) | (nouveau) |

## 4. Modèle d'extension à deux étages (WIT / Component Model)

Défini dans [`wit/host.wit`](wit/host.wit) (capabilities) et
[`wit/world.wit`](wit/world.wit) (les deux worlds : `plugin`, `module`).

Principe directeur **commun aux deux étages** :

- **La partie typée du contrat = les signatures de fonctions WIT.**
- **Les payloads structurés (config, context, services, événements, schémas,
  valeurs d'état) = chaînes JSON**, validées par le host contre les schémas que
  l'extension déclare dans son manifeste.

Pourquoi pas tout typer en WIT ? Parce que les schémas de CastMate sont riches,
dynamiques et évolutifs (Duration, Color, Resource, Range, MediaFile…). Les
encoder en types WIT figerait l'ABI à chaque nouveau type. JSON + validation par
schéma garde l'ABI **stable** pendant que le système de schémas évolue.

### 4.1 Hiérarchie de confiance et de capabilities

```
Host (kernel Rust, confiance totale)
  expose : logging, storage           (partagé)
           reactivity, modules         (→ plugins)
           events + net/input/audio/   (→ modules certifiés uniquement)
                    hid/files/process/timers
   │
   ├─ MODULES (WASM, CERTIFIÉS) — drivers/adaptateurs de protocole
   │    import : capacités système ;  export : services + events
   │    ex. obs, twitch, audio, input
   │
   └─ PLUGINS (WASM, TOUT LE MONDE) — logique utilisateur
        import : logging/storage/reactivity/modules (PAS de capacités système)
        export : actions/triggers/state/settings ;  consomme les modules
```

Un plugin **ne peut pas** demander de capacité système : ces interfaces ne sont
tout simplement pas dans son world. Seuls les modules, **après vérification de
leur certification**, se voient accorder `net`/`input`/`audio`/`hid`/… — et
encore, seulement celles que leur manifeste déclare et que l'utilisateur a
validées à l'installation.

### 4.2 Le PLUGIN (`world plugin`, tout le monde)

- **Importe** : `logging`, `storage`, `reactivity` (`set-state`,
  `emit-trigger`), `modules` (`call`, `subscribe`).
- **Exporte** (`plugin-guest`) :
  - `init() -> string` : manifeste JSON (id, name, version, icon, color, la
    déclaration actions/triggers/state/settings/secrets + **les modules requis**
    en semver).
  - `on-load() / on-unload()`.
  - `invoke-action(action-id, config-json, context-json)` : exécute une action ;
    à l'intérieur, le plugin appelle des services de modules via `modules.call`.
  - `on-settings-changed(settings-json)`.
  - `on-module-event(module-id, event-id, payload-json)` : un événement d'un
    module auquel le plugin s'est abonné. C'est là qu'un trigger « Scène OBS
    changée » se réalise : le plugin reçoit l'event du module obs, le transforme,
    et appelle `emit-trigger`.

### 4.3 Le MODULE (`world module`, dev certifié)

- **Importe** : `logging`, `storage`, `events` (`emit-event`), `oauth`
  (facilitation du core), **+ les capacités système** `net`, `input`, `audio`,
  `hid`, `files`, `process`, `timers`.
- **Exporte** (`module-guest`) :
  - `init() -> string` : manifeste JSON (id, name, version, **services** =
    méthodes + schémas, **events** + schémas, settings/secrets, **+ les capacités
    système requises**).
  - `on-load() / on-unload()` : ouvre/ferme les connexions (ex. `net.ws-connect`
    vers obs-websocket ; le module **répond** au serveur via `net.send(handle,
    bytes)` — le host garde le demi-socket d'écriture par handle, cf. le module
    `obs`).
  - `call(method, args-json) -> result<string>` : un plugin a appelé un service.
  - `on-settings-changed(settings-json)`.
  - `on-host-event(event-id, payload-json)` : donnée bas niveau d'une capacité
    que le module possède (message socket, tick timer, rapport HID, callback
    OAuth…). Le module la transforme en **event de service** via `emit-event`.

Un module porte aussi ses propres **settings/secrets + UI** (ex. le panneau de
connexion OBS : hôte, port, mot de passe). Différence clé avec un plugin : un
module n'expose **pas** d'actions/triggers — il expose des **services/events** ;
et un même module peut alimenter **plusieurs** plugins.

### 4.4 Câblage plugin ↔ module, et téléchargement à la demande

- Le manifeste d'un plugin déclare ses **modules requis** (`obs@^1`, …).
- À l'installation, le host **résout** ces dépendances et télécharge les modules
  certifiés manquants depuis le marketplace (avec écran de consentement des
  capacités système). **Aucun module n'est livré avec le cœur** : ce qui n'est
  pas requis n'est jamais téléchargé.
- À l'exécution, `modules.call` / `subscribe` est **routé et validé par le
  host** : il vérifie que le plugin a bien déclaré la dépendance, que la méthode/
  l'event existe, et valide les payloads contre les schémas de service publiés
  (même modèle JSON-sur-signatures-typées que les actions). Les `emit-event` du
  module sont diffusés aux plugins abonnés via `on-module-event`.
- Le host pourra plus tard **lier statiquement** les composants (import du plugin
  satisfait par l'export du module) comme optimisation — la couche dynamique
  reste le contrat de référence.
- **Comptage de références** : un module que plus aucun plugin n'utilise peut
  être désinstallé.

### 4.5 Certification

- **Plugins** : aucune certification requise. N'importe qui publie. La sandbox
  (pas de capacités système) est la garantie de sécurité.
- **Modules** : doivent être **signés** par l'autorité de certification Quicktys
  (programme « dev certifié » : revue + clé de signature). Le host **refuse**
  d'accorder une capacité système à un module non certifié. C'est ce qui permet
  d'ouvrir le réseau/clavier/audio à du code tiers sans exposer l'utilisateur à
  du code natif arbitraire.

### 4.6 Facilitations du core (ex. OAuth)

Le core peut **faciliter** des tâches transverses pénibles via des interfaces
host de plus haut niveau, sans pour autant porter la logique métier du module.
Première facilitation : **OAuth** (`interface oauth`, module-facing).

- Le **core** pilote tout le flow : ouverture du navigateur, capture du
  redirect, échange du code, **refresh transparent**, et **stockage sécurisé**
  des tokens (côté secrets du host). Le module n'écrit jamais la mécanique OAuth.
- Le **module** fournit uniquement l'**env** (`provider-config` : `auth-url`,
  `token-url`, `client-id`, `scopes`, `redirect`, PKCE…) — ces valeurs vivent
  dans le module (ou ses secrets), pas dans le core.
- À l'usage, le module appelle `oauth.access-token("twitch")` puis utilise le
  token via `net.fetch`. Décision : **la logique Twitch reste dans le module
  twitch**, le core ne fait que lui simplifier l'auth.

> Ce patron (« le core facilite, le module possède l'env et l'usage ») est le
> modèle à suivre pour d'autres tâches transverses futures.

## 4bis. Répartition core / modules (placement des fonctionnalités)

Décision (propriétaire) : qui possède quoi, entre le **cœur** (host Rust de
confiance) et les **modules** (drivers WASM certifiés).

**Cœur :**

- **Variables locales** d'une automatisation (portée par automatisation — les
  nœuds `Let`/`Set`, cf. `docs/automation-engine.md` §2.3).
- **Time info** : date/heure exposées dans le contexte d'expression.
- **Webserver** : le serveur HTTP local (sert les overlays, porte le transport
  des remote — cf. §12 et §13).

**Modules (certifiés) :** OBS, Twitch, Audio, Clavier, **variables non-locales de
types standard** (un module générique de globales), Overlay, Remote control.

**Deux règles de placement :**

1. **Capability ≠ module.** Les capacités host bas niveau (`audio`, `input`,
   `net`, `hid`, …) restent dans le host ; un module est le *driver* qui les
   **consomme** et expose des services. Ex. le module *Audio* consomme la cap
   `audio` ; *Clavier* la cap `input` ; *OBS*/*Twitch* la cap `net`.
2. **Les variables de domaine vivent dans le module du domaine**, pas dans le
   module générique de variables. Ex. des variables par-utilisateur du chat
   Twitch appartiennent au **module twitch**, pas au module *variable* (qui ne
   porte que des globales de types standard).

**Overlay & Remote — c'est le *driver* qui est un module** (décision) :

- **Overlay** : le module est le driver qui **sert/pilote** les overlays via le
  **webserver du cœur** ; le **contenu** d'un widget reste un **paquet frontend**
  (le 3ᵉ type d'extension, §12), pas du WASM.
- **Remote control** : le module est le driver qui expose les **surfaces** au
  matériel/clients (caps `hid`/`net`) ; le **webserver/relais** et la boucle
  `emit-trigger` restent un **service du cœur** (§13).

## 5. Système de schémas (`quicktys-schema`)

Réimplémentation clean-room du *concept* de schéma (filiation CastMate, code
neuf). Un schéma est une description auto-portée d'un
champ : type de base (String, Number, Boolean, Object, Array) + types UI riches
(Duration, Color, FilePath, Range, MediaFile, Resource-ref…), avec contraintes,
valeurs par défaut, et métadonnées d'affichage.

Trois usages :

1. **UI** : le frontend génère les formulaires à partir du schéma (inchangé vs
   CastMate).
2. **Validation** : tout payload JSON traversant l'ABI WASM est validé contre le
   schéma avant `invoke-action`.
3. **Sérialisation** : (dé)sérialisation YAML ↔ valeurs typées pour la
   persistance, round-trip-safe.

`quicktys-schema` héberge aussi le **modèle de nœuds** des automatisations (types
sérialisés, partagés avec l'UI) et le découpage des régions `{{ }}`.

## 5bis. Expressions & automatisations (`quicktys-expr` + runner)

Refonte majeure vs CastMate, détaillée dans
[`docs/automation-engine.md`](docs/automation-engine.md) :

- **Moteur d'expressions sandboxé (Rhai)** pour le **contrôle de flux** :
  conditions, filtres, activations, boucles, délais dynamiques, `let/set`. Évalué
  côté host, synchrone, déterministe, avec limites d'opérations/temps. Les
  expressions n'appellent qu'une **stdlib du cœur** (pas de fonctions injectées
  par les plugins → pas d'aller-retour ABI).
- **Templating des chaînes de config (décision) : deux syntaxes, séparées de
  Rhai.** `@nom.chemin` = **variable simple** (lookup à plat dans le contexte —
  état + base + locaux ; ex. `@pear.title`), valable seulement en début de chaîne
  ou après une espace ; un `@chemin` seul garde la valeur **typée**, sinon il
  interpole l'affichage. `{{ }}` = **Handlebars** (crate `handlebars`), les clés à
  points étant imbriquées en arbre pour que `{{pear.title}}` /
  `{{#each pear.playlist}}` marchent. Résolu par le runner
  (`resolve_config`) avant l'invocation d'action ; `@` d'abord, puis Handlebars.
  **Échappement** : `\` rend littéral le caractère suivant — `\@` / `\{{` ne sont
  pas interprétés, et `\\` donne un `\` (les `{`/`}`/`\` échappés passent par des
  marqueurs privés pour ne pas être ré-analysés par Handlebars, restaurés en fin).
  L'éditeur affiche les champs template avec un input surligné à autocomplétion
  `@` **et** `{{` (helpers), `TemplateInput.vue`, alimenté par les variables
  déclarées de chaque plugin (`PluginInfo.state`).
- **Analyse statique de l'AST** : suivi de dépendances (réactivité ciblée),
  validation et autocomplétion à l'édition.
- **Contrôle de flux complet** : variables locales, `if/switch` inline, boucles
  bornées, `try/retry`, `wait-for`, et `parallel` avec politique de jointure
  (**démarrer N branches et attendre que toutes finissent** avant de continuer —
  cas que CastMate ne gérait pas proprement). La **timeline** visuelle de
  CastMate est conservée comme un type de nœud.

## 6. Moteur du host (`quicktys-core`)

- **extensions** : registry des manifestes décodés (plugins **et** modules) ;
  mapping action-id → plugin, module-id → module ; résolution des dépendances
  plugin → modules ; métadonnées de confiance/certification.
- **bus** : event bus des triggers (les plugins émettent, les profils matchent)
  et fan-out des events de modules vers les plugins abonnés.
- **auth** : facilitation OAuth (`interface oauth`) — flow navigateur, échange
  de code, refresh transparent, stockage sécurisé des tokens. L'env du provider
  vient du module ; le core ne fait que la mécanique.
- **resources** : `Resource` / `FileResource` (Profiles, Automations,
  ActionQueues, Accounts…), registry + sync vers l'UI, persistance YAML.
- **profile** : profils, conditions d'activation (expressions booléennes), et
  **bindings trigger→séquence inline** (décision : un profil possède la séquence
  de chacun de ses triggers, façon CastMate — pas de référence à une automation
  par id). Le **`Dispatcher`** écoute le bus : sur un trigger, il fait tourner la
  séquence de chaque binding actif dont le filtre passe. Il est **câblé au boot**
  (`run_forever(bus)`) en partageant le store de profils de l'`AppBackend`
  (`Arc<RwLock<ResourceStore<Profile>>>` via `profiles_store`), pour qu'un profil
  édité dans l'UI dispatche à chaud. Côté déclenchement live, `quicktys-app`
  lance un **poller** qui appelle périodiquement le service `poll` des modules qui
  en déclarent un (ex. `pear` → `song-changed`) ; un vrai cap `timers` le
  remplacera.
- **sequence** : **runner d'automatisation** — interpréteur (tree-walking) sur le
  modèle de nœuds (`Action`, `Block`, `Parallel{join}`, `Timeline`, `If`,
  `Switch`, `Loop`, `Let/Set`, `Try`, `WaitFor`…), avec concurrence structurée,
  **abort** (Tokio `CancellationToken`) et boucles bornées. Conditions/valeurs via
  `quicktys-expr`. **Design complet** :
  [`docs/automation-engine.md`](docs/automation-engine.md).
- **queues** : files d'action (resource `ActionQueue`) — **conservées de
  CastMate**. Exécution séquentielle, `gap` configurable entre séquences,
  pause/reprise, skip, replay, historique. Essentiel pour que les alertes stream
  ne se chevauchent pas. Une automation peut cibler une file ou s'exécuter
  immédiatement.
- **reactivity** : refs réactives async côté host (équivalent du système TS).
  Quand un plugin appelle `set-state`, le host notifie l'UI et met à jour le
  contexte d'expression. Le **suivi de dépendances** des expressions (extrait de
  l'AST) permet une réévaluation ciblée.
- **template** : résolution `{{ … }}` (via `quicktys-expr`) sur `{ contexte du
  trigger + state plugins/modules + locales }`, puis coercition par schéma.
- **i18n** : registre des bundles de locales découverts (cœur + `lang/` de
  chaque extension), namespacés par id, poussés au frontend via l'IPC (cf. §10).
- **persist** : helpers YAML + disposition du répertoire utilisateur (voir §9).

## 7. IPC host ↔ WebView (`quicktys-ipc`)

Remplace `ipcMain.handle`/`ipcRenderer.invoke` (Electron) par :

- **commands** (UI → host) via `#[tauri::command]` : lister plugins/resources,
  éditer réglages, lancer une automation, CRUD resources, changer la locale…
- **events** (host → UI) via `Window::emit` : plugin enregistré/retiré, état mis
  à jour, resource ajoutée/modifiée/supprimée, progression de queue…

Le catalogue reprend 1:1 les canaux CastMate (`plugins_*`, `resources_*`,
`profiles_*`, `automations_*`, `queues_*`) pour minimiser les changements côté
composants Vue. La logique reste testable sans WebView ; `quicktys-app` ne fait
que brancher de fines coquilles Tauri.

**Télémétrie d'exécution (décision).** Le runner (`quicktys-core`) expose un
`ProgressSink` : pendant `run_with_progress`, il encadre chaque nœud de premier
niveau par `node([i], true/false)` — début/fin **réels** (donc durée réelle) au
fil de l'exécution. L'IPC relaie ça en événement `NodeProgress { automation,
path, running }`, et l'éditeur allume en direct la carte réellement en cours
(mapping `path[i]` → i-ᵉ nœud, l'ordre étant préservé à la sauvegarde). En mode
navigateur (sans host), l'éditeur retombe sur une estimation cliente des durées.
Extension future : `path` multi-niveaux pour les branches parallèles/timelines.

## 8. Frontend (WebView)

**Réécrit de zéro** (clean-room) sur la **stack** Vue 3 + PrimeVue + Pinia —
libs tierces indépendantes, libres d'usage. On **ne vendorise pas**
`castmate-ui-core` ni aucun composant CastMate ; on réimplémente la génération de
formulaires par schéma, l'éditeur, etc.

Le client parle au host via Tauri (`invoke`/`listen`). Le host envoie des
**manifestes** d'extensions (issus de `guest.init`) ; le frontend en déduit les
formulaires et l'UI.

**Slots UI extensibles par les plugins (décision).** Chaque endroit d'interface
extensible porte un **id de slot** (ex. `webview.toolbar`) et rend un
`<Slot id="…" :context="{…}"/>`. Un plugin déclare des **contributions UI** dans
son manifeste (`Manifest::ui(UiDecl)` → champ `ui`, traverse l'ABI JSON jusqu'à
`PluginManifest.ui`/`PluginInfo.ui`), sous deux formes : **bouton** (composant
natif — `label`/`icon`, au clic lance l'`action` du plugin avec le **contexte du
slot fusionné** dans la config) ou **custom** (`entry` = un fichier HTML livré par
le plugin, rendu dans une **iframe sandbox** `SlotFrame.vue` via le scheme
**`pluginui://<id>/<file>`** — même origine/CSP propre, comme les widgets, fed le
contexte par `postMessage`). `Slot.vue` agrège les contributions du slot depuis
`list_plugins`. Réf. : le plugin `obs` injecte un bouton **« Ajouter à OBS »** dans
`webview.toolbar` ; le slot passe `{url, name}` de l'overlay courant → l'action
`add-browser-source` → le module (qui suit la scène courante) envoie `CreateInput`
(browser_source). Le dossier `plugins/<id>/ui/` est copié dans le bundle par
`build-bundles.sh` et servi par `pluginui://` (`frame-src pluginui:` dans la CSP).

**Vues/onglets contribués par une extension (décision).** Au-delà d'un slot, un
**module** peut fournir une **page/onglet entière** de l'app. Il le déclare dans
son manifeste (`ModuleManifest::view(icon, label?, entry)` → champ `view`,
traverse l'ABI JSON jusqu'à `ModuleInfo.view`). L'app lit `list_modules`, ajoute
l'entrée à la **sidebar** (`Sidebar.vue`, id d'onglet `view:<moduleId>`) et rend
`entry` (un HTML du bundle) dans **`ViewFrame.vue`** — iframe sandbox servie par
le scheme **`moduleui://<id>/<file>`** (origine/CSP propre ; `frame-src
moduleui:`). Différence clé avec un slot : la page **pilote son propre module**
via un **bridge requête/réponse** `postMessage` **scopé** — `{type:"call",reqId,
service,args}` → l'hôte fait `call_module(<ce module>, …)` et répond
`{type:"result",reqId,ok,value}` ; la page **ne peut appeler que son module**
(jamais un autre). Réf. : le **module `webview`** contribue l'onglet **Overlays**
(éditeur d'overlays, `modules/webview/ui/index.html`) — il n'est plus codé en dur
dans l'app. Le dossier `modules/<id>/ui/` est copié à la racine du bundle par
`build-bundles.sh` (`cp ui/. dest/`), donc `entry = "index.html"`.

**Éditeur d'automatisation (UX, décision) : hybride timeline + blocs.** On garde
la **timeline visuelle** de CastMate pour le séquentiel/temporel (pistes, drag
& drop), et on y imbrique des **blocs structurels** pour le nouveau contrôle de
flux (`if`, `loop`, `try`, `parallel`). Familier pour les utilisateurs CastMate,
tout en exposant la puissance du nouveau modèle de nœuds.

**Plateformes (décision) : Windows / macOS / Linux dès le départ** (Tauri est
cross-platform), avec **priorité Linux** pour le test/polissage (plateforme de
dév). Les spécificités OS (injection clavier, audio, HID) sont absorbées par les
capacités host, pas par les extensions.

## 9. Persistance

Format **YAML**, compatibilité visée avec les fichiers CastMate :

```
<user-data>/quicktys/
├── settings/      profiles/      automations/
├── secrets/       queues/        accounts/
├── state/         storage/       logs/
├── installed/     # plugins/modules/widgets installés (materialisés par le marketplace)
├── installed.json # lockfile d'installation (versions, hashes, ref counts)
└── registry-cache/# index/CRL/blobs téléchargés (M6.6)
```

**Install runtime — décision (M6.2, cf. [docs/marketplace.md](docs/marketplace.md)).**
Le moteur d'installation est une crate **host-agnostique `quicktys-install`** qui ne
fait que *matérialiser* un `.qtpkg` (crate `quicktys-pkg`) : vérif du `content_hash`,
extraction **atomique** (`.tmp/ → rename`) dans `installed/<kind>s/<id>/` — la forme
exacte que le boot scanne déjà — et écriture d'un `bundle.json` compat depuis le
`manifest.json` du paquet (les **widgets** gardent leur `bundle.json` authoré, plus
riche). Le **lockfile `installed.json`** est la source de vérité (versions, hashes,
`source`, et `required_by` = **comptage de références** pour repérer les modules
orphelins). Le **hot-load** (charger dans le host en cours sans redémarrage) reste
côté app via un bridge **`HostLoader`** injecté dans `AppBackend` (même patron que
`module_caller`) : `quicktys-ipc` reste sans dépendance au host wasm. Confiance
**hash-only** pour l'instant (`certified: false`) → un module sideloadé qui déclare
une cap est refusé au load (signature = M6.5). `AppBackend` expose
`sideload_package`/`uninstall_package`/`list_installed` et émet `ExtensionsChanged`.
L'**unload** WASM à chaud (le host ne le supporte pas encore) et la consommation de
l'event côté frontend viendront ensuite.

**Signature & certification — décision (M6.5, cf. [docs/marketplace.md](docs/marketplace.md) §2 + [docs/drm.md](docs/drm.md)).**
La provenance est vérifiée par la crate **`quicktys-cert`** (sur `quicktys-crypto`,
suite hybride PQ) : une chaîne **CA → certificat d'éditeur → release**, chaque
maillon signé (Ed25519+ML-DSA). Un `TrustStore` de clés **CA épinglées** + une CRL
donnent un `Verdict` (Certified / Unsigned / Rejected). **`certified` est calculé
par crypto** à l'install (`quicktys-install` vérifie le `signature.json` du paquet :
chaîne valide + `manifest_hash` signé + `caps ⊆ caps_ceiling` + non expiré + non
révoqué), **plus lu comme un simple booléen** ; une signature invalide **refuse**
l'install, un paquet non signé reste **non certifié** (sûr : ses caps système sont
gatées au load). Le `content_hash` d'archive complète n'est **pas** signé (le
`signature.json` est *dans* l'archive → circularité) : la signature engage le
**`manifest_hash`**, et le manifeste couvre le payload via ses `hashes` par fichier.
Enfin, `ExtensionRegistry::may_grant` (certifié **et** capacité déclarée) est
désormais **appliqué à chaque dispatch réel** de cap système (net/input/audio/
process) dans le host, pas seulement au chargement — défense en profondeur et
amorce du consentement par-cap (M6.7). `TrustStore` vide par défaut (aucune
opération CA encore) → les bundles de dev restent de confiance par localité.

**Registre marketplace — décision (M6.6, cf. [docs/marketplace.md](docs/marketplace.md) §4).**
Le registre est **statique** (`index.json` signé + `crl.json` + blobs `.qtpkg`
content-addressés, servables depuis un bucket/CDN, auto-hébergeable). La crate
**`quicktys-registry`** est **transport-agnostique** : le HTTP est injecté via un
trait **`Fetcher`** (l'app le back avec `WasmHost::http_get`), donc le moteur est
testable offline et ne dépend jamais du host wasm. Elle **fetch+cache** l'index
(vérif de signature contre des clés registre épinglées si présentes), **recherche**,
**résout les dépendances** (semver, **post-ordre** deps-first, garde anti-cycle : un
plugin tire les modules certifiés qu'il requiert), et **télécharge+vérifie** chaque
blob par `content_hash` — l'installation elle-même reste `quicktys-install` (qui
vérifie la signature de chaque paquet, M6.5). La CRL cachée par le refresh est
fusionnée au `TrustStore` à chaque install (les révocations prennent effet).
`AppBackend` expose `marketplace_refresh/search`, `install_from_registry` (résout →
download → install → **hot-load ordonné**, modules avant plugins) et `list_updates`.
Les modules tirés comme dépendances sont marqués **non explicites** (→ purgeables
en orphelins, §9). URLs de registre configurables (`QUICKTYS_REGISTRY`).

**Consentement des capacités — décision (M6.7, cf. [docs/drm.md](docs/drm.md) §7).**
La certification décide qu'un module *peut* toucher une cap ; le **consentement**
décide que l'*utilisateur* l'a autorisée. Les deux se composent à chaque dispatch :
`accordé = certifié && déclaré && consenti`. Le `ConsentStore` persistant
(`<data>/consent.json`, dans `quicktys-install`) suit le modèle **grâce jusqu'à
revue** (un module non encore revu est autorisé, pour ne rien casser avant que
l'utilisateur ouvre l'écran de consentement) puis **strict** (seules les caps
`granted`). Révoquer une cap prend effet **immédiatement** ; une mise à jour qui
**élargit** les caps voit les nouvelles **refusées** jusqu'à re-revue
(`needs_review`, comparaison de version). L'enforcement est côté host : un
**sink de consentement** `(module_id, cap) -> autorisé` (`WasmHost::set_consent`)
est consulté à chaque dispatch, en plus de `may_grant`. L'app back le sink par un
`Arc<RwLock<ConsentStore>>` partagé, que `AppBackend` mute via
`grant_capabilities`/`revoke_capability` (persisté, event `ConsentChanged`) et
expose via `consent_status` (déclaré / accordé / en attente) pour l'écran de
consentement. Sink absent → non enforced (`may_grant` seul), donc les bundles de
dev restent fonctionnels.

**Entitlements & payload chiffré — décision (M6.8/M6.9, cf. [docs/drm.md](docs/drm.md)).**
Une **licence** (`quicktys-cert::LicenseKey`, chaîne `QTLIC-…`) est une
`Entitlement` **signée par le store** (hybride PQ), vérifiée **hors-ligne** contre
des `store_keys` épinglées (`QUICKTYS_STORE`). L'`EntitlementStore`
(`<data>/entitlements.json`) garde les licences saisies (`enter_license_key`).
L'install **refuse** un paquet `paid`/`subscription` sans licence couvrante (gate
dans `quicktys-install`). Le **payload chiffré** (T1) : seul le `.wasm` est chiffré
(`<wasm>.enc`, XChaCha20-Poly1305), `manifest.protection` le déclare ; la **content
key** est livrée **hors-ligne** par le `key_material` de la licence (drm §A.8, voie
sans compte ni serveur). Le **décrypteur vit au chargement** : `load_component`
(app) détecte `<wasm>.enc`, dérive la content key d'une licence couvrante,
déchiffre en mémoire et charge via `WasmHost::load_*_bytes`
(`Component::from_binary`) — **au boot comme au hot-load**, jamais réécrit en clair
sur disque. Provenance (signature) et confidentialité (chiffrement) restent deux
couches indépendantes : un paquet premium est **signé en clair** (catalogable,
`certified` calculable) **et** chiffré. Outil dev : `quicktys-pack --paid`/
`--encrypt`/`--issue-license`. Reste à faire : le **service d'activation HPKE en
ligne** (binding appareil via keystore OS, seats, grâce d'abonnement) — la crypto
`seal`/`open` hybride est déjà dans `quicktys-crypto`.

**Watermark forensique — décision (M6.10, drm.md §5).** Un **id opaque par
acheteur** (`quicktys-crypto::watermark::derive_id`, sha256(seed‖contexte)
tronqué, non-PII) est **tissé invisiblement** (caractères zero-width, répété pour
survivre à la troncature) dans les **assets texte** d'un paquet marqué
`watermark:true` — à **l'install** (marque per-livraison), **après** la vérif de
provenance (la marque ne peut pas faire partie des octets signés) et **jamais**
sur les `*.json` (casserait signature/manifest). Le seed vient de l'entitlement
couvrant (compte/sku) ou `anonymous`. Une fuite → `extract` récupère l'id →
(mapping id→acheteur **côté serveur**, vie privée). C'est **dissuasif**, pas
inviolable (le design l'assume : un watermark client se retire). Les tiers
**T2 server-gated** (logique critique exécutée côté serveur) et le **revenue
share** (comptable côté serveur) relèvent de l'infra serveur — hors périmètre du
client, conçus dans drm.md.

### 9.1 Bundles d'extension (empaquetage & chargement) — décision

Une extension n'est **pas** chargée depuis un `.wasm` brut posé à un chemin de dev
en dur, mais depuis un **bundle** : un dossier auto-descriptif regroupant le
composant, ses locales et un descripteur. `scripts/build-bundles.sh` compile
chaque extension (`wasm32-wasip2 --release`) et produit :

```
dist/bundles/                        # dev  (gitignoré, artefact de build)
├── modules/<id>/
│   ├── <id>.wasm                    # le Component
│   ├── lang/                        # locales (données, cf. §10)
│   └── bundle.json                  # { id, kind:"module", wasm, certified }
├── plugins/<id>/{<id>.wasm, lang/, bundle.json}   # bundle.json.kind = "plugin"
└── widgets/<id>/{bundle.json, lang/}              # kind:"widget" — PAS de wasm
```

Le host (`quicktys-app`) scanne deux racines — `dist/bundles` (dev) puis
`<user-data>/quicktys/installed/` (marketplace) — charge d'abord les **modules**
(drivers) puis les **plugins** (qui peuvent en dépendre), et pour chaque bundle :
lit `bundle.json`, charge le `.wasm` (les modules honorent le flag `certified` →
gating §4.5), et découvre son `lang/` namespacé par l'id. Un id déjà chargé est
ignoré, donc un bundle installé peut masquer un bundle de dev de même id. Aucun
chemin `target/…` n'apparaît plus dans le code de boot.

**Widgets (décision).** Les widgets sont le 3ᵉ type d'extension, **frontend-only
(pas de WASM)** : leur bundle est **authored** (pas compilé) — `bundle.json`
(`kind:"widget"`, `name`, `icon`, `template`, `config` = schéma des réglages) +
`lang/`. Le host ne les **exécute pas** ; il les **catalogue** (scan des dossiers
`widgets/`, lang namespacé `widget.<id>.*`) et expose les manifestes à l'UI via
`list_widgets`. Deux natures : **built-in** (rendu par un composant in-app choisi
par `template`, ex. `info` → `InfoWidget`) et **runtime** (le bundle embarque un
`index.html` que le host inclut dans le manifeste ; le frontend le rend dans une
**iframe sandbox** `WidgetFrame.vue`, avec un pont `postMessage` état↔actions —
une action = une séquence d'un nœud via `run_sequence`). Le pont `postMessage`
transmet aussi la **config d'instance** au widget (`{type:"config", config}`),
donc un widget runtime est **personnalisable** par ses réglages (ex.
`widgets/pear-music` : cases pochette/titre/artistes/barre de temps + couleurs +
arrondis). Le **formulaire de config** est généré depuis le schéma du widget par
`WidgetConfigForm.vue` (types reconnus : `Boolean`→switch, `Color`→sélecteur,
`Number[min,max]`→slider, `enum`→select) — partagé Dashboard + éditeur d'overlays.
**Preview d'éditeur (décision).** Un widget qui est **inerte au repos** (une alerte
n'apparaît qu'à l'event) peut livrer une **page de preview séparée** :
`bundle.json` déclare `"preview": "preview.html"`, un fichier **distinct** de
`index.html`. Dans l'éditeur webview seulement, `WidgetFrame` (prop `preview`)
charge ce `preview.html` **à la place** de `index.html` (natif : via le scheme
`widget://…/preview.html` ; mock : `previewHtml` embarqué dans le manifeste). Il
reçoit la **même config** par le pont `postMessage`, donc il se met en scène avec
des données placeholder tout en respectant les réglages. La **source navigateur
(OBS)** charge toujours `index.html` — le vrai widget, idle jusqu'à l'event : ainsi
`index.html` reste « basique » et la démo vit dans un fichier à part. Réf.
`widgets/alert`.
Réf. runtime : `widgets/pear-now-playing`, `widgets/pear-music`, `widgets/pear-queue`.
Le **Dashboard** est une grille 5×5 à placement libre (position `x,y` + taille
`w,h`, drag/resize snappés), persistée côté frontend (localStorage) — cf.
`DashboardView.vue` / `stores/dashboard.js`.

**Alerts — widget piloté par action (décision).** Motif « une action réveille un
widget » : le plugin `alert` expose l'action `show {name, title, content, image}`
qui pousse `{title, content, image, fired}` dans l'état `alert.<name>` via
`reactivity.set-state` ; le widget `alert` (config `name` + style), abonné à
`alert.<name>`, s'anime à chaque nouveau `fired` (invisible au repos → OK en
browser-source). Le champ `name` de l'action **s'autocomplète** avec les noms des
widgets alert placés : c'est le hint de schéma **`suggest: "<source>"`** (nouveau
champ `Common.suggest` dans `quicktys-schema`) — l'UI (`SchemaForm.vue`) rend un
`<datalist>` alimenté par un résolveur injecté (`field-suggestions`), fourni par
`App.vue` en scannant les widgets alert du dashboard + des overlays. `title`/
`content`/`image` sont des templates (`@`/`{{ }}`). Le même chemin d'état marche
en app (postMessage) et en OBS (SSE), donc l'alerte s'affiche des deux côtés.
Le hint `suggest` est **générique** : les actions OBS l'utilisent aussi
(`suggest:"obs-scene"` / `"obs-input"`), `App.vue` alimentant le résolveur depuis
les variables réactives `obs.scenes` / `obs.inputs`.

**Modules event-driven avec round-trip (décision, obs).** Le module `obs` illustre
un driver WebSocket bidirectionnel : il **envoie** des requêtes obs-websocket op 6
via `net.send` et **corrèle** les réponses op 7 (par `requestType`). Deux flux
distincts : les **events** OBS → events de module qui déclenchent des **triggers**
(`scene-changed`, `stream-started`, `media-ended`, `replay-saved`…) ; les
**réponses op 7** (sync sur `Identified` + re-poll live) → un event de module
**`state`** (bulk `{clé: valeur}`) que le plugin **mirroir** dans `obs.*` **sans**
trigger. D'où des variables riches (scène + liste de scènes/sources, statut
stream/record avec timecode/durée, profil, collection, résolution,
FPS/CPU/mémoire/frames…) correctes dès la connexion, et l'autocomplétion des
scènes/sources ci-dessus. Les **actions à round-trip** (visibilité/transform
d'une source) enchaînent en interne : le service module `resolve_scene_item`
envoie `GetSceneItemId` avec un requestId mémorisé dans une map `PENDING`, et la
réponse op 7 déclenche le suivi (`SetSceneItemEnabled`/`SetSceneItemTransform`)
avec le `sceneItemId` résolu — invisible pour le plugin/l'utilisateur.

**Overlays = un module `webview` (décision, révisée).** Le système d'overlays est
un **module** (`modules/webview`, non certifié — que la cap `storage`), pas une
ressource du cœur. Le module **possède les données** : `list/get/upsert/delete-
overlay` sur une map `id → overlay` stockée dans le `storage` (clé `overlays`).
Le frontend édite via `call_module("webview", …)` (`WebviewView.vue`), et le
**serveur HTTP du cœur** (`quicktys-app::webserver`, qui bind le port — une
facilité système) **interroge le module** sur `GET /overlay/<id>` (`get-overlay`)
et `/` (`list-overlays`) pour générer la page + le flux SSE. Le cœur ne connaît
plus les overlays (le type `Overlay` et `ResourceKind::Webview` ont été retirés) ;
c'est l'application du principe §12 « le module est le driver qui sert/pilote les
overlays via le webserver du cœur ». Un overlay reste `{ id, name, aspect
16:9/9:16, widgets[] }` avec positions/tailles **en pourcentage** (rendu identique
à toute résolution). Comme les appels module n'émettent pas d'event de ressource,
l'éditeur rappelle `provide("refresh-overlays")` après mutation (noms d'alertes +
compteurs). **Stockage persistant (décision liée) :** la cap `storage`
(`kv_get`/`kv_set`) persiste désormais sur disque, par extension
(`<data>/storage/<id>.json`, câblé par `WasmHost::set_storage_dir`) — indispensable
pour qu'un module propriétaire de données (comme `webview`) survive au redémarrage.

## 10. i18n (traductions)

Modèle **décentralisé** : **chaque « chose » possède son dossier `lang/`** — le
cœur, chaque module, chaque plugin. C'est le changement majeur vs CastMate.

### 10.1 Le dossier `lang/`

```
lang/                       → cœur / app-shell   → namespace  core.*
modules/<id>/lang/          → un module          → namespace  module.<id>.*
plugins/<id>/lang/          → un plugin          → namespace  plugin.<id>.*
```

Chaque dossier contient un JSON par locale (`en.json`, `fr.json`, `de.json`,
`es.json`, `pt.json` au départ), reflétant la structure des clés du manifeste
(`actions.<id>.name`, `settings.<id>.label`, `services.<id>.name`, …). `en.json`
est la locale de référence ; les clés manquantes retombent dessus.

### 10.2 Fonctionnement

- **Donnée, pas binaire** : le `lang/` voyage dans le paquet de l'extension,
  **à côté** du `.wasm` — il n'est jamais compilé dans le composant. On peut donc
  l'éditer, le surcharger, et la communauté peut contribir des traductions sans
  toucher au code.
- **Manifeste = clés** : un plugin/module n'émet que des **clés** via son `init`,
  jamais de chaînes traduites. Le host n'émet jamais de texte traduit non plus.
- **Découverte par le host** : au chargement d'une extension, le host lit son
  `lang/`, **namespace** les bundles par l'id (`plugin.time.*`, `module.obs.*`)
  et les pousse au frontend via l'IPC, en même temps que le manifeste. Le bundle
  du cœur (`lang/`) est chargé directement par le frontend.
- **UI** : résolution via `vue-i18n` (`$t("plugin.time.actions.delay.name")`),
  fallback anglais. `vue-i18n` est une lib tierce ; l'intégration est **écrite de
  zéro** (clean-room), alimentée par ce mécanisme décentralisé.

### 10.3 Différences vs CastMate

| CastMate | Quicktys |
| --- | --- |
| Locales **uniquement** pour les plugins | `lang/` pour le **cœur, les modules ET les plugins** |
| `registerPluginLocales()` appelé dans le renderer | **découverte automatique** du `lang/` par le host |
| Locales **importées** dans le bundle JS | Locales = **données** à côté du `.wasm`, surchargeables |
| Pas de notion de module | Les modules (UI de réglages) ont leur propre `lang/` |

> Localisation côté runtime : si un jour un plugin doit produire du texte
> localisé à l'exécution (pas seulement de l'UI), on ajoutera une capability host
> `i18n.translate(key)`. Par défaut, la traduction reste côté frontend.

## 11. Marketplace (conception cible)

> Design détaillé (formats `.qtpkg`, registre signé, chaîne de signature
> CA→éditeur→release, résolution de dépendances, install à chaud, IPC, jalons) :
> [docs/marketplace.md](docs/marketplace.md). Cette section garde les décisions de
> haut niveau ; le doc dédié détaille le système et les écarts de code à combler.

Le marketplace distribue **quatre** types d'objets :

- **Plugins** (tout le monde) — `.wasm` + manifeste + locales + UI optionnelle.
  Pas de signature de certification requise ; la sandbox suffit.
- **Modules** (devs certifiés) — `.wasm` + manifeste **signé par l'autorité de
  certification** + locales + UI de réglages. Seuls ceux-ci peuvent demander des
  capacités système.
- **Widgets** (tout le monde) — overlays/dashboards : composants UI + schéma +
  locales, **sans composant WASM** ; le JS tourne isolé dans la browser source.
- **Contenus** (automations, presets) — paquets de données validés par schéma,
  **sans code** → aucun risque d'exécution.

Parcours d'installation :

1. Téléchargement → si module, **vérification de signature** (refus si non
   certifié).
2. **Résolution de dépendances** : un plugin tire les modules certifiés qu'il
   requiert (`obs@^1`) ; rien d'inutile n'est téléchargé.
3. **Écran de consentement** : capacités système demandées par les modules
   (« accès réseau », « injection clavier »…), accordées par l'utilisateur.
4. Placement dans `installed/` → chargement à chaud par `quicktys-host`.

**Isolation** : chaque extension WASM tourne dans son propre `Store` wasmtime
avec ses seules capabilities. Un plugin malveillant ne peut ni lire le disque, ni
ouvrir un socket, ni accéder aux secrets non accordés — il ne peut agir que via
les services des modules installés, eux-mêmes médiés par le host.

### 11.1 Économie & licence (décision)

L'infra est conçue **dès le départ** pour supporter le **gratuit ET le payant**,
même si le lancement peut se faire gratuit :

- chaque objet porte un **modèle de licence** (gratuit / payant / abonnement) ;
- **entitlements** : le host vérifie le droit d'usage (achat/licence) avant
  installation/chargement ; clés de licence vérifiables hors-ligne ;
- **revenue share** possible pour les auteurs (plugins, modules **certifiés** —
  d'où l'intérêt de la certification — ou widgets) ;
- les modules **fermés/payants** sont autorisés (le `.wasm` reste sandboxé, donc
  un module fermé reste sûr pour l'utilisateur) ;
- **DRM intégré** (protection du payant) — activation, chiffrement du payload,
  binding machine, watermark forensique, révocation : design dans
  [docs/drm.md](docs/drm.md). Contrainte AGPL : le DRM protège les **objets vendus**
  (séparément licenciés), **jamais le cœur** ; le décrypteur est open (Kerckhoffs),
  le secret est dans les **clés**. Un DRM client parfait n'existe pas — l'objectif
  est de protéger la majorité honnête et de **tracer/révoquer**, pas l'inviolable.

> Cœur sous **AGPL-3.0 + exception d'extension + CLA** (cf. §0bis) : le cœur reste
> open source et un marketplace dérivé doit l'être aussi, mais les **extensions**
> (plugins/modules/widgets), couvertes par l'exception, peuvent être **fermées et
> payantes**. C'est le contenu du marketplace qui peut être gratuit ou payant.

## 12. Overlays / Widgets (3ᵉ type d'extension)

Un **widget** est un overlay/dashboard de **présentation**, affiché dans une
browser source OBS ou une fenêtre. Décision : c'est un **type d'extension à part
entière**, symétrique des plugins/modules pour le marketplace, mais **frontend**
(pas de composant WASM).

- **Paquet** : composants UI + **schéma de config** (génère le formulaire
  d'édition du widget, cf. `WidgetConfigForm.vue` — cases/couleurs/sliders/select)
  + dossier `lang/` (namespace `widget.<id>.*`). Un widget runtime reçoit sa
  **config d'instance** par le pont `postMessage` (`{type:"config"}`), donc il est
  **personnalisable** (ex. `pear-music` : éléments affichés, couleurs, arrondis).
- **Service** : le **serveur HTTP local** du host (`quicktys-app::webserver`, port
  `8787`) sert la page d'overlay (`/overlay/<id>`) et les assets des widgets
  (`/widget/<id>/…`), et pousse le **state** en temps réel via **SSE** (`/events`,
  snapshot initial puis un event par clé changée) — le widget consomme le même
  pont `postMessage` que dans l'app, donc rendu identique en app et en OBS. Une
  action postée par un widget revient en `POST /action` (séquence d'un nœud).
  Transport unidirectionnel (host → navigateur) ; SSE plutôt que websocket car
  suffisant et sans handshake. Édition WYSIWYG dans l'app, comme CastMate.
- **Sécurité** : le JS du widget tourne **isolé dans la browser source** (bac à
  sable navigateur), sans accès au host au-delà du flux de state autorisé. Un
  widget tiers ne met donc pas la machine en danger.
- **Données** : un **overlay** appartient au **module `webview`** (cf. §9 —
  `list/get/upsert/delete-overlay`, stockage persistant), édité par
  `WebviewView.vue` via `call_module` : widgets **runtime** placés **librement en
  pourcentage** (`x,y,w,h`) sur un canvas à **ratio fixe** (aperçu 16:9 ou 9:16),
  pour un rendu identique à toute résolution. Le serveur du cœur interroge le
  module pour servir `/overlay/<id>`. Le compteur d'overlays remonte dans le
  widget `info` du dashboard (source `webview`).

## 13. Boutons (widget `button`)

**Décision (révision).** Un protocole de surface remote générique (deck web +
appairage + Stream Deck via module certifié `hid`) avait été implémenté puis
**retiré** : trop de plomberie (module + plugin + serveur `/deck` + token + HID)
pour la valeur. Remplacé par un simple **widget `button`** sur le tableau de bord
— le modèle event→profil reste identique, la surface est juste un widget.

- Le **widget `button`** (built-in, `template:"button"`, config `label/icon/
  color`) rend un gros bouton cliquable sur le dashboard.
- **Clic → event.** En mode normal, cliquer **émet `button.clicked`** sur le
  `TriggerBus` via la commande host **`emit_trigger`** (`AppBackend::emit_trigger`),
  contexte `{ id, label }` (l'`id` = id d'instance du widget dans le layout).
  **Aucun WASM** dans la boucle — c'est le host qui publie directement sur le bus.
- **Bind.** `button.clicked` est un **trigger built-in** listé dans le picker des
  Profils (groupe « Boutons »). L'outil Édition **« Lier une action »** du widget
  crée/ouvre un profil `button-<id>` avec le trigger déjà filtré `id == "<id>"`.
- **Boucle** : clic → `emit_trigger` → `TriggerBus` → `Dispatcher` → la séquence
  du profil lié s'exécute — exactement comme un trigger de plugin.
- La capacité **`hid`** du host est **conservée** (infra générique, déclarée par
  le monde `module`), mais **plus aucune extension livrée ne l'utilise** ; un
  futur driver matériel (Stream Deck/Companion) pourra la réutiliser, ou une
  surface distante réémerger comme widget runtime servi par l'overlay-server.

## 14. Stratégie de migration (best-effort)

Décision : un **outil de migration unidirectionnel** CastMate → Quicktys,
best-effort :

- import des **profils, automatisations, réglages** YAML existants ;
- **mapping du modèle** d'automatisation (instant/time/stack/flow →
  arbre de nœuds, cf. [`docs/automation-engine.md`](docs/automation-engine.md)
  §2.7) ;
- **codemod des templates** JS → expressions Rhai pour les cas courants ;
- ce qui n'est pas convertible automatiquement est **signalé** à l'utilisateur
  (rapport de migration), pas silencieusement perdu.

Approche globale **incrémentale**, pas big-bang. Voir
[`docs/ROADMAP.md`](docs/ROADMAP.md). Idée clé : valider l'ABI WASM de bout en
bout sur un plugin simple (`time`) avant de porter les ~30 plugins ; les modules
« réseau lourd » (twitch, obs) valideront le modèle de capacités système.

**Implémentation (M8).** L'outil est la crate host-agnostique **`quicktys-migrate`**
(lib + bin `quicktys-migrate <dir-données-CastMate> [--out <dir>]`). Elle lit
**les données de l'utilisateur** (ses fichiers YAML — autorisé clean-room : donnée,
pas code) et écrit des ressources **Quicktys valides par construction** (elles
round-trippent dans les vrais types `Profile`/`Automation`/`Node` de
`quicktys-core`/`quicktys-schema`) plus un **rapport Markdown**. Choix :
extraction **défensive** (plusieurs orthographes de champs tolérées, cf. les
`map_*`), et tout ce qui est ambigu (`activation`/`filter` CastMate, expressions
`{{ }}` non triviales, actions non reconnues) est **rapporté, jamais deviné** —
en particulier **aucune traduction automatique JS → Rhai** n'est tentée (elle
serait non fiable) : les conditions/filtres sont laissés à ré-écrire à la main,
signalés dans le rapport. Le **codemod de templates** est limité aux cas sûrs :
`{{ }}` reste du Handlebars, et un `{{ chemin.simple }}` **seul** devient
`@chemin.simple` (valeur typée, cf. §5bis). Les mappings de champs précis sont à
affiner contre de vraies données ; le rapport est le point d'entrée pour ça.

## 15. Cloud & comptes (local-first)

Décision : **local-first, compte optionnel**.

- L'app fonctionne **100 % hors-ligne, sans compte** : tout (profils,
  automatisations, secrets, state) vit en local en YAML. C'est le mode par défaut
  et complet.
- Un **compte est optionnel** et débloque des fonctions cloud :
  - **sync multi-machine** des configurations,
  - **marketplace payant** : achat de plugins/modules/widgets et **entitlements**
    (licences **vérifiables hors-ligne** une fois acquises → l'app marche sans
    réseau),
  - **relais remote cloud** pour piloter depuis l'extérieur du LAN (cf. §13).
- Aucune fonctionnalité **locale** n'est verrouillée derrière le compte. Pas de
  télémétrie obligatoire.

## 16. Risques & questions ouvertes

- **Rédaction juridique de la licence** (modèle décidé en §0bis : AGPL-3.0 +
  exception d'extension + CLA) : reste à **écrire le texte** de l'exception
  d'extension (définir précisément « via l'ABI officielle »), le CLA, et décider
  l'option Commons Clause. Risque : si la frontière d'extension est mal définie,
  un plugin pourrait être requalifié en dérivé (ou l'inverse).
- **Frontière clean-room** : process pour garantir qu'aucun code CastMate ne
  fuit (revue, éventuellement implémenteurs n'ayant pas lu le code source de
  CastMate pour les parties sensibles).
- **Réactivité async en Rust** (**décidé & implémenté**) : bâtie sur
  **`tokio::sync`** (`broadcast` des clés changées, `RwLock` de l'état) plutôt
  qu'un framework de signaux *auto-tracking* (futures-signals/leptos) —
  inadapté ici car notre suivi de dépendances est **explicite**, extrait de
  l'AST Rhai (`CompiledExpr::deps`). Le `StateStore` (`quicktys-core::reactivity`)
  notifie par clé ; `wait_until` ne réévalue que si une clé de `deps` change
  (réévaluation ciblée). Pas de cycles : le graphe de deps est porté par les
  expressions, pas par des signaux qui s'observent mutuellement.
- **Granularité des capabilities** : assez fine pour la sécurité, assez simple
  pour les auteurs de modules.
- **Frontière module/plugin** : tranchée par défaut → la logique d'intégration
  (protocole, auth) vit dans le **module**, le core ne fournit que des
  facilitations (cf. §4.6 OAuth). À reconfirmer cas par cas si besoin.
- **100 % WASM, pas d'échappatoire natif** (décidé) : aucun module natif
  hors-process. Si un matériel exotique dépasse ce que les capacités host
  exposent, on **étend les capacités host** (en Rust, dans la zone de confiance)
  plutôt que d'ouvrir une brèche dans la sandbox.
- **Infra de certification** : autorité de signature, processus de revue,
  révocation de clés compromises, mises à jour de modules signés.
- **Performance de l'ABI JSON** : actions/services à haute fréquence ; prévoir un
  chemin binaire (ou linking statique de composants) si nécessaire.
- **Maturité Component Model** : versions wasmtime/wit-bindgen à épingler et
  suivre.
- **Compat YAML** : jusqu'où garantir l'import des fichiers CastMate existants.
```

> Ce document évoluera avec le code. Toute décision structurante s'y ajoute.
