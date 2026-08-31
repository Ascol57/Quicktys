# Quicktys

Réécriture de [CastMate](../) en **Rust + Tauri**, avec des **plugins WebAssembly**
(Component Model), pensée dès le départ pour :

- 🌍 **les traductions** (i18n de bout en bout, host ↔ UI ↔ plugins) ;
- 🛒 **un marketplace** de plugins et de contenus (overlays, automations) ;
- 🎛️ **les remote buttons** (télécommandes / satellites pilotant les automations).

> État : **M0 — scaffolding & architecture**. Aucun code fonctionnel pour
> l'instant. Voir [`ARCHITECTURE.md`](ARCHITECTURE.md) et
> [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Trois types d'extension

| Type        | Qui le développe         | Rôle | Confiance / capacités |
| ----------- | ------------------------ | ---- | --------------------- |
| **Plugin**  | n'importe qui            | lien backend ↔ utilisateur : actions / triggers / UI | WASM sandbox ; **aucun** accès externe direct ; consomme les modules |
| **Module**  | dev **certifié**         | driver : « comment parler à OBS / Twitch / l'audio / le clavier » | WASM ; capacités **système** (réseau, clavier, audio, USB) ; expose des services |
| **Widget**  | n'importe qui            | overlay / dashboard affiché en browser source OBS | frontend (pas de WASM) ; isolé dans la browser source ; reçoit le state via websocket |

Le cœur ne contient **aucun** module/widget : tout est téléchargé **à la demande**
(installer un plugin tire les modules certifiés qu'il requiert).

## Structure

```
Quicktys/
├── Cargo.toml              # workspace host (Rust)
├── wit/
│   ├── host.wit            # capabilities du host (logging, storage, oauth, net, input…)
│   └── world.wit           # les 2 worlds : `plugin` et `module`
├── crates/
│   ├── quicktys-app        # binaire Tauri (shell + boot)
│   ├── quicktys-core       # moteur domaine : extensions, resources, profils, séquences
│   ├── quicktys-schema     # système de schémas (validation, YAML, formulaires)
│   ├── quicktys-host       # runtime WASM (wasmtime), capabilities, certification, linker
│   ├── quicktys-ipc        # pont host ↔ WebView (commands/events Tauri)
│   ├── quicktys-plugin-sdk # SDK Rust côté plugin (tout le monde)
│   └── quicktys-module-sdk # SDK Rust côté module (devs certifiés)
├── lang/                   # i18n du cœur (namespace core.*)
├── plugins/                # plugins WASM — chacun avec son lang/ (plugin.<id>.*)
│   └── time/{src, lang/}
├── modules/                # modules WASM certifiés — chacun avec son lang/ (module.<id>.*)
│   └── obs/{src, lang/}
├── widgets/                # overlays/dashboards (frontend, pas de WASM) — widget.<id>.*
├── docs/                   # ROADMAP.md, automation-engine.md
└── frontend/               # WebView Vue 3 + PrimeVue (réécrite de zéro, clean-room)
```

> **i18n décentralisé** : chaque « chose » (cœur, module, plugin) porte son
> dossier `lang/` (`en.json`, `fr.json`, …). Le host le découvre et le namespace
> automatiquement. Voir [`ARCHITECTURE.md`](ARCHITECTURE.md) §10.

## Décisions fondatrices

| Sujet            | Choix                                                      |
| ---------------- | ---------------------------------------------------------- |
| Code             | **Clean-room** — aucune ligne de CastMate                  |
| Licence          | Cœur **AGPL-3.0 + exception d'extension + CLA** (§0bis)    |
| Extensions       | Tierces : **libres/vendables** (exception) — celles d'ici : © Ascol57 |
| Runtime plugins  | WebAssembly **Component Model** (wasmtime + WIT)           |
| Expressions      | **Rhai** (templating + contrôle de flux unifiés)           |
| Frontend         | **Réécrit de zéro** sur stack Vue 3 + PrimeVue             |
| Shell desktop    | **Tauri 2**                                                |
| Persistance      | **YAML** (migration best-effort depuis CastMate)           |
| Async            | **Tokio**                                                  |

## Licence

Copyright © 2026 Ascol57. Ce dépôt a **deux régimes** — voir
[`COPYING.md`](COPYING.md) pour la carte complète :

- **le cœur** (`crates/`, `frontend/`, `wit/`, `lang/`, `scripts/`) est sous
  **AGPL-3.0-only** ([`LICENSE`](LICENSE)) **+ l'exception d'extension Quicktys**
  ([`LICENSE-EXCEPTION`](LICENSE-EXCEPTION)) : une extension qui parle au cœur
  **uniquement via l'ABI officielle** n'est pas une œuvre dérivée et peut être
  **fermée et vendue** ;
- **les extensions livrées ici** (`plugins/`, `modules/`, `widgets/`) sont
  **propriétaires, source-available** — © Ascol57, tous droits réservés
  ([`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS)). Lisibles, compilables et
  utilisables avec Quicktys ; ni redistribuables ni réutilisables.

L'**API** d'extension est libre pour tout le monde ; cette **implémentation**-ci
ne l'est pas.
