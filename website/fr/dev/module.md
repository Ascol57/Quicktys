---
description: "Écrire un module Quicktys certifié : capacités système, services et events, drivers socket, OAuth, et pourquoi il ne faut jamais autoriser dans une boucle de poll."
---

# Écrire un module

Un module est un **driver** : comment parler à OBS, à Twitch, à la carte son, au
clavier. C'est le seul étage qui reçoit des capacités système, et le seul qui doit
être **certifié** pour les utiliser.

Écrivez un module quand un plugin ne peut pas faire le travail — c'est-à-dire
quand il vous faut le réseau, un périphérique, le disque, ou lancer un processus.
Sinon, écrivez un [plugin](/fr/dev/plugin).

::: tip Vérifiez qu'un module n'existe pas déjà
Le module `http` est un client HTTP générique. Si votre intégration a la forme
d'une API REST, un plugin qui déclare `requires("http", "^0")` est toute la
réponse — et vous évitez complètement le processus de certification.
:::

## Le crate

```toml
# modules/mondriver/Cargo.toml
[package]
name = "mondriver"
version = "0.1.0"
edition = "2021"
license = "MIT"

[lib]
crate-type = ["cdylib"]

[dependencies]
quicktys-module-sdk = { path = "../../crates/quicktys-module-sdk" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wit-bindgen = "=0.36.0"
```

## Le module minimal

```rust
use quicktys_module_sdk::{Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",       // « module », pas « plugin »
});

use quicktys::api::audio::play_file;

struct Sound;

impl Module for Sound {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("sound", "Sound", "0.1.0")
            .capability(Capability::Audio)
            .service(ServiceDecl::new("play").args(Schema::object([
                ("path".to_string(), Schema::string()),
                ("volume".to_string(), Schema::number()),
            ])))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "play" => {
                let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
                if path.is_empty() {
                    return Err("le chemin du son est requis".to_string());
                }
                let volume = args.get("volume").and_then(Value::as_f64).unwrap_or(1.0) as f32;
                play_file(path, volume)?;
                Ok(Value::Null)
            }
            other => Err(format!("méthode inconnue : {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Sound);
```

## Services et events

Un module expose des **services** (méthodes requête/réponse que les plugins
appellent) et des **events** (ce qu'il pousse à ses abonnés). Il n'expose **pas**
d'actions ni de triggers — c'est le travail du plugin.

```rust
ModuleManifest::new("mondriver", "Mon driver", "0.1.0")
    .capability(Capability::Net)
    .service(ServiceDecl::new("send").args(schema).result(schema))
    .event(EventDecl::new("message").payload(schema))
    .settings(schema)
    .view("pi pi-desktop", Some("module.mondriver.view".into()), "index.html")
```

Un même module peut alimenter plusieurs plugins — c'est tout l'intérêt de la
séparation.

## Les capacités

Déclarez exactement ce dont vous avez besoin, et rien de plus :

```rust
.capability(Capability::Net)
.capability(Capability::Audio)
```

Disponibles : `Net`, `Input`, `Audio`, `Hid`, `Files`, `Process`, `Timers`.
Signatures complètes dans [Capacités du host](/fr/reference/capabilities).

Trois portes séparent une déclaration d'un appel réel :

1. **La certification** — un module non certifié qui déclare une capacité est
   **refusé au chargement**. En développement, le drapeau `certified` de votre
   descripteur de bundle tient lieu de signature.
2. **La déclaration** — une capacité non déclarée est refusée.
3. **Le consentement** — l'utilisateur doit l'avoir accordée, et peut la révoquer
   à tout moment, avec effet immédiat.

Les trois sont vérifiées à **chaque dispatch**, pas une fois au chargement.

::: info Certaines capacités ne demandent aucune certification
`storage`, `logging` et `events` sont sûres et disponibles pour tout module. Un
module qui n'a besoin que de celles-là — comme `webview`, qui possède les données
d'overlay — n'a besoin d'aucune certification.
:::

## Cycle de vie

| Méthode | Quand |
| --- | --- |
| `manifest()` | au chargement |
| `on_load()` | une fois après instanciation, capacités accordées — ouvrez vos connexions ici |
| `call()` | un plugin a appelé un service |
| `on_settings_changed()` | à chaque changement de réglage, et au chargement |
| `on_host_event()` | donnée bas niveau venue d'une capacité que vous détenez |
| `on_unload()` | avant que le composant ne soit détruit |

## Les events du host : la boucle du driver

Les capacités qui produisent de la donnée de façon asynchrone — un socket, un
timer, un rapport HID — la livrent à `on_host_event`, étiquetée avec l'`event-id`
que vous avez passé à l'ouverture. Votre travail est de transformer ces octets bas
niveau en **event de service** :

```rust
use quicktys::api::events::emit_event;
use quicktys::api::net::{ws_connect, send, close};

fn call(method: &str, args: Value) -> Result<Value, String> {
    match method {
        "connect" => {
            let url = args.get("url").and_then(Value::as_str).unwrap_or_default();
            let handle = ws_connect(url, "socket").map_err(|e| format!("{e:?}"))?;
            // mémoriser le handle dans un thread_local
            Ok(Value::Null)
        }
        _ => Err("méthode inconnue".into()),
    }
}

fn on_host_event(event_id: &str, payload: Value) {
    if event_id == "socket" {
        // parser, puis publier quelque chose de signifiant
        emit_event("message", &payload.to_string());
    }
}
```

::: warning Les payloads non-JSON
Les events du host portent ce que le transport a produit. Un socket JSON se parse ;
un protocole texte comme IRC, non. Le SDK laisse passer un payload non-JSON en
`Value::String` plutôt que de perdre la ligne dans un `null` — traitez donc les
deux formes.
:::

### `ws-connect` connecte avant de rendre la main

Le handle que vous récupérez est une connexion vivante et enregistrée. C'est
important pour les protocoles où **le client parle en premier** — l'IRC de Twitch
reste muet tant que `CAP`/`NICK`/`JOIN` ne sont pas arrivés. Si la connexion était
encore en cours d'établissement en arrière-plan, vos premiers `send` seraient
perdus et vous auriez un socket ouvert qui ne produit jamais un message.

`net.close` ferme vraiment : il coupe les deux moitiés, donc le thread de lecture
se termine. Ne fermer que l'écriture laisserait le lecteur garé sur un socket
vivant, en train de continuer à livrer — c'est ainsi qu'un chat rejoint livre
chaque message en double.

## Requête/réponse sur un socket

Un driver WebSocket a généralement besoin de corréler une réponse avec la requête
qui l'a causée. Le motif du module OBS :

1. envoyer une requête avec un id généré, et le mémoriser dans une map d'attente ;
2. quand la réponse arrive dans `on_host_event`, retrouver l'id ;
3. poursuivre l'opération avec ce qui est revenu.

Ça permet à un service comme « changer la visibilité d'une source » de résoudre
d'abord l'id de l'élément de scène puis d'agir dessus — invisible pour le plugin
comme pour l'utilisateur.

## Deux flux d'events

Gardez-les séparés. Le module OBS est la référence :

- Les **vrais events** (une scène a changé, un stream a démarré) deviennent des
  events de module qu'un plugin transpose en **triggers**.
- La **synchronisation d'état** (la scène courante, la liste des sources, les
  compteurs de performance) devient un unique event `state` en bloc, que le plugin
  **mirroir dans l'état sans rien déclencher**.

