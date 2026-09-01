---
description: "Visite de la fenêtre Quicktys — tableau de bord, profils, automatisations, variables, marketplace et réglages — et pourquoi chaque formulaire est généré depuis un schéma."
---

# L'interface

Quicktys est une application de bureau à onglets. Une barre latérale à gauche
liste les sections ; cliquer sur l'une d'elles l'ouvre en onglet, et tous les
onglets sauf le tableau de bord peuvent être fermés.

<Screenshot src="/screens/sidebar.png" caption="La barre latérale, dossier Profils déplié" />

## La barre latérale

| Section | Ce qu'on y trouve |
| --- | --- |
| **Tableau de bord** | Votre grille de widgets. Toujours ouvert, jamais fermable. |
| **Profils** | Un *dossier* : il se déplie pour lister vos profils, chacun s'ouvrant dans son propre onglet. |
| **Automatisations** | Des séquences autonomes, éditables et exécutables seules. |
| **Variables** | Toutes les valeurs réactives publiées par les plugins et modules chargés. |
| **Marketplace** | Parcourir et installer plugins et widgets. |
| **Réglages** | Langue, comptes, consentement des capacités, clés de licence. |

Un **module** installé peut ajouter son propre onglet. Ce n'est pas un cas
particulier codé en dur : un module déclare une `view` dans son manifeste, et
l'app rend la page HTML qu'il livre dans une iframe isolée. L'onglet **Overlays**
qui apparaît après l'installation du plugin Overlays est exactement ça —
contribué par le module `webview`, pas codé dans l'app.

## Tableau de bord

L'onglet d'accueil : une grille où vous placez des widgets. En mode **Édition**
vous pouvez les ajouter, déplacer, redimensionner et configurer ; la disposition
est stockée localement. Tour complet dans [Tableau de
bord](/fr/guide/dashboard).

<Screenshot src="/screens/dashboard.png" caption="Le tableau de bord" />

## Profils

Un profil est un ensemble nommé de liaisons trigger→séquence, avec une
**condition d'activation** qui allume ou éteint tout le lot. Les profils sont ce
qui permet d'avoir un jeu de réactions « Just Chatting » et un jeu « Gaming »
sans qu'ils se marchent dessus.

Chaque liaison possède sa propre séquence en ligne — un profil *possède* ses
séquences plutôt que de référencer des automatisations par id. Voir [Profils &
triggers](/fr/guide/profiles).

## Automatisations

Des séquences autonomes, non rattachées à un trigger. Utile pour ce qu'on lance à
la main, ce qu'on réutilise depuis un bouton, ou ce qu'on construit et teste
avant de le câbler.

L'éditeur est le même que celui des profils : un canvas de nœuds avec palette de
recherche, réorganisation par glisser-déposer, et **surlignage d'exécution en
direct** — pendant qu'une séquence tourne, le nœud en cours s'allume, piloté par
de vrais événements de début et de fin remontés par le runner du host, pas par une
estimation côté client.

<Screenshot src="/screens/automation-editor.png" caption="L'éditeur d'automatisation" />

## Variables

Toutes les valeurs publiées par les extensions chargées, à plat et namespacées
par id d'extension : `obs.scene`, `pear.title`, `twitch.chat`, `countdown.timer`.
C'est le même contexte que lisent vos conditions et vos templates — donc le moyen
le plus rapide de savoir comment une variable s'appelle vraiment et quelle forme
elle a en ce moment.

<Screenshot src="/screens/variables.png" caption="La vue Variables" />

L'état est namespacé **à plat** : la clé est bien la chaîne `obs.scene`. Le moteur
d'expressions imbrique les clés pointées en arbre avant d'évaluer, ce qui fait que
`obs.scene == "Intro"` fonctionne comme un chemin bien que la clé ne soit pas un
identifiant.

## Marketplace

Parcourir, installer, mettre à jour et retirer des **plugins et des widgets**.
Les modules ne sont volontairement **pas** parcourables : ils n'arrivent que
tirés par un plugin qui les déclare, pour que vous n'installiez jamais un driver
dont vous n'avez pas l'usage. Voir [Marketplace](/fr/guide/marketplace).

## Réglages

<Screenshot src="/screens/settings.png" caption="Les réglages" />

- **Langue** — la locale de l'UI. Chaque extension contribue ses traductions.
- **Comptes** — connecter Twitch et tout ce qui demande OAuth. Voir [Connecter ses
  comptes](/fr/guide/accounts).
- **Capacités** — revoir et révoquer ce que chaque module installé a le droit de
  faire. Une révocation prend effet immédiatement.
- **Licences** — coller une clé pour débloquer une extension payante.
- **Réglages d'extension** — chaque plugin et module qui déclare un schéma de
  réglages obtient ici un formulaire généré.

## Les formulaires sont générés, pas écrits

Partout où vous remplissez les champs d'une action, les réglages d'un plugin ou
la configuration d'un widget, ce formulaire a été **généré depuis un schéma**
déclaré par l'extension. C'est pour ça que toutes les extensions se ressemblent,
et qu'un champ peut être un slider, un sélecteur de couleur, une durée, une liste
déroulante ou un chemin de fichier sans que l'extension livre la moindre ligne
d'UI.

Les champs qui acceptent des templates s'affichent avec un input surligné à
autocomplétion `@` et `{{ }}`, alimenté par les variables d'état déclarées par
tous les plugins chargés.

<Screenshot src="/screens/template-input.png" caption="Un champ template avec l'autocomplétion @ ouverte" />
