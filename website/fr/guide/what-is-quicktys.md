# Qu'est-ce que Quicktys ?

Quicktys est une application de bureau qui réagit à ce qui se passe autour de
votre stream. Quelqu'un vous suit sur Twitch, une musique change, une scène
bascule dans OBS, vous cliquez sur un bouton de votre tableau de bord — et
Quicktys exécute une séquence que vous avez construite : changer de scène, jouer
un son, poster dans le chat, afficher une alerte sur l'overlay, attendre trois
secondes, faire deux choses à la fois.

L'idée n'est pas neuve. [CastMate](https://github.com/LordTocs/CastMate) l'a
défrichée, et Quicktys en est un descendant assumé sur le plan des concepts. Ce
qui est neuf, c'est tout ce qu'il y a dessous : Quicktys est une **réécriture
clean-room en Rust**, où chaque intégration est une **extension WebAssembly
isolée** plutôt qu'un module JavaScript avec les pleins pouvoirs.

## L'idée centrale : trois niveaux de confiance

La décision de conception qui façonne tout le produit tient en une phrase : **le
code que n'importe qui peut publier est celui qui peut faire le moins de
choses.**

| | Qui l'écrit | Bac à sable | Ce qu'il atteint |
| --- | --- | --- | --- |
| **Plugin** | n'importe qui | WebAssembly | Uniquement des capacités sûres : logs, son propre stockage clé/valeur, état réactif, appels de modules. **Ni réseau, ni disque, ni OS.** |
| **Module** | développeurs certifiés, signé | WebAssembly | Des capacités système — `net`, `input`, `audio`, `hid`, `files`, `process`, `timers` — et seulement celles qu'il a déclarées et que vous avez accordées. |
| **Widget** | n'importe qui | bac à sable navigateur | Rien. C'est du frontend dans une iframe isolée, alimentée par un pont de messages. |

Un plugin est la partie avec laquelle vous interagissez : il déclare des
**actions** (« envoyer un message dans le chat »), des **triggers** (« un nouveau
follower est arrivé ») et de l'**état** (« le titre en cours »). Un module est un
*driver* : il sait parler obs-websocket, ou l'IRC et Helix de Twitch, ou décoder
un MP3. Un widget est ce qui s'affiche sur votre tableau de bord ou à l'antenne.

Quand le plugin Twitch veut envoyer un message, il ne peut pas ouvrir de socket —
il n'en a pas le droit. Il appelle un service du module Twitch certifié, et le
host vérifie que le plugin a bien déclaré cette dépendance, que le service
existe, et que le payload correspond au schéma publié par le module. Chaque saut
est arbitré.

::: tip Ce que ça change concrètement
Un plugin malveillant — ou simplement buggé — ne peut pas exfiltrer votre clé de
stream, lire votre `~/.ssh`, ni installer quoi que ce soit. Le rayon d'action de
« j'ai installé un plugin trouvé sur un forum » est borné par conception, pas par
la confiance.
:::

## Ce que livre le cœur : rien

L'application cœur ne contient **aucun module et aucun widget**. Installer un
plugin fait résoudre au host les dépendances déclarées (`requires: obs@^1`) et
télécharger exactement ces modules certifiés, en vous montrant un écran de
consentement pour les capacités système demandées. Rien que vous n'ayez demandé
n'est jamais récupéré, et un module dont plus aucun plugin ne se sert peut être
retiré.

## Ce qui change par rapport à CastMate

Quicktys garde les concepts que CastMate a bien vus — profils, triggers,
séquences, files d'action, timeline visuelle — et refait les parties qui
atteignaient leur plafond.

**Un vrai modèle d'automatisation.** Une séquence CastMate est une liste plate
d'actions avec quelques cas particuliers. Une automatisation Quicktys est un
*arbre de nœuds typés* : `if`, `switch`, boucles bornées, `try`/retry,
`wait-for`, variables locales avec `let` et `set`, et `parallel` avec une
politique de jointure. « Démarrer trois choses, attendre qu'elles finissent
toutes, puis continuer » est un nœud de première classe, pas une bidouille.

**Des expressions analysables.** CastMate évalue ses templates en compilant du
JavaScript arbitraire. Quicktys utilise [Rhai](https://rhai.rs), sandboxé et
évalué côté host sur un contexte déjà matérialisé. Parce qu'une expression est un
arbre syntaxique et non du JS opaque, le host sait quelles variables elle lit —
ce qui est précisément ce qui rend possibles la réévaluation réactive ciblée, la
validation à la saisie et l'autocomplétion.

**Des traductions partout.** Chez CastMate, seuls les plugins avaient des
locales, importées dans le bundle JS. Chez Quicktys, le cœur, chaque plugin,
chaque module et chaque widget porte un dossier `lang/` de JSON par locale,
découvert et namespacé automatiquement par le host. Les manifestes émettent des
**clés**, jamais des chaînes traduites, et les locales voyagent en données à côté
du `.wasm` — un traducteur ne touche jamais au code.

**Une marketplace prévue dès le départ.** Paquets signés, registre statique signé
et auto-hébergeable, résolution de dépendances, licences vérifiables hors-ligne,
consentement par capacité qui prend effet immédiatement, chaînes de signature
hybrides post-quantiques. Le gratuit et le payant ont été conçus dès le premier
jour, même si le lancement se fera gratuit.

## Ce que Quicktys n'est pas

- **Pas un fork de CastMate.** Il ne partage volontairement aucune ligne de code
  avec lui. Les concepts et les APIs ont été étudiés, jamais le code. C'est cette
  contrainte qui explique que le frontend soit une app Vue 3 + PrimeVue écrite de
  zéro plutôt qu'une réutilisation de la bibliothèque d'UI de CastMate.
- **Pas un logiciel cloud.** Il fonctionne intégralement hors-ligne, sans compte.
  Un compte ne débloque que des suppléments optionnels — sync multi-machine,
  achats sur la marketplace, relais distant. Aucune fonctionnalité locale n'est
  verrouillée derrière, et il n'y a pas de télémétrie obligatoire.
- **Pas fini.** Voir la [roadmap](/fr/contributing/roadmap) pour l'état de chaque
  jalon.

## Et ensuite

- [Installation](/fr/guide/installation) — le faire tourner chez vous.
- [Première automatisation](/fr/guide/getting-started) — dix minutes.
- [Le modèle d'extension](/fr/dev/overview) — si vous préférez construire
  qu'utiliser.
