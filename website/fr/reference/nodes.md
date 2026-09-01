---
description: "Référence de chaque nœud d'automatisation Quicktys : action, block, parallel, timeline, if, switch, loop, wait-for, try, let, set, return, break et continue."
---

# Nœuds d'automatisation

Une automatisation est un arbre de nœuds typés, sérialisé en YAML avec un
discriminant `kind`, et exécuté par un interpréteur tree-walking dans
`quicktys-core`.

Les expressions sont stockées sous forme de **texte source** et compilées au
chargement : une expression cassée est donc une erreur visible avant d'être une
surprise à l'exécution.

## Quel champ parle quel langage

C'est le point à ne pas se tromper :

| Champ | Langage |
| --- | --- |
| les valeurs de **config** d'une action | templating — `@` et `{{ }}` |
| `if.cond`, `loop.cond`, `wait-for.cond`, un **filtre** de trigger, une **activation** de profil | **Rhai** |
| `let.value`, `set.value`, `switch.value`, `loop.count`, `loop.list`, `wait-for.ms` | **Rhai** |
| le champ `equals (JSON)` d'un cas de `switch` | du JSON, pas une expression |

Dans l'éditeur, chaque champ Rhai est un input de template en **mode expression** :
l'autocomplétion `@` fonctionne toujours et un `@` en tête est retiré, mais le menu
des helpers `{{ }}` est masqué, puisqu'il y serait invalide.

## `action`

```yaml
- kind: action
  plugin: obs
  action: set-scene
  config: { scene: "Intro" }
  bind: result        # optionnel
```

| Champ | |
| --- | --- |
| `plugin` | l'id du plugin |
| `action` | l'id de l'action |
| `config` | la config de l'action, templatée et validée contre son schéma |
| `bind` | nom optionnel auquel lier la valeur de retour |

`bind` remplace l'ancien motif de « mapping de résultat » : le nom lié est une
locale ordinaire, utilisable par tous les nœuds suivants dans la portée.

## `block`

```yaml
- kind: block
  body:
    - { kind: action, plugin: notify, action: log, config: { message: "un" } }
    - { kind: action, plugin: notify, action: log, config: { message: "deux" } }
```

Des enfants ordonnés partageant une portée de variables locales. Une séquence est
un bloc à son niveau le plus haut.

## `parallel`

```yaml
- kind: parallel
  join: all           # all | any | race
  branches:
    - { kind: block, body: [ ... ] }
    - { kind: action, plugin: sound, action: play, config: { file: intro.mp3 } }
```

| Jointure | Comportement |
| --- | --- |
| `all` | Attendre **toutes** les branches, puis continuer. |
| `any` | Continuer dès que la première branche finit ; annuler les autres. |
| `race` | La première gagne, les perdantes sont interrompues. |

Chaque branche est un nœud complet : elle peut contenir des actions, des
conditionnelles et des boucles.

## `timeline`

```yaml
- kind: timeline
  tracks:
    - offset_ms: 0
      body: { kind: action, plugin: sound, action: play, config: { file: sting.mp3 } }
    - offset_ms: 1500
      body: { kind: action, plugin: obs, action: set-scene, config: { scene: "Game" } }
```

Des pistes décalées depuis un départ commun — la forme visuelle du
glisser-sur-un-axe-temps. Le flow **rejoint** les pistes à la fin au lieu de les
lancer et de les oublier.

## `if`

```yaml
- kind: if
  cond: 'obs.scene == "Intro"'
  then: { kind: block, body: [ ... ] }
  otherwise: { kind: block, body: [ ... ] }     # optionnel
```

## `switch`

```yaml
- kind: switch
  value: "tier"
  cases:
    - { pattern: "1", body: { ... } }
    - { pattern: "2", body: { ... } }
  default: { ... }
```

## `loop`

Trois formes, chacune avec un `max` obligatoire :

```yaml
- kind: loop
  loop: repeat
  count: "3"
  max: 100
  body: { ... }

- kind: loop
  loop: while
  cond: "x < 10"
  max: 100
  body: { ... }

- kind: loop
  loop: for-each
  var: item
  list: "pear.playlist"
  max: 100
  body: { ... }
```

