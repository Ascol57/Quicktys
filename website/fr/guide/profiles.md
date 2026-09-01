# Profils & triggers

Un **profil** est un ensemble nommé de liaisons trigger→séquence, qu'on peut
allumer et éteindre d'un bloc. C'est l'unité de « à quoi Quicktys réagit en ce
moment ».

Pourquoi ne pas lier les triggers globalement ? Parce qu'un stream a des modes.
En *Just Chatting* vous voulez des alertes de follow bavardes ; pendant un boss
vous les voulez silencieuses. Plutôt que d'éditer une douzaine de liaisons deux
fois par session, on les met dans deux profils et on laisse une **condition
d'activation** choisir.

<Screenshot src="/screens/profile.png" caption="Un profil : condition d'activation et liaisons de trigger" />

## Anatomie

```yaml
# profiles/gaming.yaml
id: gaming
name: Gaming
activation: 'obs.scene == "Game"'
bindings:
  - trigger: twitch.chat-message
    filter: 'message.starts_with("!")'
    sequence:
      kind: block
      body:
        - kind: action
          plugin: notify
          action: log
          config: { message: "commande de {{ user }} : {{ message }}" }
```

Trois pièces mobiles :

| | |
| --- | --- |
| **Activation** | Une expression Rhai. Tant qu'elle est fausse, aucune liaison du profil ne se déclenche. Laissez-la vide pour « toujours actif ». |
| **Trigger** | Quel événement écouter, sous la forme `<plugin>.<trigger>` — `twitch.follow`, `obs.scene-changed`, `button.clicked`. |
| **Filtre** | Une expression Rhai sur le **contexte** du trigger. La séquence ne tourne que s'il est vrai. |

Chaque liaison possède sa séquence **en ligne**. Un profil est autonome : il ne
pointe pas vers une automatisation par id. Ça le rend copiable et lisible d'un
bloc, au prix de ne pas partager une séquence entre deux triggers — pour ça,
utilisez une [automatisation](/fr/guide/automations) autonome.

## Choisir un trigger

Cliquez sur **Ajouter un trigger** : le sélecteur s'ouvre, avec une recherche sur
tous les triggers déclarés par les plugins chargés, groupés par plugin.

<Screenshot src="/screens/trigger-picker.png" caption="Le sélecteur de trigger" />

Le groupe intégré **Boutons** contient `button.clicked`, que le host émet
directement sans plugin dans la boucle. Tout le reste vient d'un plugin installé —
installez le plugin Twitch et ses quatre triggers apparaissent ici.

## Le contexte d'un trigger

Chaque occurrence porte un **contexte** : la charge utile décrivant *cette*
occurrence précise. Un message de chat porte l'utilisateur, le texte, les badges ;
un changement de musique porte le titre et l'artiste.

Le contexte est accessible de deux façons dans la liaison :

- dans le **filtre**, sous forme d'identifiants nus — `message`, `user` ;
- dans la **séquence**, sous forme de champs template — `{{ message }}`, ou
  `@message` pour la valeur typée.

Les plugins déclarent la forme de leur contexte dans leur manifeste, ce qui
alimente l'autocomplétion. Ce que porte un trigger donné est listé dans
[Intégrations](/fr/guide/integrations).

## Les filtres

Le filtre est le bon endroit pour restreindre un trigger. Il est évalué avant que
la séquence démarre et avant qu'une file d'action n'entre en jeu.

```rhai
// Uniquement les commandes
message.starts_with("!")

// Un seul bouton précis
id == "w3"

// Uniquement un gros raid
viewers >= 50

// Combiner — `&&` et `||` fonctionnent comme attendu
message.starts_with("!") && user != "monbot"
```

Filtres et conditions d'activation lisent aussi l'état réactif, avec ou sans le
sigle `@` :

```rhai
obs.scene == "Intro"
@obs.scene == "Intro"     // identique — le sigle est retiré
```

Les deux orthographes marchent pour que ce que l'autocomplétion de l'éditeur
insère dans un champ d'action se colle tel quel dans une condition.

::: danger Une expression cassée est silencieusement fausse
Les erreurs de compilation comme d'évaluation sont toutes deux converties en
`false`. Une faute de frappe — `=` au lieu de `==`, une variable qui n'existe pas
— désactive donc la liaison sans le moindre message, nulle part.

Si une liaison ne se déclenche jamais, soupçonnez le filtre avant tout le reste,
et vérifiez le nom exact de la variable dans la vue
[Variables](/fr/guide/interface#variables).
:::

## Les conditions d'activation

Même langage, portée différente : l'activation conditionne le profil entier.
Motifs courants :

```rhai
obs.streaming                      // uniquement en direct
obs.scene == "Game"                // un mode piloté par la scène
obs.streaming && obs.scene != "BRB"
```

Comme le moteur d'expressions extrait les variables lues, le host sait que
l'activation d'un profil dépend d'`obs` et ne la réévalue que lorsqu'une clé
`obs.*` change.

## Où vivent les profils

`<user-data>/quicktys/profiles/<id>.yaml`, un fichier par profil, écrit de façon
atomique. C'est du YAML simple : lisible, diffable, copiable d'une machine à
l'autre. Voir [Fichiers & dossier de données](/fr/reference/files).

Le dispatcher partage le store de profils avec l'UI, donc un profil édité prend
effet immédiatement — sans redémarrage ni rechargement.

## Et ensuite

- [Automatisations](/fr/guide/automations) — ce qu'on peut mettre dans une
  séquence.
- [Variables & templating](/fr/guide/variables) — les langages de template et
  d'expression en détail.
