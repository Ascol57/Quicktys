---
description: "Écrire un plugin Quicktys en Rust : le manifeste, les schémas, actions, triggers, état réactif, stockage, et l'appel des services d'un module certifié."
---

# Écrire un plugin

Un plugin est la moitié visible d'une intégration : il déclare des **actions**,
des **triggers**, de l'**état** et des **réglages**, et il compose des modules
pour tout ce qu'il ne peut pas faire lui-même — c'est-à-dire tout ce qui implique
le monde extérieur.

N'importe qui peut en publier un. Ni certification ni signature requises. Le bac à
sable est la garantie.

## Le crate

Les extensions vivent hors du workspace host. Créez le dossier et un paquet
autonome :

```toml
# plugins/greeter/Cargo.toml
[package]
name = "greeter"
version = "0.1.0"
edition = "2021"
license = "MIT"          # votre extension, votre licence

[lib]
crate-type = ["cdylib"]  # un composant WebAssembly, pas une bibliothèque Rust

[dependencies]
quicktys-plugin-sdk = { path = "../../crates/quicktys-plugin-sdk" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wit-bindgen = "=0.36.0"
```

::: warning Épinglez `wit-bindgen` à `=0.36.0`
Les 0.5x plus récentes ont besoin de la fonctionnalité std instable `wasip2`.
L'épinglage est délibéré, pas de la négligence.
:::

## Le plugin minimal

```rust
// plugins/greeter/src/lib.rs
use quicktys_plugin_sdk::{ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

// Génère les bindings du world `plugin`. Doit précéder define_plugin!.
wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};

struct Greeter;

impl Plugin for Greeter {
    fn manifest() -> Manifest {
        Manifest::new("greeter", "Greeter", "0.1.0")
            .icon("mdi mdi-hand-wave")
            .color("#8dc1a9")
            .action(
                ActionDecl::new("greet").config(Schema::object([(
                    "who".to_string(),
                    Schema::template_string().named("plugin.greeter.fields.who"),
                )])),
            )
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            "greet" => {
                let who = config.get("who").and_then(Value::as_str).unwrap_or("monde");
                log(Level::Info, &format!("bonjour, {who}"));
                Ok(Value::Null)
            }
            other => Err(ActionError::failed(format!("action inconnue : {other}"))),
        }
    }
}

quicktys_plugin_sdk::define_plugin!(Greeter);
```

C'est un plugin complet et chargeable.

`define_plugin!` relie votre implémentation de `Plugin` à l'export généré
`plugin-guest`, gère le marshalling JSON dans les deux sens, et émet les shims
d'export du composant. Il doit être invoqué **après** `wit_bindgen::generate!`,
pour que les chemins `exports::…` générés et la macro `export!` soient en portée.

## Le manifeste

`manifest()` est tout le contrat. Le host le lit pour construire l'UI, valider les
payloads et résoudre les dépendances.

```rust
Manifest::new("greeter", "Greeter", "0.1.0")
    .icon("mdi mdi-hand-wave")      // une classe Material Design Icons
    .color("#8dc1a9")               // couleur d'accent dans l'éditeur
    .action(ActionDecl::new("greet").config(schema).result(schema))
    .trigger(TriggerDecl::new("greeted").context(schema))
    .state_field(StateDecl::new("last").schema(schema))
    .settings(schema)               // un formulaire généré dans les Réglages
    .requires("http", "^0")         // une dépendance de module, en semver
    .ui(UiDecl::button("webview.toolbar", "greet"))
```

::: danger Émettez des clés, pas des phrases
Toute chaîne lisible par un humain est une **clé i18n** :
`.named("plugin.greeter.fields.who")`, pas `.named("Qui")`. Le texte vit dans
`lang/en.json`. Un manifeste qui émet de l'anglais ne sera jamais traduit. Voir
[Traductions](/fr/dev/i18n).
:::

### Les schémas

Config, contexte, résultats et réglages se décrivent tous avec `Schema`, le type
qu'utilise le host lui-même. Le formulaire de l'éditeur en est généré.

