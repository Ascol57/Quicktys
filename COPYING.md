# Licences — qui couvre quoi

Copyright © 2026 Ascol57. Ce dépôt contient **deux régimes de licence distincts**.
Ce fichier dit lequel s'applique à quoi ; les textes qui font foi sont les
fichiers `LICENSE*`.

| Ce que c'est | Où | Licence | Texte |
| --- | --- | --- | --- |
| **Le cœur Quicktys** | `crates/` (sauf ci-dessous), `frontend/`, `wit/`, `lang/`, `scripts/`, `docs/`, la racine | **AGPL-3.0-only** **+** *Quicktys Extension Exception* | [`LICENSE`](LICENSE) + [`LICENSE-EXCEPTION`](LICENSE-EXCEPTION) |
| **Les SDK invités** | `crates/quicktys-plugin-sdk`, `crates/quicktys-module-sdk` | **MIT** | [`LICENSE-SDK`](LICENSE-SDK) |
| **Les extensions de référence** | `plugins/`, `modules/`, `widgets/` | **Propriétaire, source-available** — tous droits réservés | [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS) |
| **L'outillage de commerce** | `crates/quicktys-regserve`, `crates/quicktys-pack` | **Propriétaire, source-available** — tous droits réservés | [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS) §1(b) |
| **Vos extensions tierces** | hors de ce dépôt | **la licence de votre choix**, y compris fermée et payante | l'exception vous y autorise |

## Le cœur : AGPL-3.0 + exception d'extension

Le cœur est du logiciel libre sous **AGPL-3.0-only**. Copyleft *réseau* : toute
version modifiée que vous **distribuez** ou que vous **exposez comme service**
(marketplace alternatif, backend de remote buttons, service hébergé) doit publier
ses sources. Un fork fermé et revendu est donc impossible.

L'**exception d'extension** ([`LICENSE-EXCEPTION`](LICENSE-EXCEPTION)) corrige le
seul effet indésirable de ce choix : elle garantit qu'une extension qui parle au
cœur **uniquement via l'ABI officielle** (les contrats WIT de [`wit/`](wit/), les
SDK invités, l'IPC host ↔ WebView, le pont widget) **n'est pas une œuvre dérivée**
du cœur. Vous pouvez donc écrire, **vendre** et distribuer un plugin, un module ou
un widget **fermé**, sans contaminer votre code.

C'est l'isolation WASM + la stabilité de l'ABI qui rendent ce découplage
défendable (ARCHITECTURE.md §0bis, §4).

Ce que l'exception ne fait **pas** :

- elle ne s'applique **pas** au cœur lui-même — le modifier reste soumis à l'AGPL,
  §13 réseau compris ;
- elle ne couvre pas une extension qui **copie du code du cœur** ou qui contourne
  l'ABI pour taper dans les internes ;
- elle ne cède aucun droit sur la marque « Quicktys ».

SPDX pour les fichiers du cœur :

```
AGPL-3.0-only WITH quicktys-extension-exception
```

⚠️ Identifiant d'exception **personnalisé**, pas un identifiant SPDX enregistré :
un outil qui valide strictement le SPDX peut le rejeter.

## Les SDK invités : MIT

`quicktys-plugin-sdk` et `quicktys-module-sdk` sont **sous MIT**, pas sous la
licence du cœur ([`LICENSE-SDK`](LICENSE-SDK)). Raison : ils sont **liés
statiquement dans chaque extension**. L'exception d'extension les inscrit déjà
dans l'« Official Extension ABI » (§0(b)), donc les lier n'a jamais rendu une
extension dérivée du cœur — mais les passer en MIT supprime la question au lieu
de la résoudre par renvoi, et permet en plus de les vendoriser, forker ou
réutiliser hors de Quicktys.

Conséquence pratique : vous pouvez écrire, vendre et distribuer une extension
**fermée** sans qu'aucune obligation de copyleft ne remonte par le SDK.

## L'outillage de commerce : propriétaire

Le marketplace a deux moitiés, et elles ne sont **pas** sous la même licence.

- **Le côté consommateur reste dans le cœur, sous AGPL** :
  [`quicktys-registry`](crates/quicktys-registry) (chercher, résoudre,
  télécharger), [`quicktys-cert`](crates/quicktys-cert) et
  [`quicktys-crypto`](crates/quicktys-crypto) (vérifier les chaînes de signature,
  les entitlements, les payloads chiffrés),
  [`quicktys-install`](crates/quicktys-install). La **vérification doit rester
  ouverte** : l'utilisateur doit pouvoir auditer ce que son app accepte de faire
  confiance, et un DRM client dont le déchiffreur est secret n'est que du théâtre
  de sécurité (cf. [`docs/drm.md`](docs/drm.md) §0).
