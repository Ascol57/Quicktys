---
description: "Écrire un widget Quicktys : bundle.json, le schéma de configuration, le pont postMessage, les pages d'aperçu, et les règles d'isolation sur les polices et le réseau."
---

# Écrire un widget

Un widget est le troisième type d'extension, et le plus simple à écrire : **pas de
Rust, pas de WebAssembly, pas d'étape de compilation**. C'est un fichier HTML, un
descripteur JSON et un dossier de traductions. Le bundle est authoré, pas compilé.

Les widgets apparaissent à deux endroits — la grille du tableau de bord et les
overlays de stream — et s'y rendent à l'identique.

## Le bundle

```
widgets/maboite/
├── bundle.json      le descripteur + le schéma de config
├── index.html       le widget lui-même
├── preview.html     optionnel : ce que montre l'éditeur d'overlays
└── lang/
    ├── en.json
    └── fr.json
```

`scripts/build-bundles.sh` copie ce dossier tel quel. Ajoutez-le à la liste
`WIDGETS` pour qu'il soit livré :

```bash
WIDGETS=(
  "widget:widgets/maboite:maboite"
)
```

## `bundle.json`

```json
{
  "id": "maboite",
  "kind": "widget",
  "name": "Ma boîte",
  "icon": "pi pi-star",
  "runtime": "html",
  "entry": "index.html",
  "preview": "preview.html",
  "requires": ["countdown"],
  "defaultW": 2,
  "defaultH": 1,
  "config": {
    "type": "Object",
    "properties": {
      "name": { "type": "String", "default": "timer" },
      "format": { "type": "String", "enum": ["auto", "mm:ss"], "default": "auto" },
      "showLabel": { "type": "Boolean", "default": true },
      "textColor": { "type": "Color", "default": "#ffffff" },
      "radius": { "type": "Number", "default": 12, "min": 0, "max": 60 }
    }
  }
}
```

| Champ | Signification |
| --- | --- |
| `runtime: "html"` + `entry` | un widget **runtime** : votre HTML dans une iframe isolée |
| `template` | un widget **built-in** : dessiné par un composant interne à l'app |
| `preview` | une page séparée que l'éditeur d'overlays charge à la place d'`entry` |
| `requires` | les ids de plugins dont ce widget a besoin — montrés à l'utilisateur |
| `defaultW` / `defaultH` | taille initiale, en cellules de la grille |
| `config` | le schéma de réglages, qui génère le formulaire ⚙ |

## Le formulaire de config

Le formulaire généré comprend quatre formes :

| Schéma | Contrôle |
| --- | --- |
| `Boolean` | interrupteur |
| `Color` | sélecteur de couleur |
| `Number` avec `min`/`max` | slider |
| `String` avec `enum` | liste déroulante |

::: danger Nommez vos booléens `showX`, jamais `hideX`
Un booléen non défini s'affiche à **ON** et se lit comme `!== false`. Une option
`hideFoo` afficherait donc un interrupteur allumé pendant que la chose est visible
— l'interrupteur mentirait sur son propre sens. Tous les widgets livrés utilisent
`showX`.
:::

Les libellés sont des clés i18n, résolues depuis votre dossier `lang/` :

```
widget.maboite.name
widget.maboite.config.<clé>
widget.maboite.options.<clé>.<valeur>     ← libellés des options d'énumération
```

::: warning Les libellés d'énumération ne vivent pas sous `config`
`widget.maboite.config.format` doit rester une **chaîne** — c'est le libellé du
champ lui-même. Les libellés d'options vont sous
`widget.maboite.options.format.auto`. Une clé qui résout vers un objet casse la
résolution de traduction.
:::

## Le pont

Votre widget parle à l'app par `postMessage`. Deux messages entrants, deux
sortants.

**Host → widget**

```js
{ source: "quicktys", type: "config", config: { /* vos réglages */ } }
{ source: "quicktys", type: "state",  state:  { /* l'état réactif */ } }
```

**Widget → host**

```js
{ source: "quicktys-widget", type: "ready" }
{ source: "quicktys-widget", type: "action",
  plugin: "countdown", action: "report-finished", config: { name, seq } }
```

Une action s'exécute comme une séquence à un nœud, exactement comme si elle était
dans une automatisation.

### Un widget minimal

