---
description: "Toutes les intégrations livrées avec Quicktys : Twitch, OBS Studio, Pear Desktop, Discord, son et synthèse vocale, overlays, alertes, comptes à rebours, entrées et commandes shell."
---

# Intégrations

Ce qui est livré aujourd'hui, ce que chaque chose demande, et ce qu'elle apporte.
Les listes exhaustives d'actions, triggers, état et services sont dans
[Extensions livrées](/fr/reference/extensions).

Rappel de la répartition : un **plugin** est ce que vous voyez dans l'éditeur ; un
**module** est le driver certifié auquel il parle. Vous installez le plugin ; le
module suit s'il est nécessaire.

## Twitch

**Plugin** `twitch` · **module** `twitch` (certifié, `net`)

| | |
| --- | --- |
| **Faire** | Envoyer un message dans le chat. Poster une annonce (avec une couleur). Suivre le chat d'une chaîne. Suivre les follows, subs et raids. |
| **Réagir à** | Message de chat. Nouveau follower. Nouvel abonné. Raid reçu. |
| **Lire** | `twitch.chat` (les 200 derniers messages), `twitch.chat-channel`, `twitch.badges` |
| **Demande** | Un compte pour envoyer et pour follow/sub/raid. L'affichage du chat ne demande rien. |

Le chat est le vrai flux IRC avec les tags IRCv3 : couleurs, noms d'affichage,
badges et positions d'emotes sont tous analysés, donc le widget de chat s'affiche
correctement plutôt qu'en texte brut.

Follows, subs et raids passent par EventSub. Il y a derrière des exigences de
forme très précises que le module gère pour vous — `channel.follow` doit être en
v2, et les abonnements doivent être créés dans les dix secondes suivant
l'ouverture du socket, sinon Twitch raccroche.

