# Roadmap Quicktys

Migration **incrémentale** de CastMate. Chaque jalon est livrable et testable.

## M0 — Scaffolding & architecture ✅

- [x] Arborescence du repo, workspace Cargo, crates stubs.
- [x] Contrat d'extension WIT à deux étages (`wit/host.wit` + `wit/world.wit`).
- [x] Document d'architecture (`ARCHITECTURE.md`).
- [x] **Versions épinglées exactement** (`=`) pour des builds reproductibles :
      `wasmtime`/`wasmtime-wasi` `=27.0.0`, `tauri` `=2.11.5`, `tauri-build`
      `=2.6.3` (workspace + `quicktys-app`). Le host génère ses bindings avec le
      `bindgen!` Component-Model de **wasmtime** (il n'utilise PAS la crate
      `wit-bindgen`) ; le générateur d'ABI **guest** `wit-bindgen` est pinné
      `=0.36.0` dans le manifeste de **chaque extension** (`plugins/*`,
      `modules/*` — paquets séparés hors workspace). La ligne `wit-bindgen`
      inerte du workspace a été retirée (aucun crate ne la référençait ; le graphe
      host tire de toute façon une 0.57.x transitive).
- [x] **`cargo check --workspace` vert** sur les 16 crates host (la porte
      d'acceptation M0) ; `Cargo.lock` inchangé par l'épinglage.

## M1 — Noyau host

- [x] `quicktys-schema` : `Schema` (types de base + UI riches), validation,
      (dé)sérialisation YAML/JSON, régions de template, modèle de nœuds `Node`.
      (`Expr` stocké en source ; types riches restants à ajouter au fil de l'eau.)
- [x] `quicktys-expr` : moteur Rhai sandboxé (limites op/profondeur/tailles,
      `eval`/modules désactivés), compilation + analyse statique (deps via walk
      AST), coercition par schéma, rendu de templates + `resolve_field` bout-en-
      bout. → templating `{{ }}` fonctionnel. (stdlib = défauts Rhai ; helpers
      date/durée dédiés à ajouter au fil de l'eau.)
- [x] `quicktys-core` : `Resource` + `ResourceStore<T>` (CRUD mémoire mirroré en
      `<id>.yaml` par répertoire, ids sanitizés), `DataDir` (layout §9) + helpers
      YAML load/save (écriture atomique temp+rename).
- [x] Runner d'automatisation : interpréteur tree-walking sur **tous** les nœuds
      M1 (Action/Block/Parallel{join}/Timeline/If/Switch/Loop/Let/Set/Try/
      WaitFor-Delay/Return/Break/Continue), abort coopératif (token `watch`),
      concurrence structurée (`join_all`/`select_all`, perdants droppés), boucles
      bornées, config templatée résolue au run, `ActionInvoker` mocké en test.
      Voir `docs/automation-engine.md`. (Restent : `WaitFor::Until`/`Event` →
      réactivité/bus ; scopes lexicaux ; cache de `CompiledExpr`.)
- [x] Réactivité host : `StateStore` (`tokio::sync` : `broadcast` des clés +
      `RwLock`) + `set` + `wait_until` à réévaluation **ciblée par deps**, câblé au
      runner (contexte = base ∪ state ∪ locals ; débloque `WaitFor::Until`).
- [x] Event bus (`TriggerBus` : emit/subscribe + `wait_for_event` → débloque
      `WaitFor::Event`) + profils (`Profile` activable + `TriggerBinding`) +
      `Dispatcher` (trigger → activation/filtre → run de l'automation, `run_forever`).
- [x] Files d'action (`ActionQueue`) : worker séquentiel non-chevauchant, `gap`
      configurable, pause/resume, skip (abort du job courant), clear, historique +
      replay, `wait_idle`. `QueueConfig` = resource persistée.
- [x] i18n (`LocaleRegistry`) : découverte des `lang/`, namespacing par id
      (`core.*`/`plugin.<id>.*`/`module.<id>.*`/`widget.<id>.*`), bundle mergé par
      locale, `translate` avec fallback `en`.
- [x] Extensions (`ExtensionRegistry`) : manifests Plugin/Module décodés du JSON
      de l'ABI, index des actions, résolution de deps (semver minimal), **gating
      certification** des capacités système, détection des modules orphelins.

## M2 — ABI plugin de bout en bout ✅

- [x] `bindgen!` host (`quicktys-host`) contre le world `plugin` (wasmtime 27,
      Component Model, sync) + WASI via `wasmtime-wasi`.
- [x] Capabilities plugin implémentées : `logging` (→ tracing), `storage` (kv),
      `reactivity` (`set-state` → `StateStore` namespacé par id, `emit-trigger` →
      `TriggerBus`), `modules` (stub `not-installed`, le tier certifié arrive M4).
      `PluginRuntime` charge un composant, décode le manifeste (`init`) dans
      l'`ExtensionRegistry`, et expose un `ActionInvoker` réel pour le runner.
- [x] `quicktys-plugin-sdk` (trait `Plugin` + `Manifest` réutilisant
      `quicktys-schema::Schema`) + macro `define_plugin!` (glue + export shims).
- [x] **Plugin `time` compilé en composant WASM, chargé et exécuté de bout en
      bout** (test d'intégration : `delay` → `log` + `set-state`, état observé
      côté host). Valide tout le contrat plugin (logique pure, sans module).
      (NB : le `delay` du plugin est pur — le sleep temporisé reste le nœud
      `WaitFor::Delay` du runner.)

## M3 — Shell Tauri + UI (réécrite de zéro) ✅

- [x] `quicktys-ipc` : façade `AppBackend` (commands UI→host : plugins,
      resources CRUD, run automation, state, i18n) + catalogue d'événements
      host→UI (`AppEvent` via `EventSink`). **Sans dépendance Tauri → testé**
      (5 tests : CRUD+events, run automation, plugins, i18n, state forwarder).
- [x] `tauri` ajouté à `quicktys-app` : fenêtre + boot (core services + WASM
      host + runner câblés), commands `#[tauri::command]` (fines coquilles sur
      `AppBackend`), `EventSink` Tauri (`emit`), capability `default`. Compile.
- [x] Frontend **clean-room** Vue 3 + PrimeVue + Pinia + vue-i18n (Vite) :
      client IPC (avec mocks navigateur), liste de plugins, **`SchemaForm`**
      (génération de formulaire par schéma), sélecteur de locale, et un
      **éditeur d'automatisation façon CastMate** (modèle étudié en clean-room) :
      **canvas grille temporelle pannable** ; séquence = **carte trigger** en tête
      (couleur trigger + bouton **▶ Test**) suivie de **cartes d'action chaînées**
      (`AutomationCanvas` + `TriggerCard` + `NodeCard`), hauteur fixe, header
      foncé (3 teintes par couleur, drag) + corps = **aperçu compact** de la
      config ; la **config éditable complète** est dans le **panneau latéral**
      (`Inspector` + `SchemaForm` : labels flottants IftaLabel, dropdowns, texte
      avec bouton template `{..}`, slider, toggles). Tous les nœuds couverts
      (`AutomationNode` récursif pour if/switch/loop/parallel/timeline/try/…),
      canvas **récursif éditable** (`SequenceFlow`) : **parallèle** en **lanes
      empilées** éditables (+/− branches) et **timeline** en **pistes décalées
      sur l'axe temps** (offset ms éditable, +/− tracks) ; **drop-zones
      d'insertion** (« + » entre les cartes → palette popover) ; **drag-and-drop
      intra/inter-lanes** (vuedraggable `group`) ; sélection de n'importe quel
      nœud (par id) → inspecteur ; **ruler temporel** (axe secondes) ; **zoom**
      Ctrl+molette + **largeur des cartes ∝ durée** (wait-delay) + offsets timeline
      à l'échelle ; **sélection rectangle** (rubber-band) + **copier/couper/coller**
      (multi-nœuds) + **suppression** + **undo/redo** (historique deep-watch,
      raccourcis) ; **aperçu de test** (surbrillance animée de la carte en cours).
      Rendu vérifié par capture headless.
- [x] i18n décentralisé câblé : **découverte des `lang/` au boot** (cœur +
      plugin `time`, namespacés) → command `locale_bundle` → `vue-i18n` (fallback
      `en`). (Reste : bundles de/es/pt + lang des extensions installées.)
- [x] **L'app charge `time` au boot** (scan `installed/plugins/` + plugin in-tree
      en dev) et **exécute une automation depuis l'UI** : bouton « Run » →
      `upsert_resource` + `run_automation` → Runner → invoker WASM → `time.delay`
      → `set-state` → event → UI. La tranche Runner→WASM→state est prouvée par le
      test `runner_plugin`. (Run GUI : OK sur poste desktop ; GTK indisponible
      dans le sandbox headless.)

> Lancer l'app : `npm --prefix frontend run build` puis
> `cargo run -p quicktys-app` (le binaire embarque `frontend/dist`, **pas** de
> serveur dev requis). Pour le dev avec HMR : `npm --prefix frontend run dev`
> puis remettre `devUrl`/`beforeDevCommand` dans `tauri.conf.json` (ou utiliser
> `cargo tauri dev`).

## M4 — Étage MODULE (drivers certifiés)

- [x] `bindgen!` host contre le world `module` (deuxième monde, partagé avec le
      world `plugin` via un `World` commun). Sous-ensemble de caps lié pour
      l'instant : `logging`/`storage`/`events`. (Caps **système** `net`/`input`/…
      à brancher avec le premier vrai driver — voir obs ci-dessous.)
- [x] `quicktys-module-sdk` (trait `Module` + `ModuleManifest`) + macro
      `define_module!`.
- [x] **Linker plugin↔module** : `modules.call` routé et validé (dépendance
      déclarée + méthode existante), `subscribe`/`unsubscribe`, et fan-out des
      `emit-event` → `on-module-event` en **livraison différée** (après le retour
      de l'appel, pour éviter la ré-entrance dans le plugin encore sur la pile).
- [x] **Gating de certification** : un module non certifié déclarant des capacités
      système est **refusé au chargement** ; `may_grant(module, cap)` exige
      certifié **ET** déclaré.
- [x] **Validé de bout en bout** par un module auto-contenu `counter` + plugin
      `counter-client` (test `two_tier`) : plugin → service module → événement
      module → état réactif du plugin. → le contrat à deux étages est prouvé.
- [x] **Capacités système câblées** (tout le world `module` lié) : **`net.fetch`
      réel** (client HTTP host-médié, `http://`), reste (`tcp`/`ws-connect`/`send`,
      input/audio/hid/files/process/timers) en **stubs** documentés. Module
      certifié `netdemo` + test `net_cap` : fetch réel d'un serveur local **+
      gating** (module non certifié avec caps → refusé au chargement).
- [x] **Facilitation OAuth** (`interface oauth`) : `TokenStore` host (env du
      provider, `access-token` avec **refresh transparent** via le client HTTP,
      `is-authorized`, `revoke`), testé contre un endpoint token local.
- [ ] **Reste M4** (nécessite I/O réseau réel / session desktop, non testable
      hors-ligne) : TLS (`https`/`wss`) + **sockets streaming** `ws-connect` →
      `on-host-event` ; **`authorize` interactif** (navigateur + capture du
      redirect) ; **module `obs` réel** (driver OBS-WebSocket) + plugin `obs`.

## M5 — Overlays / Widgets

- [ ] Serveur HTTP local + flux de state par websocket vers les browser sources.
- [ ] 3ᵉ type d'extension `widget` : paquet (UI + schéma + `lang/`), édition
      WYSIWYG, layouts d'overlay comme resources (`overlays/*.yaml`).
- [ ] Isolation du JS widget dans la browser source.

## M6 — Marketplace, DRM & distribution (gratuit & payant)

> Design de référence : [marketplace.md](marketplace.md) (format `.qtpkg`, registre
> statique signé, chaîne CA→éditeur→release, résolution de dépendances, install à
> chaud, IPC) et [drm.md](drm.md) (activation ECDH/HPKE, chiffrement partiel du
> payload, binding machine, watermark, **crypto hybride post-quantique** — suite
> `hyb-mlkem1024-mldsa87-v1`, niveau NIST 5). L'ordre ci-dessous est le **plan
> d'implémentation** : chaque étape est livrable et testable seule.

**Fondations (pas de réseau, pas de serveur — tout est `cargo test`-able)**
- [x] **M6.1 — Crate `quicktys-pkg`** : struct `Manifest` (sur-ensemble de
      `bundle.json` : `version`, `requires`, `capabilities`, `min_host`,
      `publisher`, `license`, `hashes`, `protection`), archive `.qtpkg` (tar
      **déterministe** + zstd), `content_hash` (sha256), `read_manifest` sans
      dépaquetage, `to_bundle_json` (pont rétrocompat), `unpack` anti-traversal.
      8 tests (round-trip, déterminisme, défauts, chemins non sûrs). ✅
- [x] **M6.2 — Install runtime (sideload, hash-only)** : crate `quicktys-install`
      (moteur pur : vérif hash → extract **atomique** `.tmp/→rename` →
      `installed/<kind>s/<id>/` + écriture `bundle.json` compat) ; lockfile
      `installed.json` + **comptage de références** (`required_by`) + détection
      d'orphelins ; garde anti-hijack d'id (publisher). Bridge `HostLoader`
      (hot-load WASM sans redémarrage) + `sideload_package`/`uninstall_package`/
      `list_installed` sur `AppBackend`, event `ExtensionsChanged`. `ensure_layout`
      gagne `registry-cache/`. 6 tests moteur. ✅ *(unload WASM live et
      consommation de l'event côté frontend : itérations suivantes.)*
- [x] **M6.3 — Semver & résolution** : `version_satisfies` réécrit sur la crate
      `semver` (ranges réels `^`, `~`, `>=,<`, parsing indulgent) ; `module_call`
      **enforce la version** (module incompatible refusé, pas seulement absent) via
      `dependency_satisfied` ; warn des deps non satisfaites au `load_plugin`.
      Côté install : champ `explicit` (modèle manuel/auto façon apt/cargo),
      orphelin = module `!explicit` sans dépendant, `purge_orphans` (point fixe) +
      commande `purge_orphans`. Tests : ranges semver, `dependency_satisfied`,
      purge/promotion. ✅

**Confiance (crypto hybride PQ)**
- [x] **M6.4 — Crate `quicktys-crypto`** : suite `hyb-mlkem1024-mldsa87-v1`
      implémentée — **signatures** hybrides Ed25519+ML-DSA-87 (les deux vérifiées),
      **KEM** hybride X25519+ML-KEM-1024 (combineur HKDF-SHA-384 lié au transcript),
      **seal/open** façon HPKE (encaps → dérivation → XChaCha20-Poly1305) pour la
      livraison de content key, **AEAD** direct (payload). Sérialisation
      octet+`SUITE_ID` de toutes les clés/signatures/sceaux. Crates pures Rust
      `fips203`/`fips204` (FIPS finaux) + dalek + chacha20poly1305 + hkdf. 9 tests
      (round-trips, détection d'altération des deux moitiés de signature, liaison
      AAD, mauvaise clé). ✅
- [x] **M6.5 — Signature & certification** : crate `quicktys-cert` (chaîne
      CA→éditeur→release sur `quicktys-crypto`, `TrustStore` de clés CA épinglées,
      CRL, `Verdict` Certified/Unsigned/Rejected). Install **calcule `certified`
      par crypto** (vérif de `signature.json` : chaîne + `manifest_hash` signé +
      caps ⊆ plafond + expiry + CRL) au lieu du flag en clair ; **refuse** une
      signature invalide, **autorise** le non-signé (non certifié). `may_grant`
      **câblé au dispatch réel** des caps (14 points net/input/audio/process).
      TrustStore vide par défaut (pas d'opérations CA encore) → comportement
      inchangé. 9 tests cert + 3 tests d'install signé (chaîne réelle). ✅
      *(Restant : re-vérif au boot des bundles `installed/` contre un TrustStore
      côté host — bundles `dist/bundles` de confiance par localité —, CRL signée,
      SLH-DSA pour la racine.)*

**Distribution**
- [x] **M6.6 — Registre statique** : crate `quicktys-registry` transport-agnostique
      (trait `Fetcher`) — `index.json` signé (vérif contre clés registre épinglées),
      `crl.json` caché (appliqué à l'install), blobs **content-addressés**
      (téléchargement + vérif `content_hash`), recherche, **résolution de
      dépendances** semver en **post-ordre** (un plugin tire ses modules, ordre de
      chargement deps-first, garde anti-cycle), détection de maj. AppBackend :
      `marketplace_refresh/search`, `install_from_registry` (résout → download →
      install → **hot-load ordonné**), `list_updates` ; `Fetcher` sur le client
      HTTP du host (`WasmHost::http_get`) ; registres via `QUICKTYS_REGISTRY`.
      7 tests moteur + 1 test d'intégration AppBackend bout-en-bout. **UI
      `MarketplaceView`** (onglets Parcourir/Installées) branchée. **Outils dev** :
      `quicktys-regserve` (serveur de registre HTTP : packe des sources pkg en
      blobs content-addressés + index signé + sert) et `quicktys-pack`
      (`dist/bundles` → sources pkg via extraction du vrai `init()` manifeste). ✅
      *(Restant : multi-registres réels (une seule URL), CRL signée, `details`.)*
- [x] **M6.7 — Consentement par-cap** : `ConsentStore` persistant
      (`<data>/consent.json`) dans `quicktys-install` — modèle *grâce jusqu'à
      revue* puis strict, `grant`/`revoke`/`needs_review` (re-prompt sur maj qui
      **élargit** les caps → nouvelles caps refusées jusqu'à re-revue). Enforcement
      au **dispatch réel** : `cap_granted = may_grant && consent` (host
      `set_consent`, sink `(id, cap) -> allowed`). AppBackend `grant_capabilities`/
      `revoke_capability`/`consent_status` (+ commandes), store partagé avec le
      host, event `ConsentChanged`. 5 tests store + 1 test host (révocation refuse
      la cap d'un module certifié bout-en-bout). **UI** : onglet Permissions de
      `MarketplaceView` (bascule accordé/révoqué par cap, badge « à revoir »). ✅

**Commerce & protection (DRM)**
- [x] **M6.8 — Entitlements & clés hors-ligne** : `Entitlement`/`LicenseKey`
      store-signés (hybride PQ) vérifiés **hors-ligne** contre des `store_keys`
      épinglées (`quicktys-cert`) ; `EntitlementStore` persistant
      (`entitlements.json`) ; **gate d'install** (un paquet `paid`/`subscription`
      exige une licence couvrante) ; `enter_license_key`/`list_entitlements` +
      `QUICKTYS_STORE` ; outil dev `quicktys-pack --paid`/`--issue-license`. Clés
      au format `QTLIC-…`. Tests licence + gate. ✅
- [x] **M6.9 — T1 chiffré** : payload `.enc` (chiffrement **partiel** : seul le
      `.wasm`, XChaCha20-Poly1305), `manifest.protection` ; **content key livrée
      hors-ligne** par le `key_material` de la licence (drm §A.8) ; décrypteur au
      **load** (`load_component` : `<wasm>.enc` → déchiffre → `Component::from_binary`,
      au boot **et** au hot-load) ; `quicktys-pack --encrypt`. Round-trip testé. ✅
      *(Restant : service d'activation **HPKE en ligne** + binding appareil/keystore
      OS + seats + grâce d'abonnement + CRL DRM revérifiée au load — la crypto seal/
      open hybride est prête dans `quicktys-crypto`.)*
- [x] **M6.10 — Watermark forensique** : `quicktys-crypto::watermark` — id
      **opaque** par acheteur (`derive_id` = sha256(seed‖ctx) tronqué),
      **embed/extract** invisible (zero-width, répété → survit à la troncature).
      Câblé à **l'install** : les assets texte (html/js/css/svg, **jamais** les
      `*.json`) d'un paquet `watermark:true` sont marqués **après** la vérif de
      signature (marque per-livraison), seed = compte/sku de l'entitlement ou
      `anonymous`. `quicktys-pack --watermark`. 4 tests crypto + 1 test d'install.
      ✅ *(**T2 server-gated** — la partie critique tourne côté serveur — et le
      **revenue share** — comptable côté serveur — nécessitent l'infra serveur ;
      hors périmètre client, documentés drm.md §4/§8.)*

## M7 — Boutons (surface générique)

**Décision (révision) :** l'ancien *deck* (module + plugin `deck` + serveur
`/deck` + appairage + driver Stream Deck `hid`) a été **retiré** au profit d'un
simple **widget `button`** sur le tableau de bord. Trop de plomberie pour la
valeur ; le widget réutilise le dashboard existant et le modèle event→profil.

- [x] **Widget `button`** (built-in, `template:"button"`, config `label/icon/
      color`) : un clic (hors mode Édition) **émet `button.clicked`** sur le bus
      via la commande host **`emit_trigger`** (`AppBackend::emit_trigger` →
      `TriggerBus`), contexte `{ id, label }` (l'`id` = id d'instance du widget).
      Pulse visuel au clic. **Aucun WASM** (ni module ni plugin) dans la boucle.
- [x] **Clic → automation** : `button.clicked` est un **trigger built-in** exposé
      dans le picker des Profils (groupe « Boutons »). En mode Édition, l'outil
      **« Lier une action »** du widget crée/ouvre un profil `button-<id>` avec le
      trigger `button.clicked` **déjà filtré** `id == "<id>"` → on édite la
      séquence inline. Boucle complète : clic → `emit_trigger` → Dispatcher →
      séquence du profil. Supprimer le widget supprime son profil lié.
- [~] Cap **`hid`** (host, `hidapi` `linux-native`) **conservée** comme infra
      générique (le monde `module` la déclare, le host l'implémente), mais **plus
      aucune extension livrée ne l'utilise** depuis le retrait du deck — dispo pour
      un futur driver matériel.
- [ ] *(Abandonné)* client web mobile / appairage QR / Companion — remplacés par
      le widget ; un overlay/surface distante pourrait réémerger comme widget.

## M8 — Migration & port de masse

- [x] **Outil de migration best-effort** CastMate → Quicktys (crate + bin
      **`quicktys-migrate`**). Lit les *données* CastMate de l'utilisateur (ses
      YAML — autorisé par la règle clean-room : c'est **sa** donnée, pas du code
      CastMate) et écrit des **ressources Quicktys valides** (round-trippent dans
      les vrais types `Profile`/`Automation`/`Node`) + un **rapport de migration**
      Markdown. Extraction **défensive** (plusieurs orthographes de champs
      tolérées) ; tout ce qui est ambigu est **rapporté, pas deviné** :
      - **profils** → `Profile{triggers:[TriggerBinding]}` ; chaque trigger →
        séquence d'`Action` nodes. `activation`/`filter` CastMate laissés `None`
        et **signalés** (à ré-écrire en Rhai — pas de traduction JS→Rhai devinée).
      - **actions** : deux orthographes (`{plugin,action,config}` **ou**
        raccourci `{ "plugin.action": {…} }`) ; `resultMapping` → `bind`.
      - **codemod de templates** : `{{ }}` conservé (Handlebars) ; un
        `{{ chemin.simple }}` **seul** devient `@chemin.simple` (valeur *typée*) ;
        toute expression (`{{ a + b }}`) est **flaggée** (Quicktys `{{ }}` =
        Handlebars, pas JS). Bin : `quicktys-migrate <dir> [--out <dir>]`.
        10 tests (unit + e2e round-trip). *(Les mappings de champs sont à affiner
        contre de vraies données ; le rapport dit quoi corriger.)*
- [ ] Porter les ~30 plugins CastMate, par lots de complexité croissante.
      *(Incrémental — le socle ABI/SDK est prouvé ; chaque lot = des extensions
      de plus, cf. `scripts/build-bundles.sh`.)*
  - [x] **Lot 1** :
    - **`random`** (plugin **pur**, caps safe) — `number`/`dice`/`coin`/
      `chance`/`pick` ; entropie via `wasi:random` (`getrandom`), chaque action
      **retourne** sa valeur (bindable). Test bornes (`random_plugin.rs`).
    - **`twitch`** (module certifié `net` + plugin) — le module pilote l'API
      **Helix** (`send-chat` → `POST /chat/messages`, `announce`, `get-stream`,
      escape-hatch `helix`), auth bearer + client-id via `set-auth`,
      `set-base-url` pour les tests ; le plugin (`chat`/`announce`, sandboxé,
      `requires: twitch`) passe par ses services. Test bout-en-bout contre un
      **serveur Helix stand-in** + refus si non-certifié (`twitch_module.rs`).
      *(Les **triggers** chat/follow/sub via **EventSub** attendent le chemin
      streaming `ws-connect → on-host-event` — non déclarés pour ne pas mentir.)*