```html
<!doctype html>
<html>
  <body>
    <div id="title">—</div>
    <script>
      let config = {};
      let state = {};

      function render() {
        document.body.style.color = config.textColor || "#fff";
        document.getElementById("title").textContent = state["pear.title"] || "—";
      }

      window.addEventListener("message", (e) => {
        const d = e.data;
        if (!d || d.source !== "quicktys") return;
        if (d.type === "config") config = d.config || {};
        else if (d.type === "state") state = d.state || {};
        render();
      });

      render();
      // Signaler qu'on écoute — le host répond avec la config et l'état.
      parent.postMessage({ source: "quicktys-widget", type: "ready" }, "*");
    </script>
  </body>
</html>
```

Envoyez `ready` **en dernier**, une fois votre écouteur attaché. Le host y répond
avec la config et l'état courants.

Les clés d'état arrivent **à plat et namespacées** : `state["pear.title"]`, pas
`state.pear.title`.

## Où ça tourne

Dans l'app, l'iframe charge depuis un schéma d'URI dédié
**`widget://localhost/<id>/`** (`http://widget.localhost/<id>/` sous Windows), donc
chaque widget est son propre document **sans CSP héritée** — c'est pour ça qu'un
`<script>` inline fonctionne.

Dans une source navigateur OBS, le même fichier est servi par le serveur HTTP
local de l'app et l'état arrive en SSE, converti dans la même forme
`postMessage`. C'est pour ça que le rendu est identique des deux côtés.

::: warning Aucun accès réseau
L'iframe ne peut rien récupérer. Une police web retombe silencieusement sur une
autre : **n'utilisez que des piles de polices système**. Idem pour les images et
les scripts : inlinez-les, ou prenez-les dans l'état.
:::

## Les pages d'aperçu

Un widget invisible au repos — une boîte d'alerte n'apparaît que déclenchée — ne
peut pas être positionné dans l'éditeur d'overlays. Livrez un `preview.html` :

- l'**éditeur d'overlays** charge `preview.html`,
- une **source navigateur OBS** charge toujours `index.html`.

L'aperçu reçoit le même message de `config` : il doit donc se mettre en scène avec
du contenu factice tout en respectant les couleurs et options de l'utilisateur.
`index.html` reste ainsi simple et inerte, avec la démo dans un fichier séparé.

## Interaction

Les widgets de l'éditeur d'overlays sont rendus **inertes** par
`pointer-events: none`, pour que l'éditeur garde tous les clics et qu'un aperçu ne
puisse jamais avaler un glisser. Sur le tableau de bord, ils sont pleinement
interactifs.

## Noms et autocomplétion

Un widget désigné par un nom — une boîte d'alerte, un compte à rebours — lit ce
nom dans sa propre config. Le côté plugin déclare `.suggest("<type de widget>")`
sur le champ correspondant pour que le champ de nom de l'action s'autocomplète avec
les widgets réellement placés, sur le tableau de bord **et** dans les overlays.

Pour qu'une nouvelle source de suggestion produise quelque chose, elle doit aussi
être câblée dans le fournisseur de suggestions du frontend. Une source non câblée
ne suggère rien, silencieusement.

## Deux endroits, deux formulaires

Les widgets se configurent dans **deux UI distinctes** avec du code séparé : le
formulaire du tableau de bord et le panneau ⚙ de l'éditeur d'overlays. Elles
rendent le même schéma, mais un réglage ajouté à l'une n'est pas automatiquement
dans l'autre.

Comme une page de module isolée n'a pas accès à l'i18n de l'app, l'éditeur
d'overlays résout les libellés côté host et les livre dans le message d'init.

## Les locales

```json
// widgets/maboite/lang/fr.json
{
  "name": "Ma boîte",
  "config": {
    "name": "Compte à rebours",
    "format": "Format",
    "showLabel": "Afficher le libellé",
    "textColor": "Couleur du texte"
  },
  "options": {
    "format": { "auto": "Automatique", "mm:ss": "Minutes et secondes" }
  }
}
```

## Les widgets built-in

Mettre `template` au lieu de `runtime`/`entry` fait dessiner le widget par un
composant Vue interne à l'app. C'est ainsi que fonctionnent la boîte d'info et le
bouton. Ce n'est pas disponible pour un widget tiers — il faudrait que le composant
existe dans l'app — donc le vôtre sera un widget runtime.

## Le tester

```bash
scripts/build-bundles.sh
setup/run.sh --solo
```

Tableau de bord → Édition → Ajouter un widget → le vôtre.

## Et ensuite

- [Traductions](/fr/dev/i18n)
- [Extensions livrées](/fr/reference/extensions) — le schéma de chaque widget
  livré.
