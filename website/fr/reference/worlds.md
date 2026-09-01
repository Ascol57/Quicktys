# Worlds & cycle de vie

Les contrats d'extension sont des interfaces WIT du paquet
**`quicktys:api@0.2.0`**, définies dans
[`wit/world.wit`](https://github.com/Ascol57/Quicktys/blob/main/wit/world.wit).

## La règle de l'ABI

> La partie typée du contrat est la **signature de fonction WIT**. Tout payload
> structuré — config, contexte, arguments de service, events, schémas, valeurs
> d'état — est une **chaîne JSON**, validée par le host contre le schéma déclaré
> par l'extension dans son manifeste.

Ça garde l'ABI stable pendant que le système de schémas évolue. Ajouter un
nouveau type de schéma riche ne change aucune signature WIT.

## `world plugin`

```wit
world plugin {
    import logging;
    import storage;
    import reactivity;
    import modules;
    export plugin-guest;
}
```

C'est la liste complète de ce qu'un plugin peut atteindre. Les capacités système
ne sont pas dans ce world : un plugin ne peut pas les demander — il ne peut même
pas les nommer.

### `interface plugin-guest`

```wit
variant invoke-error { aborted, failed(string) }

init: func() -> string;
on-load: func() -> result<_, string>;
on-unload: func();
invoke-action: func(action-id: string, config-json: string, context-json: string)
    -> result<string, invoke-error>;
on-settings-changed: func(settings-json: string);
on-module-event: func(module-id: string, event-id: string, payload-json: string);
```

| Export | Contrat |
| --- | --- |
| `init` | Retourne le manifeste JSON : id, nom, version, icône, couleur, les actions / triggers / état / réglages / secrets déclarés **avec leurs schémas**, et la liste des **modules requis** avec leurs plages semver. |
| `on-load` | Appelé une fois après instanciation. Abonnez-vous aux events de module ici. |
| `on-unload` | Appelé avant destruction du composant. |
| `invoke-action` | Exécute une action. Le host a déjà validé `config` et `context` contre les schémas déclarés, **et résolu les templates**. |
| `on-settings-changed` | Les réglages du plugin ont changé. Appelé aussi au chargement. |
| `on-module-event` | Un event de module auquel ce plugin s'est abonné. C'est là qu'un trigger comme « scène OBS changée » se réalise : recevoir l'event du module, le transformer, appeler `emit-trigger`. |

`invoke-error::aborted` signifie annulation, et le host ne la rapporte pas comme
une erreur. `failed(string)` est un vrai échec.

## `world module`

```wit
world module {
    import logging;
    import storage;
    import events;
    import oauth;
    // Capacités système — modules certifiés uniquement.
    import net;
    import input;
    import audio;
    import hid;
    import files;
    import process;
    import timers;
    export module-guest;
}
```

### `interface module-guest`

```wit
init: func() -> string;
on-load: func() -> result<_, string>;
on-unload: func();
call: func(method: string, args-json: string) -> result<string, string>;
on-settings-changed: func(settings-json: string);
on-host-event: func(event-id: string, payload-json: string);
```

| Export | Contrat |
| --- | --- |
| `init` | Retourne le manifeste JSON : id, nom, version, les **services** (méthodes + schémas), les **events** (+ schémas), réglages/secrets, les **capacités système requises**, et une `view` contribuée optionnelle. |
| `on-load` | Ouvrez vos connexions ici — les capacités sont accordées à ce stade. |
| `call` | Un plugin a appelé un de vos services. `args` a été validé par le host. |
| `on-settings-changed` | Les réglages ont changé. Appelé aussi au chargement. |
| `on-host-event` | Donnée bas niveau d'une capacité que vous détenez : données de socket, tick de timer, rapport HID. Transformez-la en event de service avec `events.emit-event`. |

Un module expose des **services et des events**, jamais des actions et des
triggers. Un même module peut alimenter plusieurs plugins.

::: warning Les payloads d'`on-host-event` ne sont pas toujours du JSON
Un socket JSON se parse ; un protocole texte comme IRC, non. Le SDK laisse passer
un payload non-JSON sous forme de chaîne plutôt que de perdre la ligne dans un
`null`.
:::

## Ordre du cycle de vie

Au démarrage, et au chargement à chaud après une installation :

1. **Les modules d'abord**, puis les plugins — les plugins peuvent dépendre des
   modules.
2. Pour chaque bundle : lire `bundle.json`, charger le `.wasm`, découvrir
   `lang/`.
3. Les capacités déclarées d'un module sont confrontées à son drapeau `certified`.
   Un module non certifié qui déclare une capacité est **refusé**.
4. `init()` → le manifeste est décodé dans le registre des extensions.
5. `on-load()`.

Un id déjà chargé est ignoré, donc un bundle installé masque un bundle de
développement de même id.

## Les appels à travers la frontière

### Plugin → module

```
plugin : modules.call("obs", "set-scene", "{\"scene\":\"Intro\"}")
   │
   ├─ host : « obs » est-il dans les `requires` du plugin ?
   ├─ host : « obs » exporte-t-il un service « set-scene » ?
   ├─ host : les arguments valident-ils contre son schéma publié ?
   ▼
module : call("set-scene", args) -> result<string, string>
```

Les échecs reviennent en `call-error` :

```wit
variant call-error { not-installed, no-such-method, denied, failed(string) }
```

### Module → plugin

```
module : events.emit-event("scene-changed", payload)
   │
   ▼ le host diffuse à tous les plugins abonnés
plugin : on-module-event("obs", "scene-changed", payload)
   │
   ▼ typiquement
plugin : reactivity.emit-trigger("scene-changed", context)
   │
   ▼ le host émet `obs.scene-changed` sur le bus de triggers
```

## Isolation

Chaque extension tourne dans **son propre `Store` wasmtime** avec ses seules
capacités. Un plugin ne peut pas lire le disque, ouvrir un socket, ni atteindre
des secrets qui ne lui ont pas été accordés. Il ne peut agir que via les services
des modules installés, eux-mêmes arbitrés par le host.

## Conventions WIT

Si vous éditez les contrats :

- la déclaration `package` doit être la **première ligne** de chaque fichier — un
  bloc de commentaire en tête est interprété comme un commentaire de documentation
  sur le paquet, et deux fichiers en portant chacun un est une erreur ;
- les interfaces du même paquet s'importent par nom court : `import logging;`, pas
  `import quicktys:api/logging;` ;
- les identifiants ne peuvent pas masquer un mot-clé WIT — d'où `list-dir`,
  puisque `list` est le type `list<T>`.

Les bindings côté host sont générés par le `bindgen!` Component Model de wasmtime
lui-même dans `quicktys-host` ; ceux côté invité par `wit_bindgen::generate!` dans
chaque crate d'extension.

## Et ensuite

- [Capacités du host](/fr/reference/capabilities) — chaque fonction importable.
- [Types de schéma](/fr/reference/schema) — le vocabulaire des payloads JSON.