Voir [Connecter ses comptes](/fr/guide/accounts#twitch).

## OBS Studio

**Plugin** `obs` · **module** `obs` (certifié, `net`)

| | |
| --- | --- |
| **Faire** | Changer de scène. Afficher/masquer une source. Déplacer, redimensionner et pivoter une source. Activer un filtre. Couper le son. Régler une source texte. Changer l'URL d'un navigateur. Rafraîchir un navigateur. Capturer une source. Contrôler un média. Déclencher un raccourci. Démarrer/arrêter/basculer le stream, l'enregistrement, le replay buffer et la caméra virtuelle. Mettre l'enregistrement en pause et le reprendre. Sauvegarder le replay buffer. Mode studio et transitions. Ajouter une source navigateur. |
| **Réagir à** | Changement de scène. Stream démarré/arrêté. Enregistrement démarré/arrêté. Replay buffer démarré/arrêté/sauvegardé. Caméra virtuelle démarrée/arrêtée. Mode studio activé/désactivé. Média démarré/terminé. |
| **Lire** | `obs.scene`, `obs.scenes`, `obs.inputs`, `obs.filters`, `obs.hotkeys`, `obs.streaming`, `obs.recording`, `obs.replay-buffering`, `obs.virtual-cam`, `obs.studio-mode` |
| **Demande** | obs-websocket v5 (intégré à OBS 28+) activé, avec son URL et son mot de passe. |

L'état est rempli dès que la connexion est établie, pas paresseusement — c'est
pour ça que les champs de scène et de source **s'autocomplètent** avec vos vraies
scènes et sources.

Le plugin contribue aussi un bouton **Ajouter à OBS** dans la barre d'outils de
l'éditeur d'overlays : un clic crée la source navigateur de l'overlay en cours.

## Pear Desktop

**Plugin** `pear` · **module** `pear` (certifié, `net`)

| | |
| --- | --- |
| **Faire** | Lecture, pause, bascule. Piste suivante et précédente. Aimer, ne pas aimer. Aléatoire. Couper le son. Régler le volume. Se déplacer dans la piste. |
| **Réagir à** | Changement de musique. |
| **Lire** | `pear.title`, `pear.artist`, `pear.artists`, `pear.cover`, `pear.url`, `pear.album`, `pear.video-id`, `pear.lyrics`, `pear.playlist`, `pear.playing`, `pear.queue` |
| **Demande** | Pear Desktop en cours d'exécution, et une autorisation unique dans le lecteur. |

Les trois widgets Pear (Now Playing, Musique, File d'attente) sont pilotés par cet
état.

::: info La première observation ne déclenche jamais le trigger
Lancer Quicktys alors qu'une musique joue déjà synchronise `pear.*` mais ne
déclenche **pas** *Changement de musique*. Seul un changement depuis une musique
précédente connue le fait. Sinon chaque lancement déclencherait votre alerte de
musique.
:::

## Overlays

**Plugin** `overlays` · **module** `webview`

| | |
| --- | --- |
| **Faire** | Créer un overlay (ou en renommer un). Supprimer un overlay. Rafraîchir la liste. |
| **Lire** | `overlays.count`, `overlays.ids` |

Installer ça vous donne l'onglet Overlays et les pages `/overlay/<id>`. Voir
[Overlays](/fr/guide/overlays).

## Alertes

**Plugin** `alert` · widget `alert`

Une action, **Afficher une alerte**, qui prend un nom, un titre, un contenu et une
URL d'image. Elle pousse dans `alert.<nom>`, et le widget d'alerte abonné à ce nom
s'anime.

Le champ de nom s'autocomplète avec les widgets d'alerte réellement placés, sur le
tableau de bord **et** dans les overlays. Titre, contenu et image sont des champs
template : ils peuvent porter `@user` ou `{{ message }}`.

Mettez les alertes dans une [file d'action](/fr/guide/automations#files-d-action)
pour que deux followers simultanés ne se chevauchent pas.

## Compte à rebours

**Plugin** `countdown` · widget `countdown`

| | |
| --- | --- |
| **Faire** | Démarrer (ou redémarrer) un compte à rebours nommé. Ajouter du temps (négatif pour en retirer). Pause, reprise, arrêt. Signaler la fin par avance. |
| **Réagir à** | Compte à rebours terminé. |
| **Lire** | `countdown.<nom>` — `running`, `endAt`, `remainingMs`, `remaining`, `display`, `label` |

Les timers survivent à un redémarrage. Voir [la note sur qui fait tourner
l'horloge](/fr/guide/dashboard#comptes-a-rebours-qui-fait-tourner-l-horloge) — le
trigger *terminé* exige que le compte à rebours soit affiché quelque part.

## Son

**Plugin** `sound` · **module** `sound` (certifié, `audio`)

Jouer un fichier audio (wav, mp3, ogg, flac) à un volume choisi, décodé et mixé
par le host. Également la **synthèse vocale**, avec une voix ou une langue prise
parmi celles installées sur votre OS.

## Discord

**Plugin** `discord` · **module** `http` (certifié, `net`)

Poster un message dans un salon Discord via un webhook entrant, avec un nom
d'utilisateur et un avatar optionnels. Ni compte Discord ni bot nécessaires.

Notez qu'il n'y a pas de module spécifique à Discord : le plugin passe par le
module **HTTP** générique, réutilisable par tout plugin qui le déclare.

## Système d'exploitation

**Plugin** `os` · **module** `os` (certifié, `process`)

Exécuter une commande shell. Ouvrir un fichier ou une URL avec le gestionnaire par
défaut du système.

::: warning Celui-là exécute vraiment des commandes
`process` est la capacité qui permet à un module de lancer des programmes sur
votre machine. Il vaut la peine d'être délibéré au moment de la consentir, et sur
ce qu'on met dans le champ de commande — surtout si elle est construite depuis un
template qui inclut du texte de chat.
:::

## Entrées

**Plugin** `input` · **module** `input` (certifié, `input`)

Taper du texte, presser une combinaison de touches (`ctrl+shift+c`), cliquer avec
la souris. Demande une vraie session graphique.

## Notify

**Plugin** `notify` — pas de module.

Écrire une ligne de log, ou afficher un toast dans l'app. Les deux prennent un
message template et un niveau. Le plugin le plus simple possible, et bien utile
pour déboguer une séquence.

## Aléatoire

**Plugin** `random` — pas de module.

Nombre aléatoire dans un intervalle (entier ou non), lancer de dés, pile ou face,
tirage à un pourcentage, choix dans une liste. Liez le résultat et branchez
dessus.

## Temps

**Plugin** `time` — pas de module.

Une action de délai. Notez qu'il n'est **pas** installé par défaut : son `delay`
est une fixture de test de l'ABI qui n'attend pas réellement. Utilisez le nœud
**Attente**, qui est la vraie chose.

## Et ensuite

- [Extensions livrées](/fr/reference/extensions) — les tables exhaustives.
- [Écrire un plugin](/fr/dev/plugin) — ajouter le vôtre.