- **Le côté émetteur est propriétaire, source-available** :
  [`quicktys-regserve`](crates/quicktys-regserve) (le serveur d'API du registre)
  et [`quicktys-pack`](crates/quicktys-pack) (empaqueter, **signer**, émettre des
  licences). Mêmes termes que les extensions de référence
  ([`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS) §1(b)) : lisibles et compilables
  pour le développement local, **ni redistribuables ni réutilisables**.

Autrement dit : n'importe qui peut vérifier ce que le store signe ; frapper des
signatures et faire tourner le store, non.

⚠️ L'AGPL §13 continue de s'appliquer au **cœur** : un marketplace *dérivé du
cœur* doit publier ses sources. Ce qui précède ne concerne que ces deux crates-ci.

## Les extensions de ce dépôt : à moi, mais visibles

Les plugins, modules et widgets livrés ici (`time`, `obs`, `pear`, `twitch`,
`discord`, `sound`, `os`, `input`, `http`, les widgets `pear-*`…) sont des
**œuvres propriétaires d'Ascol57**. Leur source est publique — pour la revue, la
transparence, et parce que le cœur en a besoin comme fixtures de test — mais
**publier n'est pas donner** :

- ✅ vous pouvez les **lire**, les **compiler** et les **utiliser** avec Quicktys,
  les modifier **en privé** pour tester ;
- ❌ vous ne pouvez pas les **redistribuer**, les **forker publiquement**, les
  **vendre**, ni les **réutiliser** dans un autre produit ou une autre extension.

Elles restent en revanche du **matériau de référence** : rien ne vous empêche
d'étudier *comment* elles appellent l'ABI publique et d'écrire la vôtre contre
cette même ABI. Ce qui est interdit, c'est de réutiliser **ce code-là** pour le
faire.

Détail des permissions exactes : [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS).

SPDX déclaré dans leurs `Cargo.toml` : `LicenseRef-Ascol57-Proprietary`.

## Contributions (CLA)

Les contributions sont acceptées sous **CLA** : le contributeur cède/licencie ses
droits au propriétaire, qui détient le projet et peut donc **dual-licencier**
(vendre des licences commerciales exemptées d'AGPL). Le texte du CLA reste à
rédiger — voir ARCHITECTURE.md §0bis.

## Réécriture clean-room

Quicktys ne contient **aucune ligne de code de CastMate** (MPL-2.0). C'est une
réimplémentation clean-room : les *concepts* et *APIs* ont été étudiés, tout le
code est écrit de zéro — frontend compris. Voir ARCHITECTURE.md §1.

---

## Summary in English

This repository is **dual-regime**:

- **Quicktys core** (`crates/`, `frontend/`, `wit/`, `lang/`, `scripts/`, root) —
  free software under **AGPL-3.0-only**, plus the **Quicktys Extension Exception**
  ([`LICENSE-EXCEPTION`](LICENSE-EXCEPTION)). Any extension that talks to the core
  *solely through the official ABI* is not a derivative work, and may be licensed
  and **sold under any terms, including closed source**.
- **The guest SDKs** (`quicktys-plugin-sdk`, `quicktys-module-sdk`) — **MIT**
  ([`LICENSE-SDK`](LICENSE-SDK)), because they are linked into every extension.
- **The reference extensions shipped here** (`plugins/`, `modules/`, `widgets/`) —
  **proprietary, source-available**, © Ascol57, all rights reserved. Readable,
  buildable and usable with Quicktys; **not** redistributable or reusable. See
  [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS).
- **The commerce tooling** (`quicktys-regserve`, `quicktys-pack`) — same
  proprietary source-available terms. The *verification* side
  (`quicktys-registry`, `quicktys-cert`, `quicktys-crypto`, `quicktys-install`)
  deliberately stays in the AGPL core.

The public extension **API** is free for anyone to build against; this particular
**implementation** of it is not free to copy.

---

*Ces textes sont une proposition de rédaction, pas un avis juridique. Ils n'ont
pas été relus par un·e juriste — ARCHITECTURE.md §0bis prévoit explicitement que
l'exception et le CLA soient validés par un·e professionnel·le avant toute
exploitation commerciale.*