```rust
Schema::object([
    ("message".into(), Schema::template_string()),      // accepte @ et {{ }}
    ("count".into(),   Schema::number()),
    ("loud".into(),    Schema::boolean()),
    ("level".into(),   Schema::string().options(["info", "warn", "error"])),
    ("scene".into(),   Schema::string().suggest("obs-scene")),
    ("tags".into(),    Schema::array(Schema::string())),
])
```

Deux indices à connaître :

- **`template_string()`** marque un champ comme templatable. L'éditeur l'affiche
  avec l'autocomplétion `@`/`{{`, et le host le résout avant que votre action ne
  tourne — donc au moment où vous le lisez, `@pear.title` est déjà le titre.
- **`.suggest("<source>")`** attache une source d'autocomplétion. La valeur reste
  une chaîne libre ; l'éditeur ne fait que proposer. Sources existantes : `alert`,
  `countdown`, `tts-voice`, `obs-scene`, `obs-input`.

  Une nouvelle source doit être **câblée dans le fournisseur de suggestions du
  frontend** pour produire quoi que ce soit. Une source non câblée n'autocomplète
  rien, silencieusement, ce qui laisse l'utilisateur deviner.

Liste complète des types dans [Types de schéma](/fr/reference/schema).

## Les actions

`invoke_action` reçoit l'id de l'action, la config **déjà validée et déjà
templatée**, et le contexte du trigger.

```rust
fn invoke_action(action: &str, config: Value, context: Value) -> Result<Value, ActionError> {
    match action {
        "roll" => {
            let sides = config.get("sides").and_then(Value::as_u64).unwrap_or(6);
            let value = /* … */ 4;
            Ok(quicktys_plugin_sdk::json!({ "value": value }))
        }
        other => Err(ActionError::failed(format!("action inconnue : {other}"))),
    }
}
```

La `Value` retournée est ce que capture le **bind** d'un nœud action : déclarez un
schéma de `result` si vous renvoyez quelque chose de structuré.

Deux modes d'échec :

| | |
| --- | --- |
| `ActionError::failed(msg)` | un vrai échec — termine le run sauf s'il est intercepté par un nœud `try` |
| `ActionError::Aborted` | annulation — le host ne la présente pas comme une erreur |

## L'état

Publiez une valeur : elle apparaît dans la vue Variables, dans l'autocomplétion,
et dans tous les templates et conditions.

```rust
use quicktys::api::reactivity::set_state;

set_state("last", &serde_json::json!("monde").to_string());
```

La clé est **locale** ; le host la namespace en `greeter.last`. Déclarez-la dans
le manifeste avec `state_field(StateDecl::new("last"))` pour que l'éditeur la
connaisse avant même que votre plugin n'ait tourné.

## Les triggers

```rust
use quicktys::api::reactivity::emit_trigger;

emit_trigger("greeted", &serde_json::json!({ "who": who }).to_string());
```

Là encore l'id est local — le host émet `greeter.greeted` sur le bus, et les
profils qui y sont liés s'exécutent. Le JSON que vous passez devient le **contexte
du trigger** : ce que les filtres évaluent et ce que `{{ }}` lit.

Déclarez la forme du contexte avec `TriggerDecl::new("greeted").context(schema)`
pour que l'éditeur puisse l'autocompléter.

## Le stockage

Clé/valeur par extension, persisté dans
`<user-data>/quicktys/storage/<id>.json` :

```rust
use quicktys::api::storage::{kv_get, kv_set};

kv_set("timers", &dump);
let restored = kv_get("timers");
```

Les valeurs sont des chaînes — sérialisez votre propre JSON. C'est une capacité
sûre : aucune certification nécessaire.

## Appeler un module

C'est ainsi qu'un plugin atteint quoi que ce soit de réel. Déclarez la dépendance
dans le manifeste, puis appelez :

