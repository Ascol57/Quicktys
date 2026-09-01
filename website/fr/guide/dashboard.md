---
description: "Construire un tableau de bord Quicktys : ajouter, déplacer, redimensionner et configurer des widgets, le trigger Bouton, et pourquoi un compte à rebours est décompté par son widget."
---

# Tableau de bord

Le tableau de bord est une grille de **widgets** — les boîtes qui vous montrent ce
qui se passe et vous donnent des choses à cliquer. C'est l'onglet d'arrivée, et le
seul qu'on ne peut pas fermer.

<Screenshot src="/screens/dashboard.png" caption="Le tableau de bord" />

## Modifier la disposition

Cliquez sur **Édition**. La grille devient modifiable :

| Action | Comment |
| --- | --- |
| Ajouter | **Ajouter un widget** ouvre une palette avec recherche. |
| Déplacer | Glisser le widget. |
| Redimensionner | Tirer la poignée en bas à droite. Elle s'aligne sur des cellules entières — les widgets grandissent aussi en hauteur. |
| Configurer | Le ⚙ du widget ouvre son formulaire de réglages. |
| Retirer | Le ✕ du widget. |

<Screenshot src="/screens/dashboard-edit.png" caption="Le mode Édition, avec la palette de widgets ouverte" />

La disposition est stockée localement sur cette machine, pas dans vos profils.

## Configurer un widget

Chaque widget déclare un **schéma de configuration**, et le formulaire en est
généré. Il connaît quatre formes :

| Schéma | Contrôle |
| --- | --- |
| `Boolean` | un interrupteur |
| `Color` | un sélecteur de couleur |
| `Number` avec `min`/`max` | un slider |
| `String` avec `enum` | une liste déroulante |

<Screenshot src="/screens/widget-config.png" caption="Les réglages d'un widget" />

::: tip Un booléen non défini vaut ON
Un booléen non défini s'affiche à ON. Les options de widget s'appellent donc
`showX`, jamais `hideX` — sinon l'interrupteur mentirait sur son propre défaut.
:::

## Les widgets livrés

| Widget | Ce qu'il fait |
| --- | --- |
| **Boîte d'info** | Un compteur pour une source au choix : plugins, modules, profils, automatisations, widgets, overlays. |
| **Bouton** | Un gros bouton cliquable qui émet le trigger intégré `button.clicked`. |
| **Compte à rebours** | Un timer nommé, avec formats, couleurs, anneau de progression et seuil d'alerte. |
| **Alerte** | Une boîte d'alerte pilotée par le plugin Alerts — invisible tant qu'elle n'est pas déclenchée. |
| **Chat Twitch** | Le chat en direct, avec badges, emotes, couleurs et plusieurs styles de ligne. |
| **Now Playing (Pear)** | Pochette, titre et bouton lecture/pause. |
| **Musique (Pear)** | Une boîte plus riche : pochette, titre, artistes, barre de progression, minutage. |
| **File d'attente (Pear)** | La file de lecture, piste en cours surlignée. |

Tables de configuration complètes dans [Extensions
livrées](/fr/reference/extensions).

## Deux natures de widget

La différence ne se remarque pas à l'usage, mais elle explique pourquoi certains
widgets sont plus riches que d'autres :

- Les widgets **built-in** sont dessinés par un composant interne à l'app, choisi
  par le champ `template` de leur bundle (la boîte d'info et le bouton).
- Les widgets **runtime** livrent un `index.html` rendu dans une **iframe
  isolée**, chargée depuis un schéma d'URI `widget://` dédié pour que chacun soit
  son propre document avec sa propre CSP. Tout le reste de la liste est de ce
  type.

Les deux parlent à l'app par le même pont de messages : le host y pousse l'état et
la config, le widget peut demander au host de lancer une action.

## Le widget Bouton

Il mérite une section : c'est le moyen le plus économique d'obtenir un trigger
manuel.

Cliquer dessus émet le trigger intégré `button.clicked` avec le contexte
`{ id, label }`, où `id` est l'id d'instance du widget dans la disposition. Aucun
WebAssembly n'intervient — le host publie directement sur le bus d'événements.

En mode Édition, l'outil **Lier une action** du bouton crée (ou ouvre) un profil
nommé `button-<id>` avec le trigger déjà lié et déjà filtré sur ce bouton. À
partir de là, c'est une liaison de [profil](/fr/guide/profiles) ordinaire.

## Les noms, et d'où vient l'autocomplétion

Plusieurs plugins prennent un **nom** qui désigne un widget — le `name` du plugin
Alerts, celui du plugin Countdown. Ces champs s'autocomplètent avec les noms des
widgets correspondants réellement placés, collectés à la fois sur le tableau de
bord **et** dans vos [overlays](/fr/guide/overlays).

Si un champ de nom n'autocomplète rien, c'est que le widget n'est encore placé
nulle part.

## Comptes à rebours : qui fait tourner l'horloge

Le plugin Countdown détient des timers nommés et publie `countdown.<nom>`, mais le
host ne les fait **pas** tourner. La valeur publiée porte un instant de fin
absolu, et le widget en dérive le temps restant localement — un affichage précis à
la seconde, pour un coût nul côté host.

Ça a une conséquence visible : le trigger **Compte à rebours terminé** du plugin
est rapporté par le widget quand il franchit zéro. **Un compte à rebours que
personne n'affiche ne déclenche donc jamais ce trigger.** Si vous voulez le
trigger sans montrer le timer, placez le widget quelque part et masquez-le, ou
déclenchez la fin vous-même avec l'action *Signaler la fin du compte à rebours*.

Les timers survivent aux redémarrages. Un run expiré pendant que l'app était
fermée est restauré comme terminé et pré-acquitté, donc démarrer l'app ne
déclenche jamais un arriéré d'alertes.

## Et ensuite

- [Overlays](/fr/guide/overlays) — les mêmes widgets, à l'antenne.
- [Extensions livrées](/fr/reference/extensions) — les options de chaque widget.
