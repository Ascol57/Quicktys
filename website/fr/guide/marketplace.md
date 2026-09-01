---
description: "Installer plugins et widgets depuis un registre Quicktys signé : résolution de dépendances, vérification de signature, consentement des capacités, mises à jour et licences payantes."
---

# Marketplace

L'onglet Marketplace est l'endroit où l'on récupère plugins et widgets. Il parle à
un **registre** — un index statique signé, servable depuis un bucket, un CDN, ou
votre propre machine.

<Screenshot src="/screens/marketplace.png" caption="La Marketplace" />

## Ce qu'on peut parcourir

**Uniquement des plugins et des widgets.** Les modules ne sont volontairement pas
parcourables : un module est un driver, et il n'y a aucune raison d'en installer
un tout seul. Les modules arrivent tirés par un plugin qui les déclare, et
repartent quand plus rien n'en a besoin.

C'est pour ça que le système d'overlays a un plugin **Overlays** en façade alors
que le vrai travail est dans le module `webview` — sans un plugin le requérant, le
module ne pourrait jamais être installé du tout.

## Installer

Choisir un plugin déclenche une petite machine à états :

1. **Résolution.** Le registre calcule l'ensemble complet des dépendances — le
   plugin plus les modules certifiés qu'il déclare (`obs@^1`), en profondeur
   d'abord, en refusant les cycles.
2. **Téléchargement.** Chaque paquet est récupéré par empreinte de contenu et
   vérifié contre elle.
3. **Vérification.** La chaîne de signature de chaque paquet est contrôlée :
   autorité → certificat d'éditeur → release, chaque maillon signé, contre des
   clés CA épinglées et une liste de révocation. Une signature invalide **refuse**
   l'installation. Un paquet non signé s'installe mais reste *non certifié* — ce
   qui veut dire que ses capacités système seront refusées au chargement.
4. **Consentement.** Si un module demande des capacités système, on vous montre
   lesquelles et pourquoi avant d'accorder quoi que ce soit.
5. **Installation.** Les fichiers sont extraits de façon atomique dans
   `installed/`, et les extensions sont chargées à chaud — les modules d'abord,
   puis les plugins qui en dépendent. Sans redémarrage.

<Screenshot src="/screens/marketplace-consent.png" caption="L'écran de consentement des capacités" />

## Certification et consentement sont deux choses différentes

Ça se confond souvent, alors disons-le simplement :

- La **certification** décide si un module *peut* toucher une capacité. C'est
  cryptographique, et ce n'est pas votre décision — la signature se vérifie contre
  une autorité épinglée, ou non.
- Le **consentement** décide si *vous* l'autorisez. C'est votre décision, stockée
  sur votre machine, et révocable.

Les deux doivent tenir à **chaque appel** : `accordé = certifié && déclaré &&
consenti`. Révoquer une capacité prend effet immédiatement, pas au prochain
redémarrage.

Une mise à jour qui **élargit** ce qu'un module demande voit ses nouvelles
capacités refusées jusqu'à ce que vous les revoyiez.

::: info Grâce jusqu'à revue
Un module que vous n'avez jamais revu obtient ce qu'il a déclaré, pour qu'une
installation ne casse rien avant que vous n'ouvriez l'écran de consentement. Une
fois revu, seules les capacités accordées sont autorisées.
:::

## Mises à jour et désinstallation

Les **mises à jour** sont listées par extension ; les mêmes règles de vérification
et de consentement s'appliquent à la nouvelle version.

**Retirer** un plugin retire les modules qui n'avaient été tirés que comme ses
dépendances. Un module installé comme dépendance est marqué non explicite, ce qui
rend le nettoyage des orphelins possible. Un module dont autre chose a encore
besoin reste.

## Extensions payantes

Un paquet porte un modèle de licence : gratuit, payant, ou abonnement. Un paquet
payant ne s'installe pas sans une **licence** qui le couvre.

Une licence est une clé (`QTLIC-…`) : un droit **signé par le store** et vérifié
**hors-ligne** contre des clés de store épinglées. Une fois que vous l'avez, aucun
réseau n'est nécessaire — l'app continue de marcher dans un avion.

Certains paquets payants livrent leur WebAssembly **chiffré**. La clé de contenu
est livrée dans la licence elle-même, et le déchiffrement se fait **au chargement,
en mémoire** — le clair n'est jamais réécrit sur le disque. Signature et
chiffrement sont deux couches indépendantes : un paquet premium est signé en clair
(pour rester catalogable et sa certification calculable) **et** chiffré.

Collez une clé de licence dans **Réglages → Licences**.

## Héberger son propre registre

Le registre est un `index.json` statique (signé), plus un `crl.json`, plus des
blobs `.qtpkg` adressés par contenu. Il n'a rien de dynamique : n'importe quel
hébergement de fichiers fait l'affaire.

```bash
quicktys-regserve <dossier_paquets> --port 8000 --sign
```

Pointez l'app dessus avec la variable d'environnement `QUICKTYS_REGISTRY`. En
développement, `setup/run.sh` fait exactement ça — il empaquette et signe le
catalogue local, puis le sert, pour que la chaîne marketplace complète soit
exercée plutôt que contournée.

Voir [Empaqueter & publier](/fr/dev/packaging) pour le côté publication, et
[Outils en ligne de commande](/fr/reference/cli) pour les options.

## Sideload

Vous pouvez installer un `.qtpkg` depuis le disque. Il passe par la même
vérification : un paquet non signé s'installe comme non certifié, et un module non
certifié qui déclare des capacités est **refusé au chargement**. Le sideload
n'achète aucun privilège.

## Et ensuite

- [Connecter ses comptes](/fr/guide/accounts) — après avoir installé Twitch.
- [Empaqueter & publier](/fr/dev/packaging) — l'autre côté du comptoir.
