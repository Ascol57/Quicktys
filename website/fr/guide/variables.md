# Variables & templating

Il y a **trois** endroits où l'on tape une expression dans Quicktys, et ils
n'utilisent pas tous le même langage. Le distinguer évite beaucoup de confusion.

| Où | Syntaxe | Langage |
| --- | --- | --- |
| Les **champs de config** d'une action (« Message », « Scène »…) | `@chemin` et `{{ }}` | lookup de variable + **Handlebars** |
| **Conditions et filtres** (`if`, `while`, `until`, filtres de trigger, activation de profil) | chemins nus, `@` optionnel | **Rhai** |
| **Valeurs** (`let`, `set`, `count` de `repeat`, `list` de `for-each`, `ms` de `delay`, valeur de `switch`) | chemins nus, `@` optionnel | **Rhai** |

La règle de pouce : **les champs qui produisent du texte utilisent le templating ;
les champs qui produisent une décision ou une valeur utilisent Rhai.**

## Le contexte

Tout ce que vous pouvez lire vient d'un seul contexte plat, assemblé à chaque
exécution :

```
{
  …le contexte du trigger,   // cette occurrence : user, message, viewers…
  …l'état plugins/modules,   // valeurs réactives : obs.scene, pear.title…
  …les variables locales,    // let / set dans l'automatisation
}
```

Les clés d'état sont namespacées **à plat** : la clé est littéralement la chaîne
`obs.scene`. La vue **Variables** liste toutes celles actuellement publiées, avec
leur valeur — le moyen le plus rapide de savoir comment quelque chose s'appelle
vraiment.

<Screenshot src="/screens/variables.png" caption="La vue Variables" />

## Le templating dans les champs d'action

### `@chemin` — une variable

```
@pear.title
Écoute en cours : @pear.title
```

`@` n'est reconnu qu'en **début de chaîne ou après une espace**, ce qui explique
que `contact@example.com` soit laissé tranquille.

Un `@chemin` **seul** — rien d'autre dans le champ — garde la valeur **typée**.
Une liste reste une liste, un nombre reste un nombre. C'est important pour les
champs qui ne sont pas des chaînes :

```
@pear.playlist          → la vraie liste
Titres : @pear.playlist → "Titres : a, b, c" (interpolé pour l'affichage)
```

### `{{ }}` — Handlebars

Pour tout ce qui dépasse le simple lookup, le champ est du
[Handlebars](https://handlebarsjs.com) :

```handlebars
{{upper pear.title}} — {{pear.artist}}
{{#if twitch.chat-channel}}on suit {{twitch.chat-channel}}{{/if}}
{{#each pear.playlist}}{{this}}, {{/each}}
{{truncate message 80}}
```

Les clés pointées sont imbriquées en arbre avant le rendu, donc `{{pear.title}}`
fonctionne bien que la clé soit plate. Une valeur absente rend une chaîne vide
plutôt qu'une erreur.

Le catalogue complet des helpers — maths, trigonométrie, texte, comparaison, et
les helpers de bloc intégrés — est dans [Helpers de
templating](/fr/reference/templating).

### Ordre et échappement

`@` est résolu **d'abord**, puis Handlebars. Un antislash rend le caractère
suivant littéral :

| Vous tapez | Vous obtenez |
| --- | --- |
| `\@pasunevariable` | `@pasunevariable` |
| `\{{ littéral }}` | `{{ littéral }}` |
| `\\` | `\` |

### Dans l'éditeur

Les champs template s'affichent avec un input surligné à autocomplétion : tapez
`@` pour les variables (alimentées par l'état déclaré par tous les plugins
chargés) ou `{{` pour le menu des helpers.

<Screenshot src="/screens/template-input.png" caption="Un champ template avec l'autocomplétion @" />

## Rhai dans les conditions et les valeurs

Conditions, filtres, compteurs de boucles et valeurs de `let`/`set` sont du
[Rhai](https://rhai.rs) — un petit langage embarqué à la syntaxe entre Rust et
JavaScript.

```rhai
obs.scene == "Intro"
viewers >= 50 && message.starts_with("!")
pear.title != ""
[1, 2, 3]
count * 2 + 1
```

Les deux orthographes d'une lecture d'état fonctionnent :

```rhai
obs.scene == "Intro"
@obs.scene == "Intro"    // identique — un @ en tête est retiré
```

C'est délibéré : ce que l'autocomplétion de l'éditeur dépose dans un champ
d'action se colle tel quel dans une condition.

### Le bac à sable

Rhai n'a ici ni système de fichiers, ni réseau, ni `eval`, ni chargement de
modules, et il a des limites strictes sur le nombre d'opérations, la profondeur
des expressions et la taille des valeurs. L'évaluation est **synchrone** et se
fait sur un contexte **déjà matérialisé** — aucune I/O n'a lieu pendant
l'évaluation, ce qui la rend déterministe et assez rapide pour tourner à chaque
changement d'état.

Les expressions n'appellent qu'une **bibliothèque standard fournie par le cœur**.
Les plugins ne peuvent pas injecter de fonctions : une fonction de plugin vivrait
dans son composant WebAssembly, et l'appeler en cours d'évaluation imposerait un
aller-retour ABI asynchrone à chaque fois. Les plugins exposent des **valeurs**
via l'état, à la place.

### Le piège

::: danger `=` n'est pas `==`, et les erreurs sont silencieusement fausses
En Rhai, `=` affecte et `==` compare. Toute erreur de compilation comme
d'évaluation est convertie en **`false`**. Une faute de frappe désactive donc une
liaison sans aucun message, nulle part.

Si une liaison, une condition ou une activation ne se déclenche jamais, vérifiez
d'abord l'expression — et le nom exact de la variable dans la vue Variables.
:::

## D'où viennent les valeurs

| Source | Exemple | Écrite par |
| --- | --- | --- |
| **Contexte du trigger** | `message`, `user`, `viewers` | l'occurrence du trigger |
| **État de plugin** | `pear.title`, `obs.scene`, `twitch.chat` | un plugin appelant `set-state` |
| **Locales** | `bonus`, `x` | vos propres `let` / `set` |
| **Résultats d'action** | ce que vous avez lié | le `bind` d'un nœud action |

Les plugins déclarent leur état dans leur manifeste, donc l'éditeur connaît les
noms avant même que quoi que ce soit ait tourné.
[Intégrations](/fr/guide/integrations) liste ce que publie chaque plugin livré.

## Réactivité

Parce qu'une expression est un arbre syntaxique, le host extrait les variables
qu'elle lit. Un `wait-for until` n'est réévalué que lorsqu'une de *ses*
dépendances change, pas à chaque écriture d'état.

Une conséquence à connaître : les dépendances sont des **identifiants racines**
(`obs`) alors que les clés d'état sont pointées (`obs.scene`) — une clé modifiée
est donc comparée à une dépendance par son segment racine.

## Et ensuite

- [Helpers de templating](/fr/reference/templating) — tous les helpers, avec leurs
  signatures.
- [Nœuds d'automatisation](/fr/reference/nodes) — quel champ de quel nœud parle
  quel langage.
