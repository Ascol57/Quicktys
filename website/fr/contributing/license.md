---
description: "Les quatre régimes de licence de Quicktys : un cœur AGPL avec exception d'extension, des SDK invités MIT, et des extensions et outils de commerce propriétaires."
---

# Licences

Le dépôt comporte **quatre** régimes de licence. Lequel s'applique dépend de ce
que vous regardez.

| Quoi | Où | Licence |
| --- | --- | --- |
| **Le cœur Quicktys** | `crates/` (sauf ci-dessous), `frontend/`, `wit/`, `lang/`, `scripts/`, la racine | **AGPL-3.0-only** + l'*exception d'extension Quicktys* |
| **Les SDK invités** | `quicktys-plugin-sdk`, `quicktys-module-sdk` | **MIT** |
| **Les extensions de référence** | `plugins/`, `modules/`, `widgets/` | **Propriétaire, source-available** — © Ascol57 |
| **L'outillage de commerce** | `quicktys-regserve`, `quicktys-pack` | **Propriétaire, source-available** — © Ascol57 |
| **Vos extensions tierces** | hors de ce dépôt | **la licence de votre choix**, y compris fermée et commerciale |

Les textes qui font foi sont les fichiers `LICENSE*` ;
[`COPYING.md`](https://github.com/Ascol57/Quicktys/blob/main/COPYING.md) en est la
carte.

## Le cœur : AGPL-3.0 + une exception d'extension

Le cœur est du logiciel libre sous **AGPL-3.0-only** — copyleft *réseau*. Toute
version modifiée que vous **distribuez** ou que vous **exposez comme service** (un
marketplace alternatif, un backend hébergé) doit publier ses sources. Un fork
fermé et revendu est donc impossible.

L'**exception d'extension** corrige le seul effet indésirable de ce choix. Elle
garantit qu'une extension qui parle au cœur **uniquement via l'ABI officielle**
**n'est pas une œuvre dérivée**. Vous pouvez donc écrire, **vendre** et distribuer
un plugin, un module ou un widget **fermé**, sans contaminer votre code.

L'« Official Extension ABI » est définie de façon limitative :

- les contrats WIT de `wit/` et toute liaison générée mécaniquement depuis eux ;
- les crates de SDK invités ;
- le catalogue de commandes et d'événements IPC host ↔ WebView ;
- le pont widget — le transport local, le schéma `widget://`, le protocole
  `postMessage` documenté ;
- les formats de manifeste, de bundle, de schéma et de `lang/`.

C'est l'isolation WASM plus la stabilité de l'ABI qui rendent ce découplage
défendable.

**Ce que l'exception ne fait pas :**

- elle n'accorde rien concernant le cœur lui-même — le modifier reste soumis à
  l'AGPL, §13 compris ;
- elle ne couvre pas une extension qui **copie du code du cœur** ou contourne
  l'ABI pour taper dans les internes ;
- elle ne cède aucun droit sur le nom « Quicktys ».

SPDX pour les fichiers du cœur :

```
AGPL-3.0-only WITH quicktys-extension-exception
```

::: warning Un identifiant d'exception personnalisé
`quicktys-extension-exception` n'est **pas** un identifiant SPDX enregistré. Un
outil qui valide strictement le SPDX peut le rejeter.
:::

## Les SDK invités : MIT

`quicktys-plugin-sdk` et `quicktys-module-sdk` sont sous **MIT**
([`LICENSE-SDK`](https://github.com/Ascol57/Quicktys/blob/main/LICENSE-SDK)), et
non sous la licence du cœur — parce qu'ils sont **liés statiquement dans chaque
extension**.

L'exception les inscrivait déjà dans l'ABI officielle, donc les lier n'a jamais
rendu une extension dérivée. Les passer en MIT supprime la question au lieu d'y
répondre par renvoi, et permet en plus de vendoriser, forker et réutiliser les SDK
entièrement hors de Quicktys.

Conséquence pratique : **rien de copyleft ne peut atteindre votre extension par le
SDK.**

::: warning Un trou identifié
Les SDK ré-exportent `quicktys_schema::Schema`, donc `quicktys-schema` —
actuellement AGPL — est compilé dans chaque extension aussi. C'est le même
problème que l'exception règle pour les SDK, mais ce crate n'y est pas nommé.

Il faut soit l'ajouter à la liste de l'ABI dans l'exception, soit le passer en
MIT. C'est consigné comme point ouvert dans `ARCHITECTURE.md` §0bis. Si vous
livrez une extension fermée aujourd'hui, ayez-le en tête.
:::

## Les extensions de ce dépôt : à moi, mais visibles

Les plugins, modules et widgets livrés ici sont des **œuvres propriétaires
d'Ascol57**. Leur source est publique — pour la revue, la transparence, et parce
que le cœur en a besoin comme fixtures de test — mais **publier n'est pas
donner** :

- ✅ vous pouvez les **lire**, les **compiler** et les **utiliser** avec Quicktys,
  les modifier **en privé** pour tester ;
- ❌ vous ne pouvez pas les **redistribuer**, les **forker publiquement**, les
  **vendre**, ni les **réutiliser** dans un autre produit ou une autre extension.

Elles restent du **matériau de référence** : rien ne vous empêche d'étudier
*comment* elles appellent l'ABI publique et d'écrire la vôtre contre cette même
ABI. Ce qui est interdit, c'est de réutiliser **ce code-là** pour le faire.

SPDX dans leurs manifestes : `LicenseRef-Ascol57-Proprietary`.

## L'outillage de commerce : propriétaire

La marketplace a deux moitiés, et elles ne sont pas sous la même licence.

**Le côté consommateur reste dans le cœur AGPL** : `quicktys-registry` (chercher,
résoudre, télécharger), `quicktys-cert` et `quicktys-crypto` (vérifier les chaînes
de signature, les droits, les payloads chiffrés), `quicktys-install`.

La vérification **doit** rester ouverte. L'utilisateur doit pouvoir auditer ce que
son application accepte de croire, et un DRM client dont le déchiffreur est secret
n'est que du théâtre de sécurité — `docs/drm.md` §0 le pose dès la première page :
le déchiffreur est ouvert par conception, le secret est dans les clés.

**Le côté émetteur est propriétaire** : `quicktys-regserve` (le serveur d'API du
registre) et `quicktys-pack` (empaqueter, **signer**, frapper des licences), sous
les mêmes termes que les extensions de référence.

Autrement dit : n'importe qui peut vérifier ce que le store signe ; frapper des
signatures et faire tourner le store, non.

L'AGPL §13 continue de s'appliquer au **cœur** — un marketplace *dérivé du cœur*
doit publier ses sources. Ces deux crates n'y changent rien.

## Contributions : un CLA

Les contributions sont acceptées sous **Contributor License Agreement** : le
contributeur cède ou licencie ses droits au propriétaire, qui détient ainsi le
projet et peut le **dual-licencier** — vendre des licences commerciales exemptées
d'AGPL.

**Le texte du CLA n'est pas encore rédigé.**

## La contrainte clean-room

Quicktys ne contient **aucune ligne de code CastMate** (MPL-2.0). C'est une
réimplémentation clean-room : les *concepts* et les *APIs* ont été étudiés, tout le
code est écrit de zéro — frontend compris.

Lire le YAML CastMate d'un utilisateur pour la migration est licite : c'est sa
donnée, pas le code de CastMate.

## Ceci n'est pas un avis juridique

Ces textes sont une proposition de rédaction, pas un avis juridique. Ils **n'ont
pas été relus par un·e juriste**. `ARCHITECTURE.md` §0bis prévoit explicitement que
l'exception et le CLA soient validés par un·e professionnel·le avant toute
exploitation commerciale.

## Et ensuite

- [Architecture](/fr/contributing/architecture)
- [Empaqueter & publier](/fr/dev/packaging)
