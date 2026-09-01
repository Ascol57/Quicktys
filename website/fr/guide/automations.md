# Automatisations

Une automatisation est un **arbre de nœuds typés**. Pas une liste d'actions avec
deux ou trois cas particuliers greffés dessus — un vrai arbre, exécuté par un
interpréteur du host avec annulation, concurrence structurée et boucles bornées.

On la rencontre à deux endroits : en ligne dans une liaison de
[profil](/fr/guide/profiles), et comme automatisation autonome dans l'onglet
**Automatisations**.

<Screenshot src="/screens/automation-editor.png" caption="L'éditeur d'automatisation" />

## L'éditeur

Cliquez sur **+** pour ouvrir la palette de nœuds — avec recherche, groupée par
catégorie.

<Screenshot src="/screens/node-palette.png" caption="La palette de nœuds" />

| Catégorie | Nœuds |
| --- | --- |
| **Core** | Action |
| **Structure** | Block, Parallel, Timeline |
| **Flow** | If, Switch, Loop, Wait, Try |
| **Vars** | Let, Set |
| **Jump** | Return, Break, Continue |

Les nœuds se réorganisent par glisser-déposer et s'imbriquent en les déposant
dans le corps d'un nœud conteneur. Pendant qu'une séquence tourne, le nœud en
cours s'allume — le host remonte de vrais événements de début et de fin pour
chaque nœud de premier niveau, donc le surlignage suit l'exécution réelle et non
une estimation des durées.

## Les nœuds

Référence champ par champ dans [Nœuds
d'automatisation](/fr/reference/nodes). En résumé :

### Action

Invoque l'action d'un plugin. Choisissez le plugin, l'action, remplissez le
formulaire — **généré depuis le schéma** que le plugin a déclaré pour cette
action.

Vous pouvez **lier** (bind) le résultat à un nom, qui devient une variable locale
disponible pour les nœuds suivants. Ça remplace l'ancien motif de « mapping de
résultat ».

### Block

Une liste ordonnée d'enfants partageant une portée de variables locales. C'est ce
qu'*est* une séquence à son niveau le plus haut.

### Parallel

Exécute ses branches en même temps, avec une **politique de jointure** :

| Jointure | Comportement |
| --- | --- |
| `all` | Attendre **toutes** les branches, puis continuer. |
| `any` | Continuer dès que la première branche finit ; les autres sont annulées. |
| `race` | Comme `any` — la première gagne, les perdantes sont interrompues. |

Chaque branche est un nœud complet : elle peut contenir des actions, des
conditionnelles et des boucles, pas seulement une action.

```yaml
- kind: parallel
  join: all
  branches:
    - kind: block
      body:
        - { kind: action, plugin: obs, action: start-record }
        - { kind: wait-for, wait: delay, ms: "3000" }
    - kind: action
      plugin: sound
      action: play
      config: { file: intro.mp3, volume: 80 }
# atteint seulement une fois que LES DEUX branches ont fini
- { kind: action, plugin: twitch, action: chat, config: { message: "C'est parti" } }
```

::: tip Le nœud qu'on ne voit pas venir
« Démarrer trois choses et attendre qu'elles finissent toutes » est un nœud de
première classe. Ce n'est pas un délai réglé à la main, et les perdantes d'un
`any` sont réellement annulées, pas laissées à tourner dans le vide.
:::

### Timeline

Des pistes décalées dans le temps depuis un départ commun — la forme visuelle du
glisser-sur-un-axe-temps, familière depuis CastMate. Chaque piste a un `offset_ms`
et un corps. Contrairement aux offsets de CastMate, le flow **rejoint** les pistes
à la fin au lieu de les lancer et de les oublier.

### If / Switch

Conditionnelles en ligne. `if` prend une condition et un `then`, éventuellement un
`else`. `switch` évalue une valeur et la confronte à des cas, avec un défaut
optionnel.

Les conditions sont des expressions Rhai. Voir [Variables &
templating](/fr/guide/variables).

### Loop

Trois formes, toutes avec un plafond `max` obligatoire :

| Boucle | Champs |
| --- | --- |
| `repeat` | `count` — une expression, donc le nombre peut être dynamique |
| `while` | `cond` — réévaluée à chaque passage |
| `for-each` | `var` + `list` — itère une expression qui produit une liste |

Le plafond n'est ni optionnel ni indicatif. C'est ce qui rend acceptable de
laisser n'importe qui écrire une boucle dans un logiciel qui tourne sans
surveillance pendant huit heures.

### Wait

Trois formes :

| Attente | Signification |
| --- | --- |
| `delay` | Dormir `ms` — une expression, donc les délais peuvent être calculés. |
| `until` | Bloquer jusqu'à ce qu'une condition sur l'état réactif devienne vraie, avec timeout. |
| `event` | Bloquer jusqu'au déclenchement d'un trigger, avec filtre optionnel et timeout. |

`until` est vraiment réactif : le host sait quelles clés d'état la condition lit
et ne réévalue que quand l'une d'elles change.

### Try

Enveloppe un corps avec un gestionnaire `catch` optionnel et une politique de
`retry` optionnelle (`max` tentatives, `delay_ms` entre elles). Sans `try`, une
action qui échoue termine le run en statut *failed*.

### Let / Set

`let` introduit une locale dans la portée du bloc courant ; `set` réaffecte une
locale existante. Les deux prennent une expression.

```yaml
- { kind: let, name: bonus, value: "viewers * 2" }
- { kind: if, cond: "bonus > 100", then: { kind: set, target: bonus, value: "100" } }
```

### Return / Break / Continue

`return` termine l'automatisation, éventuellement avec une valeur. `break` et
`continue` s'appliquent à la boucle la plus interne.

## Annulation

Chaque run porte un jeton d'annulation propagé à tous les nœuds. Interrompre un
run arrête les actions, réveille les attentes, casse les boucles et annule les
branches parallèles — rien ne reste à tourner dans le vide.

Une action annulée rapporte `aborted` plutôt que `failed`, donc le host ne la
présente pas comme une erreur.

## Files d'action

Certaines choses ne doivent pas se chevaucher. Deux alertes de follow simultanées,
ce sont deux sons l'un sur l'autre et deux animations d'overlay qui se battent.

Une **file d'action** est une ressource qui exécute les séquences **une à la
fois** :

- un **écart** configurable entre deux exécutions,
- **pause** et **reprise**,
- **skip**, qui interrompt le job courant et passe au suivant,
- **vider**,
- un **historique** rejouable.

Une automatisation vise soit une file, soit une exécution immédiate. Les alertes
appartiennent à une file ; un changement de scène, non.

## Où vivent les automatisations

`<user-data>/quicktys/automations/<id>.yaml`. Les expressions sont stockées sous
forme de **texte source** et compilées au chargement, donc une expression cassée
est une erreur visible avant d'être une surprise à l'exécution.

## Et ensuite

- [Variables & templating](/fr/guide/variables) — les deux syntaxes de template et
  le langage d'expression.
- [Référence des nœuds](/fr/reference/nodes) — chaque champ de chaque nœud.
