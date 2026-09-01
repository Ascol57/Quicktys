# Types de schéma

Un **schéma** est la description auto-portée d'un champ. Il pilote trois choses à
la fois :

1. **l'UI** — le frontend en génère un formulaire ;
2. **la validation** — tout payload JSON traversant l'ABI WebAssembly est vérifié
   contre lui ;
3. **la sérialisation** — YAML ↔ valeurs typées pour la persistance, avec
   aller-retour sûr.

Les schémas se sérialisent avec un discriminant `type` interne :
`{ "type": "Number", "min": 0, "max": 100 }`.

## Métadonnées partagées

Chaque variante porte :

| Champ | Signification |
| --- | --- |
| `name` | Le libellé — **une clé i18n**, pas une chaîne traduite. |
| `required` | Si le champ doit être présent (vérifié quand il est propriété d'un objet). |
| `suggest` | Un nom de source d'autocomplétion. Ne contraint pas la valeur ; l'éditeur ne fait que proposer. |

## Types de base

### `String`

```json
{ "type": "String", "default": "", "enum": ["a", "b"],
  "minLength": 0, "maxLength": 200, "multiline": false, "template": false }
```

`enum` produit une liste déroulante. `multiline` produit une zone de texte.
`template` active le templating `@`/`{{ }}`.

```rust
Schema::string()
Schema::string().options(["info", "warn", "error"])
Schema::template_string()
```

### `Number`

```json
{ "type": "Number", "default": 0, "min": 0, "max": 100,
  "step": 1, "slider": false, "unit": "%", "template": false }
```

`min` + `max` dans une config de widget produit un slider.

### `Boolean`

```json
{ "type": "Boolean", "default": false }
```

::: warning Non défini vaut ON dans les configs de widget
Le formulaire de config de widget lit un booléen non défini comme `!== false` : il
l'affiche donc à ON. Nommez les options `showX`, jamais `hideX`.
:::

### `Object`

```json
{ "type": "Object", "properties": { "a": { "type": "String" } } }
```

Les propriétés sont **ordonnées** — l'ordre est celui du formulaire généré.

```rust
Schema::object([
    ("message".into(), Schema::template_string()),
    ("level".into(),   Schema::string().options(["info", "warn"])),
])
```

### `Array`

```json
{ "type": "Array", "items": { "type": "String" }, "minItems": 0, "maxItems": 10 }
```

## Types d'UI riches

Ils existent pour qu'un formulaire soit expressif sans que l'extension livre la
moindre ligne d'UI. En ajouter un revient à ajouter une variante et un bras de
validation — **l'ABI ne change pas**, puisque les payloads restent du JSON validé
contre le schéma.

### `Duration`

```json
{ "type": "Duration", "default": 5, "min": 0, "max": 3600, "template": false }
```

Une durée **en secondes**, rendue par un contrôle de durée plutôt qu'un nombre
nu.

### `Color`

```json
{ "type": "Color", "default": "#7c5cff", "template": false }
```

Une chaîne de couleur RGBA/hexa, rendue par un sélecteur de couleur.

### `FilePath`

```json
{ "type": "FilePath", "default": "", "template": false }
```

Un chemin de système de fichiers — fichier ou média. Stocké en chaîne, rendu avec
un sélecteur de fichier.

### `Range`

```json
{ "type": "Range", "default": { "min": 0, "max": 10 }, "template": false }
```

Un intervalle numérique. Les deux bornes sont optionnelles et templatables
indépendamment.

### `Toggle`

```json
{ "type": "Toggle", "default": "toggle" }
```

Tri-état : `true`, `false`, ou la chaîne `"toggle"` qui signifie *inverser l'état
actuel*. C'est ce qui permet à une seule action « Muet » de couper, rétablir ou
basculer.

### `Resource`

```json
{ "type": "Resource", "resourceType": "queue", "default": null }
```

Une référence à une ressource du host, par id.

## Templating et coercition

Un champ avec `template: true` peut contenir du `@` et du `{{ }}`. Le host le
résout **avant** l'invocation de l'action, puis coerce le résultat vers le type du
champ :

| Type | Coercition |
| --- | --- |
| String, Color, FilePath | → chaîne, en respectant `maxLength` |
| Number, Duration | → nombre, borné à `min`/`max` |
| Boolean | `"false"` → `false`, tout le reste → vrai |
| Toggle | `"toggle"` ou un booléen |
| Range | templating appliqué récursivement à `min` et `max` |

Un champ templatable saute la vérification stricte de type quand sa valeur est
elle-même un template — le type est vérifié après résolution.

## Sources d'autocomplétion

`suggest` attache une source nommée à un champ chaîne :

```rust
Schema::string().suggest("obs-scene")
```

| Source | Propose |
| --- | --- |
| `alert` | les noms des widgets d'alerte actuellement placés |
| `countdown` | les noms des widgets de compte à rebours placés |
| `tts-voice` | les voix TTS installées |
| `obs-scene` | `obs.scenes` |
| `obs-input` | `obs.inputs` |

::: warning Une nouvelle source doit être câblée côté frontend
Le résolveur de suggestions vit dans l'app. Une source non câblée ne propose rien,
silencieusement — ce qui laisse l'utilisateur deviner, le pire des deux mondes.
:::

Les sources de noms de widget sont collectées sur le tableau de bord **et** dans
les overlays, donc un nom est proposé partout où le widget est placé.

## Où apparaissent les schémas

| Déclaré par | Pour |
| --- | --- |
| `ActionDecl::config` | le formulaire d'une action |
| `ActionDecl::result` | la forme que retourne une action (ce que capture `bind`) |
| `TriggerDecl::context` | la forme portée par une occurrence |
| `StateDecl::schema` | une valeur d'état publiée |
| `Manifest::settings` | le formulaire de réglages du plugin |
| `ServiceDecl::args` / `result` | un service de module |
| `EventDecl::payload` | un event de module |
| le `config` du `bundle.json` d'un widget | le formulaire de réglages d'un widget |

## Et ensuite

- [Helpers de templating](/fr/reference/templating)
- [Écrire un plugin](/fr/dev/plugin)
