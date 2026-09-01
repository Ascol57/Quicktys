# Connecter ses comptes

Certaines choses ont besoin de savoir qui vous êtes. Lire un chat Twitch, non ; y
poster, oui — et connaître ses followers aussi.

<Screenshot src="/screens/settings-accounts.png" caption="Réglages → Comptes" />

## Twitch

### Ce qui marche sans compte

**L'affichage du chat en direct marche sans aucun compte.** Le module Twitch se
connecte en utilisateur anonyme, en lecture seule, ce qui suffit largement à un
*affichage* de chat — aucun jeton ne circule et le widget fonctionne d'emblée.

Ce qui demande un compte :

| Fonctionnalité | Compte requis |
| --- | --- |
| Afficher le chat dans un widget ou un overlay | non |
| Le trigger `twitch.chat-message` | non |
| Envoyer un message ou une annonce | **oui** |
| Les triggers follow / sub / raid (EventSub) | **oui** |
| Les images de badges dans le widget de chat | **oui** (un chat anonyme n'a pas de jeton, donc les badges retombent sur des pastilles de texte colorées) |

### Il vous faut votre propre Client ID

Quicktys ne livre pas d'identifiant d'application Twitch. Vous en enregistrez un —
ça prend deux minutes et c'est gratuit.

1. Allez sur [dev.twitch.tv/console/apps](https://dev.twitch.tv/console/apps) et
   cliquez sur **Register Your Application**.
2. **Nom** : ce que vous voulez, mais il doit être unique parmi toutes les apps
   Twitch. S'il est refusé, ajoutez un suffixe.
3. **OAuth Redirect URL** : `http://localhost`.
4. **Catégorie** : n'importe quoi de cohérent, par exemple *Application
   Integration*.
5. Créez, puis copiez le **Client ID**.

<Screenshot src="/screens/twitch-app-registration.png" caption="Enregistrer une application sur dev.twitch.tv" />

::: info Deux choses qui semblent fausses mais ne le sont pas
L'URL de redirection n'est **jamais utilisée**. Quicktys utilise le flux Device
Code, qui n'a pas de redirection. Le formulaire de Twitch exige le champ : mettez
`http://localhost` et oubliez-le.

Vous n'avez **pas** besoin d'un client secret. N'en générez pas, et si c'est déjà
fait, Quicktys ne vous le demandera pas.
:::

### Se connecter

**Réglages → Comptes → Twitch** : collez votre Client ID, cliquez sur
**Connecter**.

Une boîte de dialogue affiche un **code utilisateur** court et une URL. Ouvrez
l'URL — sur cette machine ou sur votre téléphone — connectez-vous, entrez le code,
approuvez. La boîte se ferme d'elle-même dès que Twitch confirme.

<Screenshot src="/screens/twitch-device-code.png" caption="La boîte de dialogue du flux device code" />

### Pourquoi le device code

Le flux Authorization Code de Twitch exige un **client secret**, qui ne peut pas
voyager en sécurité dans un binaire distribué — et Twitch ne supporte pas PKCE,
qui est la façon normale de contourner ça. Le flux Implicit ne rend pas de refresh
token : vous vous reconnecteriez en permanence.

Le Device Code ne demande qu'un client ID **public**, rend un refresh token, et
n'exige ni serveur loopback ni URL de redirection déclarée. C'est le seul des
trois qui convient à une application de bureau que n'importe qui peut
télécharger.

### Les scopes

Demandés à la connexion :

| Scope | Pour |
| --- | --- |
| `moderator:read:followers` | le trigger follow (EventSub `channel.follow` v2) |
| `channel:read:subscriptions` | le trigger subscribe |
| *(aucun)* | les raids — `channel.raid` n'a besoin d'aucun scope |

### Les jetons sur le disque

Les jetons sont stockés dans `<user-data>/quicktys/oauth.json`, en `0600` sous
Unix. Le jeu de jetons **et** la configuration de fournisseur nécessaire au
rafraîchissement sont persistés, donc un redémarrage ne vous déconnecte pas. Un
jeton mort et non rafraîchissable est jeté plutôt que rechargé en faux
« connecté ».

::: warning Du JSON en clair, pour l'instant
Le fichier est protégé des autres utilisateurs de la machine, pas de ce qui tourne
sous votre propre compte. Passer au trousseau de l'OS est l'étape suivante.
Traitez-le comme n'importe quel fichier de secrets.
:::

**Déconnecter**, dans les Réglages, oublie le jeu de jetons.

## OBS Studio

OBS utilise un mot de passe, pas OAuth.

1. Dans OBS : **Outils → Réglages du serveur WebSocket**. Activez le serveur,
   notez le port (4455 par défaut) et le mot de passe.
2. Dans Quicktys : **Réglages → OBS**, remplissez l'URL (`ws://127.0.0.1:4455`) et
   le mot de passe.

Une fois connecté, l'état du plugin OBS se remplit immédiatement — scène courante,
liste des scènes et des sources, statut du stream et de l'enregistrement, profil,
résolution, FPS et compteurs de performance. Tout est disponible pour les
conditions et les templates, et les sélecteurs de scène et de source
s'autocomplètent dessus.

## Pear Desktop

Pear Desktop (un lecteur YouTube Music) s'authentifie avec un jeton émis par le
lecteur lui-même.

La première fois que Quicktys le demande, **le lecteur affiche une boîte de
dialogue Autoriser / Refuser à l'écran**. Approuvez une fois : la
ré-authentification est silencieuse ensuite.

::: warning Si vous cliquez sur Refuser
Le refus est **persistant**. Quicktys ne redemandera pas de lui-même —
délibérément, parce qu'il interroge le lecteur toutes les trois secondes et qu'une
relance naïve vous ferait apparaître une boîte de dialogue toutes les trois
secondes, indéfiniment. Utilisez l'action **Connecter** pour redemander quand vous
êtes prêt.
:::

Si vous pointez Quicktys sur une autre instance du lecteur, ou si vous changez son
client id, le jeton est jeté et le consentement redemandé — une autre instance est
une autre identité.

## Discord

Aucun compte nécessaire. Le plugin Discord poste via un **webhook entrant** :
créez-en un dans les réglages du salon de votre serveur, et collez l'URL dans les
réglages du plugin (ou dans une action individuelle).

## Et ensuite

- [Intégrations](/fr/guide/integrations) — ce que sait faire chaque plugin une
  fois connecté.
- [Dépannage](/fr/guide/troubleshooting) — quand ça ne marche pas.
