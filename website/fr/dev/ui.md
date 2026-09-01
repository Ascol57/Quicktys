# Contributions d'UI

Les extensions peuvent enrichir l'interface de deux façons : une **contribution de
slot** (quelque chose de petit déposé à un endroit nommé) et une **vue contribuée**
(un onglet entier).

Les deux sont déclaratives. Ni l'une ni l'autre ne demande de modifier l'app.

## Les slots — un plugin ajoute un bouton

Chaque endroit extensible de l'interface porte un **id de slot** et rend ce que
les plugins y ont contribué, avec un objet de contexte décrivant la situation.

Déclarez une contribution dans le manifeste :

```rust
use quicktys_plugin_sdk::UiDecl;

Manifest::new("obs", "OBS Studio", "0.1.0")
    .ui(
        UiDecl::button("webview.toolbar", "add-browser-source")
            .label("plugin.obs.ui.add-to-obs")
            .icon("pi pi-plus"),
    )
```

C'est le vrai bouton **Ajouter à OBS** de la barre d'outils de l'éditeur
d'overlays. Cliquer dessus lance l'action `add-browser-source` du plugin avec le
**contexte du slot fusionné dans la config de l'action** — pour ce slot,
`{ url, name }` de l'overlay en cours d'édition. L'action appelle le module OBS,
qui crée la source navigateur dans la scène courante.

Deux formes sont disponibles :

| | |
| --- | --- |
| `UiDecl::button(slot, action)` | un bouton natif. `.label(clé)`, `.icon(classe)`, `.config(json)` pour une config fixe supplémentaire. |
| `UiDecl::custom(slot, entry)` | un fichier HTML livré par votre plugin, rendu dans une iframe isolée et alimenté par le même contexte. |

Les fichiers d'UI custom vivent dans `plugins/<id>/ui/`, sont copiés dans le
bundle par le script de build, et sont servis sur `pluginui://<id>/<fichier>` — sa
propre origine, sa propre CSP.

### Slots connus

| Id de slot | Où | Contexte |
| --- | --- | --- |
| `webview.toolbar` | la barre d'outils de l'éditeur d'overlays | `{ url, name }` de l'overlay courant |

La liste grandit à mesure que des endroits sont rendus extensibles ; un slot,
c'est un `<Slot id="…">` dans un composant Vue.

## Les vues — un module ajoute un onglet

Un **module** peut contribuer une page entière :

```rust
ModuleManifest::new("webview", "Webview", "0.1.0")
    .view("pi pi-images", Some("module.webview.view".into()), "index.html")
```

L'app lit ça depuis `list_modules`, ajoute une entrée à la barre latérale, et rend
`entry` dans une iframe isolée servie sur `moduleui://<id>/`. Le HTML vit dans
`modules/<id>/ui/` et est copié à la racine du bundle, donc `entry` vaut simplement
`"index.html"`.

L'onglet **Overlays** est exactement ça — contribué par le module `webview`, pas
codé en dur dans l'app.

### Le pont scopé

La différence clé avec un slot : une vue contribuée peut **piloter son propre
module**, via un pont requête/réponse `postMessage`.

```js
// page → host
{ type: "call", reqId: 7, service: "list-overlays", args: {} }

// host → page
{ type: "result", reqId: 7, ok: true, value: [ /* … */ ] }
```

Le pont est **scopé** : la page ne peut appeler que *son propre* module, jamais un
autre. C'est ce qui rend acceptable de rendre du HTML tiers avec un canal vivant
vers le host.

## Sérialisation : le vrai piège

::: danger Ne passez jamais une valeur réactive Vue à `postMessage`
`postMessage` sérialise avec l'algorithme de clonage structuré, qui **lève
`DataCloneError` sur un Proxy** — et les props, les valeurs de store et tout ce qui
est réactif *sont* des Proxy.

WebKit (la webview de Tauri sous Linux et macOS) les rejette purement et
simplement, donc le symptôme est un widget ou une vue qui ne reçoit jamais ses
réglages, silencieusement.

`toRaw()` ne suffit pas : il ne déballe que le premier niveau. Tout payload de pont
passe par un aller-retour JSON (`plain()` dans le frontend) — ce qui est correct,
puisque les ponts sont du JSON par contrat.
:::

## Pourquoi des iframes isolées et pas des composants

Une page contribuée est du HTML tiers. La rendre en ligne lui donnerait l'origine
de l'app, sa CSP et son DOM. La servir depuis son propre schéma d'URI lui donne :

- sa propre origine, donc elle ne peut pas lire le stockage de l'app ;
- sa propre CSP, donc les scripts inline fonctionnent sans relâcher celle de
  l'app ;
- un canal unique et auditable vers le host.

Le même raisonnement vaut pour les widgets. Voir [Écrire un
widget](/fr/dev/widget).

## Localisation

Une page isolée n'a pas accès à l'i18n de l'app. Le host résout les libellés et
les livre dans le message d'initialisation de la page. Lisez-les de là plutôt que
d'écrire le texte en dur.

## Et ensuite

- [Écrire un module](/fr/dev/module)
- [Écrire un widget](/fr/dev/widget)