Les confondre déclenche un trigger à chaque rafraîchissement de valeur.

## Ne jamais autoriser implicitement dans une boucle

Tout module qui est interrogé — le host appelle le service `poll` de chaque module
qui en déclare un, toutes les trois secondes — ne doit jamais se
ré-authentifier de lui-même.

Le module Pear l'a appris à la dure : le lecteur affiche une **boîte de dialogue
native** quand un client inconnu demande un jeton, donc un naïf « pas de jeton ?
on s'authentifie » fait apparaître une modale à l'écran toutes les trois secondes,
indéfiniment.

Les règles qui en sont sorties, et que tout driver authentifiant devrait
reprendre :

- un **refus est persistant** — seul un service `authorize` explicite redemande ;
- une tentative **échouée** est limitée à une par minute ; une tentative
  **réussie** annule cette limite ;
- un 401 efface le jeton, ré-autorise **une fois**, et rejoue l'appel.

## OAuth

Le cœur pilote le flux interactif ; le module possède le fournisseur.

Exposez un service **`oauth-config`** retournant
`{ provider, device_url, token_url, client_id, scopes }`. L'écran Réglages de
l'app pilote le flux device code à partir de là, stocke les jetons et les
rafraîchit. Votre module relit ensuite un jeton frais :

```rust
use quicktys::api::oauth::access_token;

let token = access_token("twitch")?;
```

Aucune spécificité de fournisseur ne fuit dans le cœur, et vous n'écrivez jamais
de mécanique OAuth.

::: info `oauth.authorize` est encore un stub, volontairement
C'est un appel synchrone qui n'a nulle part où rendre un code utilisateur à
afficher pendant l'attente. La moitié interactive tourne donc côté UI, via l'IPC.
`oauth.access-token` est réel, et c'est ce que vous devez appeler.
:::

## Le rendre testable hors-ligne

Exposez un service pour pointer votre driver ailleurs :

```rust
.service(ServiceDecl::new("set-base-url").args(Schema::object([
    ("url".to_string(), Schema::string()),
])))
```

Tous les modules de référence le font — `set-base-url` sur `pear`, `set-chat-url`
et `set-eventsub-url` sur `twitch`. Ça coûte une méthode et ça rend toute
l'intégration testable contre un serveur bouchon, en CI, sans identifiants.

## Contribuer un onglet

Un module peut contribuer une page entière de l'app :

```rust
.view("pi pi-desktop", Some("module.mondriver.view".into()), "index.html")
```

Le HTML vit dans `modules/<id>/ui/`, est copié à la racine du bundle, et est servi
sur `moduleui://<id>/`. La page obtient un pont requête/réponse scopé et ne peut
appeler **que son propre module**. Voir [Contributions d'UI](/fr/dev/ui).

## Certification

Publier un module qui utilise des capacités système implique d'obtenir un
certificat d'éditeur et de signer vos releases. En développement, le drapeau
`certified` de `scripts/build-bundles.sh` en tient lieu.

Voir [Empaqueter & publier](/fr/dev/packaging#certification).

## Et ensuite

- [Capacités du host](/fr/reference/capabilities) — chaque fonction importable.
- [Empaqueter & publier](/fr/dev/packaging) — signature et distribution.
