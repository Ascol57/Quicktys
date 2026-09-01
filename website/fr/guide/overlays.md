---
description: "Construire des overlays de stream dans l'éditeur WYSIWYG de Quicktys, avec de vrais aperçus en direct et des guides d'aimantation, puis les afficher via une source navigateur OBS."
---

# Overlays

Un **overlay** est une page de widgets qu'OBS affiche comme source navigateur. On
le construit dans l'app, sur un canvas, en voyant les vrais widgets pendant qu'on
les place.

<Screenshot src="/screens/overlay-editor.png" caption="L'éditeur d'overlays" />

## D'abord, l'installer

Les overlays arrivent avec le plugin **Overlays**. Installez-le depuis la
[Marketplace](/fr/guide/marketplace) ; il tire le module `webview`, qui stocke
réellement vos overlays et livre l'onglet d'édition.

::: info Pourquoi un plugin pour une fonctionnalité du cœur
Les modules ne sont jamais parcourables — ils n'arrivent que tirés par un plugin
qui les déclare. Le module d'overlays n'avait aucun plugin le requérant : il ne
s'installait donc jamais par le chemin du registre. Le plugin Overlays est la face
utilisateur de la fonctionnalité et porte la dépendance. Le désinstaller retire
aussi le module, et avec lui l'éditeur et les pages `/overlay/<id>`.
:::

## En créer un

Onglet **Overlays** → **Nouveau**. Un overlay, c'est :

```
{ id, name, aspect: 16:9 | 9:16, widgets: [...] }
```

Le ratio est un ratio d'aperçu, pas une résolution. Les positions et tailles des
widgets sont stockées en **pourcentages**, donc l'overlay se rend à l'identique en
720p, 1080p ou 1440p.

## Placer des widgets

Ajoutez un widget : il apparaît sur le canvas, **rendu pour de vrai**, en direct,
avec vos couleurs et vos options. Ce n'est pas une maquette : c'est le widget
lui-même, dans son propre document isolé, alimenté par le même message de config
que l'overlay en direct, et rendu inerte pour ne jamais avaler un clic.

| Geste | Effet |
| --- | --- |
| Glisser n'importe où sur le widget | Le déplacer. |
| Glisser une des huit poignées | Redimensionner. |
| Flèches | Déplacer finement. **Maj** pour ×10. |
| **Suppr** | Retirer. |
| **Échap** | Désélectionner. |
| **▴** / **▾** | Monter ou descendre. L'ordre de dessin est l'ordre de la liste, donc monter revient à déplacer plus loin. |

Les bords s'aimantent aux côtés et au centre du canvas, ainsi qu'aux côtés et au
centre de chaque autre widget, en traçant un guide pour ce qui a correspondu.
**Maintenez Alt pour placer librement.** La géométrie est arrondie au dépôt, donc
les pourcentages stockés correspondent à ce qui est affiché.

<Screenshot src="/screens/overlay-widget-config.png" caption="Les réglages d'un widget dans l'éditeur d'overlays" />

::: warning Deux endroits, deux formulaires
Un widget se configure dans **deux** UI distinctes — le tableau de bord et
l'éditeur d'overlays — avec du code séparé. Elles rendent le même schéma, mais un
réglage ajouté à l'une n'est pas automatiquement dans l'autre. C'est la raison
quand une option manque d'un côté.
:::

### Les widgets invisibles au repos

Un widget d'alerte n'affiche rien tant qu'elle n'est pas déclenchée, ce qui le
rend impossible à positionner. Ces widgets livrent une **page d'aperçu séparée** :

- l'**éditeur d'overlays** charge `preview.html`,
- une **source navigateur OBS** charge toujours `index.html`.

L'aperçu reçoit le même message de `config`, donc il se met en scène avec du
contenu factice tout en respectant vos réglages. `index.html` reste ainsi simple
et inerte, la démo vivant dans un fichier à part.

## Le mettre dans OBS

L'app fait tourner un serveur HTTP local sur le port **8787**. Ajoutez une
**source navigateur** dans OBS pointant sur :

```
http://localhost:8787/overlay/<id>
```

Réglez largeur et hauteur sur la résolution de votre canvas (1920×1080 pour un
overlay 16:9). L'overlay est transparent là où rien n'est dessiné.

<Screenshot src="/screens/obs-browser-source.png" caption="Les propriétés de la source navigateur OBS" />

::: tip Laissez le plugin OBS le faire
Si le plugin OBS est installé et connecté, la barre d'outils de l'éditeur
d'overlays gagne un bouton **Ajouter à OBS**. Il crée la source navigateur dans
votre scène courante, avec la bonne URL et le bon nom — sans copier-coller.

Ce bouton n'est pas codé en dur : c'est une **contribution d'UI** que le plugin
OBS déclare dans le slot de la barre d'outils de l'éditeur. Voir [Contributions
d'UI](/fr/dev/ui).
:::

## Comment l'état arrive au navigateur

Le serveur local pousse l'état vers la page d'overlay en **SSE** (`/events`) : un
instantané d'abord, puis un événement par clé modifiée. Le widget consomme
exactement le même pont de messages que dans l'app, ce qui explique que le rendu
soit identique des deux côtés.

Une action demandée par un widget revient en `POST /action` et s'exécute comme une
séquence à un nœud.

Le transport est unidirectionnel (host → navigateur) par choix ; SSE suffit et ne
demande aucune poignée de main.

## Dépannage

**La source navigateur est vide.** Vérifiez que l'app tourne, et ouvrez l'URL dans
un navigateur normal — vous devriez voir l'overlay. Si le plugin Overlays n'est
pas installé, `/overlay/<id>` n'existe pas.

**Un widget s'affiche mais sans données.** Le plugin du widget n'est peut-être pas
installé, ou la valeur qu'il lit est vide. Vérifiez la clé dont il a besoin dans
la vue [Variables](/fr/guide/interface#variables).

**Chaque message de chat en double.** Le même chat est connecté deux fois — une
boîte du tableau de bord *et* un overlay appellent tous deux la connexion. Fermer
l'un ferme désormais vraiment son socket, donc c'est corrigé, mais une instance
d'app restée ouverte peut encore le provoquer.

**Les polices sont fausses.** Les iframes de widget n'ont aucun accès réseau : une
police web retombe silencieusement sur une autre. Les widgets n'utilisent que des
piles de polices système.

## Et ensuite

- [Tableau de bord](/fr/guide/dashboard) — les mêmes widgets, dans l'app.
- [Écrire un widget](/fr/dev/widget) — construire le vôtre.