`count`, `cond` et `list` sont des expressions : compteurs et listes peuvent donc
être calculés.

::: info `max` n'est pas indicatif
Chaque boucle est bornée. C'est ce qui rend acceptable de laisser n'importe qui
écrire une boucle dans un logiciel qui tourne sans surveillance pendant huit
heures.
:::

## `wait-for`

```yaml
- { kind: wait-for, wait: delay, ms: "1000" }

- kind: wait-for
  wait: until
  cond: "obs.streaming"
  timeout_ms: 30000

- kind: wait-for
  wait: event
  trigger: twitch.chat-message
  filter: 'message == "!go"'
  timeout_ms: 60000
```

| Attente | |
| --- | --- |
| `delay` | Dormir `ms` — une expression, donc les délais peuvent être calculés. |
| `until` | Bloquer jusqu'à ce qu'une condition sur l'état réactif devienne vraie. |
| `event` | Bloquer jusqu'au déclenchement d'un trigger, avec filtre optionnel. |

`until` est vraiment réactif : le host sait quelles clés d'état la condition lit et
ne réévalue que quand l'une d'elles change.

::: tip Utilisez `wait-for delay`, pas l'action de délai d'un plugin
L'action `delay` du plugin `time` est une fixture d'ABI pure qui n'attend **pas**
réellement — c'est pourquoi elle n'est pas livrée par défaut. Les vrais délais sont
ce nœud.
:::

## `try`

```yaml
- kind: try
  body: { ... }
  catch: { ... }                          # optionnel
  retry: { max: 3, delay_ms: 1000 }       # optionnel
```

Sans `try`, une action qui échoue termine le run en statut *failed*.

## `let` et `set`

```yaml
- { kind: let, name: bonus, value: "viewers * 2" }
- { kind: set, target: bonus, value: "100" }
```

`let` introduit une locale dans la portée du bloc courant ; `set` réaffecte une
locale existante.

## `return`, `break`, `continue`

```yaml
- { kind: return, value: "bonus" }    # valeur optionnelle
- { kind: break }
- { kind: continue }
```

`return` termine l'automatisation. `break` et `continue` s'appliquent à la boucle
la plus interne.

## Sémantique d'exécution

**Annulation.** Un jeton d'annulation est propagé à tous les nœuds. Une
interruption arrête les actions, réveille les attentes, casse les boucles et
annule les branches parallèles — rien ne reste à tourner dans le vide. Une action
annulée rapporte `aborted`, que le host ne présente pas comme une erreur.

**Concurrence structurée.** `parallel` attend selon sa politique de jointure ; les
perdantes d'un `any`/`race` sont réellement annulées, pas orphelines.

**Bornes.** Chaque boucle a un `max`, chaque `wait-for` un timeout, et il existe un
budget de temps global. Aucun flow n'est non borné.

**Erreurs.** Une action qui échoue remonte. `try` l'intercepte, avec une politique
de retry optionnelle. Non interceptée, le run se termine en *failed*.

**Progression.** Le runner encadre chaque nœud de premier niveau par de vrais
événements de début et de fin, que l'IPC relaie en
`NodeProgress { automation, path, running }`. C'est ce qui allume la carte en cours
d'exécution dans l'éditeur — l'exécution réelle, pas une estimation.

## Persistance

`<user-data>/quicktys/automations/<id>.yaml`, et en ligne dans les liaisons d'un
profil pour les séquences possédées par un profil. Écriture atomique (fichier
temporaire, puis renommage).

## En venant de CastMate

| CastMate | Quicktys |
| --- | --- |
| `InstantAction` | `action` |
| `ActionStack` (Promise.all) | `parallel { join: all }` |
| `TimeAction` + offsets | `timeline { tracks }` |
| `FlowAction` + `subFlows` | `if` / `switch` |
| `resultMapping` | `action { bind }` + `let` |
| *(rien)* | `loop`, `try`/retry, `wait-for`, `set`, portées lexicales |

L'[outil de migration](/fr/reference/cli#quicktys-migrate) transpose l'ancien
modèle sur le nouveau et rapporte ce qu'il n'a pas su convertir plutôt que de
deviner.

## Et ensuite

- [Helpers de templating](/fr/reference/templating)
- [Variables & templating](/fr/guide/variables)
