---
description: "Tous les helpers Handlebars des champs d'action Quicktys — maths, trigonométrie, texte, comparaison et helpers de bloc — plus la syntaxe @chemin et l'échappement."
---

# Helpers de templating

Les champs de config d'action qui acceptent le templating comprennent deux
syntaxes. `@` est résolu d'abord, puis Handlebars.

## `@chemin`

Un simple lookup de variable dans le contexte plat, reconnu uniquement en **début
de chaîne ou après une espace** — donc `contact@example.com` est laissé
tranquille.

```
@pear.title
Écoute en cours : @pear.title — @pear.artist
```

Un `@chemin` **seul** garde la valeur **typée** : une liste reste une liste, un
nombre reste un nombre. Mélangé dans une chaîne plus longue, il est interpolé sous
sa forme d'affichage.

## `{{ }}`

Du [Handlebars](https://handlebarsjs.com) complet. Les clés d'état pointées sont
d'abord imbriquées en arbre, donc `{{pear.title}}` fonctionne bien que la clé soit
plate. Une valeur absente rend une chaîne vide plutôt qu'une erreur.

## Échappement

Un antislash rend le caractère suivant littéral.

| Vous tapez | Vous obtenez |
| --- | --- |
| `\@pasunevariable` | `@pasunevariable` |
| `\{{ littéral }}` | `{{ littéral }}` |
| `\\` | `\` |

## Catalogue des helpers

Tous les helpers ci-dessous sont fournis par le cœur. Les plugins ne peuvent pas
en ajouter — voir [pourquoi](#pourquoi-les-plugins-ne-peuvent-pas-ajouter-de-helpers).

### Mathématiques

| Helper | Signature | Résultat |
| --- | --- | --- |
| `add` | `add a b` | a + b |
| `sub` | `sub a b` | a − b |
| `mul` | `mul a b` | a × b |
| `div` | `div a b` | a ÷ b |
| `mod` | `mod a b` | a % b |
| `idiv` | `idiv a b` | division entière |
| `pow` | `pow a b` | a ^ b |
| `min` | `min a b` | le plus petit |
| `max` | `max a b` | le plus grand |
| `neg` | `neg a` | −a |
| `abs` | `abs a` | \|a\| |
| `round` | `round a` | entier le plus proche |
| `floor` | `floor a` | arrondi inférieur |
| `ceil` | `ceil a` | arrondi supérieur |
| `sqrt` | `sqrt a` | racine carrée |
| `cbrt` | `cbrt a` | racine cubique |
| `exp` | `exp a` | e^a |
| `ln` | `ln a` | logarithme naturel |
| `log2` | `log2 a` | log base 2 |
| `log10` | `log10 a` | log base 10 |
| `log` | `log a base` | log dans une base quelconque |
| `hypot` | `hypot a b` | √(a²+b²) |

### Trigonométrie

| Helper | Signature | Résultat |
| --- | --- | --- |
| `sin` / `cos` / `tan` | `sin a` | en radians |
| `asin` / `acos` / `atan` | `asin a` | inverses |
| `atan2` | `atan2 y x` | arctan(y/x) |
| `sinh` / `cosh` / `tanh` | `sinh a` | hyperboliques |
| `rad` | `rad deg` | degrés → radians |
| `deg` | `deg rad` | radians → degrés |
| `pi` | `pi` | π |
| `tau` | `tau` | 2π |
| `e` | `e` | nombre d'Euler |

### Texte

| Helper | Signature | Résultat |
| --- | --- | --- |
| `upper` | `upper s` | MAJUSCULES |
| `lower` | `lower s` | minuscules |
| `capitalize` | `capitalize s` | Capitalisé |
| `trim` | `trim s` | retire les espaces autour |
| `truncate` | `truncate s n` | n premiers caractères, puis `…` |
| `replace` | `replace s de vers` | substitution |
| `default` | `default a b` | `b` si `a` est vide |
| `len` | `len v` | longueur d'une chaîne ou d'une liste |
| `join` | `join liste sep` | joint une liste |

### Comparaison et logique

À utiliser dans un `#if` :

| Helper | Signature |
| --- | --- |
| `eq` | `eq a b` |
| `ne` | `ne a b` |
| `gt` | `gt a b` |
| `lt` | `lt a b` |
| `gte` | `gte a b` |
| `lte` | `lte a b` |
| `and` | `and a b` |
| `or` | `or a b` |
| `not` | `not a` |

### Helpers de bloc

Ceux intégrés à Handlebars :

| Helper | Signature | Usage |
| --- | --- | --- |
| `if` | `#if cond` | bloc conditionnel |
| `unless` | `#unless cond` | conditionnel inversé |
| `each` | `#each liste` | boucle |
| `with` | `#with obj` | entrer dans la portée d'un objet |

## Exemples

```handlebars
{{upper pear.title}}

{{truncate message 80}}

{{default twitch.chat-channel "personne"}}

{{#if (gt viewers 50)}}Gros raid !{{/if}}

{{#each pear.playlist}}{{this}}, {{/each}}

{{round (mul progress 100)}} %
```

Les parenthèses imbriquent un appel de helper comme argument, comme en Handlebars
standard.

## Pourquoi les plugins ne peuvent pas ajouter de helpers

Une fonction fournie par un plugin vivrait dans son composant WebAssembly.
L'appeler en cours d'évaluation imposerait un aller-retour ABI asynchrone à chaque
appel — lent, et ça casserait le bac à sable synchrone qui rend l'évaluation
déterministe et assez économe pour tourner à chaque changement d'état.

Les plugins exposent des **valeurs** via l'état, à la place. La bibliothèque de
helpers est versionnée et étendue par les releases du cœur.

## Ce n'est pas Rhai

Cette page traite du **templating dans les champs de config d'action**. Les
conditions, les filtres et les champs de valeur sont du [Rhai](https://rhai.rs),
un autre langage avec une autre bibliothèque standard. `{{ }}` y est invalide.

Voir [Variables & templating](/fr/guide/variables) pour la répartition complète.

## Et ensuite

- [Nœuds d'automatisation](/fr/reference/nodes) — quel champ parle quel langage.
- [Types de schéma](/fr/reference/schema) — comment un champ devient templatable.
