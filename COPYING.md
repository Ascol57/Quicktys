# Licences — qui couvre quoi

Copyright © 2026 Ascol57. Ce dépôt contient **deux régimes de licence distincts**.
Ce fichier dit lequel s'applique à quoi ; les textes qui font foi sont les
fichiers `LICENSE*`.

| Ce que c'est | Où | Licence | Texte |
| --- | --- | --- | --- |
| **Le cœur Quicktys** | `crates/`, `frontend/`, `wit/`, `lang/`, `scripts/`, `docs/`, la racine | **AGPL-3.0-only** **+** *Quicktys Extension Exception* | [`LICENSE`](LICENSE) + [`LICENSE-EXCEPTION`](LICENSE-EXCEPTION) |
| **Les extensions de référence** | `plugins/`, `modules/`, `widgets/` | **Propriétaire, source-available** — tous droits réservés | [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS) |
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
- **The reference extensions shipped here** (`plugins/`, `modules/`, `widgets/`) —
  **proprietary, source-available**, © Ascol57, all rights reserved. Readable,
  buildable and usable with Quicktys; **not** redistributable or reusable. See
  [`LICENSE-EXTENSIONS`](LICENSE-EXTENSIONS).

The public extension **API** is free for anyone to build against; this particular
**implementation** of it is not free to copy.

---

*Ces textes sont une proposition de rédaction, pas un avis juridique. Ils n'ont
pas été relus par un·e juriste — ARCHITECTURE.md §0bis prévoit explicitement que
l'exception et le CLA soient validés par un·e professionnel·le avant toute
exploitation commerciale.*
