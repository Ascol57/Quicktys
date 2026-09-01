---
description: "Construire sa première automatisation Quicktys en dix minutes : un bouton de tableau de bord, un trigger lié à une séquence, puis une réaction au chat Twitch en direct."
---

# Première automatisation

Dix minutes, sans compte, sans OBS. On va faire en sorte qu'un bouton du tableau
de bord fasse quelque chose, puis que quelque chose venu d'internet fasse quelque
chose.

Le vocabulaire, d'abord :

- Un **trigger** est un événement — un clic sur un bouton, un message de chat, un
  changement de musique.
- Une **séquence** est ce qu'on en fait — un arbre de nœuds exécuté dans l'ordre.
- Un **profil** possède l'appariement : il lie des triggers à des séquences, et
  s'active ou se désactive d'un bloc.

## 1. Poser un bouton sur le tableau de bord

Ouvrez l'app. Vous arrivez sur le **Tableau de bord** — une grille vide.

1. Cliquez sur **Édition**, en haut à droite. La grille devient modifiable.
2. Cliquez sur **Ajouter un widget**. Une palette avec recherche s'ouvre.
3. Choisissez **Bouton**. Il se pose sur la grille.
4. Cliquez sur le ⚙ du widget pour mettre `Bonjour` comme **libellé**, et choisir
   une couleur.
5. Tirez la poignée en bas à droite pour le redimensionner ; glissez le widget
   pour le déplacer.

<Screenshot src="/screens/dashboard-edit.png" caption="Le tableau de bord en mode Édition, palette de widgets ouverte" />

Quittez le mode Édition. Cliquer sur le bouton ne fait rien — mais il émet
**déjà** un événement.

::: info Ce qui vient de se passer
Cliquer sur un widget bouton émet le trigger intégré `button.clicked` sur le bus
d'événements du host, avec le contexte `{ id, label }` où `id` est l'id
d'instance du widget. Aucun WebAssembly n'intervient : le host publie
directement. Personne n'écoute encore — c'est l'étape suivante.
:::

## 2. Le lier à une séquence

Toujours en mode Édition, le widget bouton propose un outil **Lier une action**.
Cliquez dessus.

Quicktys crée (ou rouvre) un profil nommé `button-<id>`, avec le trigger
`button.clicked` déjà lié et déjà filtré sur *ce* bouton (`id == "<id>"`), et
l'ouvre. Vous pouvez faire la même chose à la main : **Profils → Nouveau**, puis
**Ajouter un trigger**, choisir **Boutons → Bouton cliqué** dans le sélecteur, et
écrire le filtre vous-même.

<Screenshot src="/screens/profile.png" caption="Un profil : une condition d'activation et une liaison trigger" />

La liaison a une séquence vide. Ajoutez un nœud :

1. Cliquez sur **+** pour ouvrir la palette de nœuds.
2. Choisissez **Action**.
3. Dans le nœud, sélectionnez le plugin **Notify** et l'action **Afficher un
   toast**.
4. Tapez `Bonjour depuis Quicktys` dans le champ **Message**.

Quittez le profil. Cliquez sur votre bouton. Un toast apparaît.

<Screenshot src="/screens/getting-started-final.png" caption="Le toast déclenché par le bouton" />

Voilà toute la boucle : **clic → `emit_trigger` → le bus d'événements → le
dispatcher → votre séquence s'exécute.** Chaque trigger de Quicktys fonctionne
exactement comme ça, y compris ceux qui viennent des plugins.

## 3. Lui faire faire plus d'une chose

Retournez dans la séquence et ajoutez un second nœud — mais pas une action.

1. Ajoutez un nœud **Attente**. Mode **delay**, valeur `1000`.
2. Ajoutez un nœud **Action** : **Notify → Écrire dans les logs**, message
   `Toujours là`.

Relancez. Le toast apparaît, une seconde passe, une ligne arrive dans les logs.

Essayez un branchement. Ajoutez un nœud **If** entre les deux :

- **Condition** : `true`
- Dans **then**, déplacez votre action de log.

Le champ de condition est une expression [Rhai](https://rhai.rs), pas un
template. Il s'autocomplète sur l'état réactif de tous les plugins chargés :
tapez `@` pour voir. `true` est un bouchon ; `obs.scene == "Intro"` ou
`twitch.chat-channel != ""` sont les vraies choses.

::: warning `=` n'est pas `==`
En Rhai, `=` affecte et `==` compare. Une condition qui ne compile pas est
considérée comme **fausse**, silencieusement — une faute de frappe désactive donc
la liaison sans aucun message. Si une liaison ne se déclenche mystérieusement
jamais, vérifiez la condition en premier.
:::

## 4. Réagir à quelque chose de réel

Les boutons font une bonne première boucle mais une mauvaise démo. Regardons un
chat Twitch — ce qui ne demande **aucun compte**, Quicktys se connectant
anonymement pour le chat en lecture seule.

1. **Marketplace** → installez le plugin **Twitch**. Il tire le module `twitch`
   certifié comme dépendance et affiche un écran de consentement pour les
   capacités demandées (`net`).
2. Dans n'importe quelle séquence, lancez une fois l'action **Twitch → Suivre le
   chat d'une chaîne**, avec le nom de la chaîne. (Un bouton fait très bien
   l'affaire.)
3. Créez un nouveau profil avec le trigger **Twitch → Message de chat**.
4. Donnez-lui une action : **Notify → Afficher un toast**, message :

   ```
   @twitch.chat-channel : {{ message }}
   ```

Chaque message de cette chaîne lève désormais un toast. Deux syntaxes
apparaissent là, et ce ne sont pas les mêmes :

- `@twitch.chat-channel` lit une **variable d'état réactive** publiée par le
  plugin.
- `{{ message }}` lit un champ du **contexte du trigger** — la charge utile portée
  par cette occurrence précise.

Les deux sont détaillées dans [Variables &
templating](/fr/guide/variables).

::: tip Filtrez avant d'agir
Une liaison de trigger a un **filtre** — une expression Rhai évaluée sur le
contexte. `message.starts_with("!")` transforme « chaque message » en « seulement
les commandes ». Filtrer dans la liaison est plus économe et plus lisible qu'un
`if` en tête de séquence.
:::

## 5. Le mettre à l'antenne

Le chat en toasts, c'est pour vous, pas pour vos viewers. Pour le mettre sur le
stream :

1. Installez le plugin **Overlays** (il tire le module `webview`, qui stocke
   réellement les overlays et livre l'éditeur).
2. Ouvrez l'onglet **Overlays**, créez-en un, et posez le widget **Chat Twitch**
   sur le canvas. Il se rend pour de vrai, en direct, pendant que vous le placez.
3. Dans OBS, ajoutez une **source navigateur** pointant sur
   `http://localhost:8787/overlay/<id>`.

Le tour complet dans [Overlays](/fr/guide/overlays).

## Et ensuite

- [Profils & triggers](/fr/guide/profiles) — conditions d'activation, filtres, et
  pourquoi les profils existent.
- [Automatisations](/fr/guide/automations) — chaque type de nœud, les files, la
  concurrence structurée.
- [Variables & templating](/fr/guide/variables) — `@`, `{{ }}` et Rhai.
- [Intégrations](/fr/guide/integrations) — ce que sait faire chaque plugin livré.