```rust
use quicktys::api::modules::call as module_call;

Manifest::new("discord", "Discord", "0.1.0").requires("http", "^0")

let req = json!({
    "method": "POST",
    "url": url,
    "headers": { "Content-Type": "application/json" },
    "body": payload.to_string(),
});
let response = module_call("http", "request", &req.to_string())
    .map_err(|e| ActionError::failed(format!("{e:?}")))?;
```

Le host vérifie que la dépendance a été déclarée, que la méthode existe, et que
les arguments correspondent au schéma publié par le module. Les erreurs reviennent
sous forme de `CallError::{NotInstalled, NoSuchMethod, Denied, Failed(String)}`.

::: tip Réutilisez un module générique avant d'en écrire un
Le module `http` est un client HTTP généraliste. Si votre intégration se résume à
« POSTer du JSON à une URL », vous n'avez besoin d'aucun module : déclarez
`requires("http", "^0")` et c'est fini. C'est exactement ainsi que fonctionne le
plugin Discord.
:::

## S'abonner aux events d'un module

```rust
use quicktys::api::modules::subscribe as module_subscribe;

fn on_load() -> Result<(), String> {
    module_subscribe("pear", "song-changed").map_err(|e| format!("{e:?}"))?;
    Ok(())
}

fn on_module_event(module_id: &str, event_id: &str, payload: Value) {
    if (module_id, event_id) == ("pear", "song-changed") {
        emit_trigger("song-changed", &payload.to_string());
    }
}
```

C'est la forme de la plupart des vrais plugins : un module fait le travail de
protocole et émet des events ; le plugin en mirroir certains dans l'état et
transpose les autres sur le bus de triggers.

::: warning Ne déclenchez pas de trigger à la première observation
Quand un driver apprend l'état courant pour la première fois — la musique qui
jouait déjà au démarrage — mirrorez-le dans l'état mais ne déclenchez **pas** le
trigger. Sinon chaque lancement déclenche une alerte de musique. Ne déclenchez que
sur un changement depuis une valeur précédente **connue**.
:::

## Les réglages

```rust
.settings(Schema::object([(
    "webhook".to_string(),
    Schema::string().named("plugin.discord.settings.webhook"),
)]))
```

Le host rend le formulaire, persiste les valeurs, et vous les livre :

```rust
fn on_settings_changed(settings: Value) {
    // stockez-les — un thread_local RefCell est l'idiome habituel
}
```

## Cycle de vie

| Méthode | Quand |
| --- | --- |
| `manifest()` | au chargement, via `init` |
| `on_load()` | une fois après instanciation — abonnez-vous ici |
| `invoke_action()` | à chaque action |
| `on_settings_changed()` | à chaque changement de réglage, et au chargement |
| `on_module_event()` | quand un event de module auquel vous êtes abonné survient |
| `on_unload()` | avant que le composant ne soit détruit |

## Les locales

```json
// plugins/greeter/lang/en.json
{
  "name": "Greeter",
  "description": "Says hello.",
  "fields": { "who": "Who" },
  "actions": { "greet": { "name": "Say hello" } },
  "triggers": {},
  "state": {},
  "settings": {}
}
```

Le host découvre `lang/` et le namespace en `plugin.greeter.*`. `en.json` est la
locale de référence ; les clés manquantes dans les autres locales y retombent.
Voir [Traductions](/fr/dev/i18n).

## Le construire et le charger

Ajoutez-le à `scripts/build-bundles.sh` :

```bash
EXTENSIONS=(
  # …
  "plugin:plugins/greeter:greeter:greeter.wasm"
)
```

Puis :

```bash
scripts/build-bundles.sh
setup/run.sh --solo
```

Votre plugin apparaît dans le sélecteur d'actions.

## Et ensuite

- [Écrire un module](/fr/dev/module) — quand un plugin ne suffit pas.
- [Contributions d'UI](/fr/dev/ui) — ajouter un bouton ou un onglet à l'app.
- [Empaqueter & publier](/fr/dev/packaging) — le distribuer.
