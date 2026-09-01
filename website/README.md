# Quicktys — site & documentation

Site de présentation + documentation de Quicktys, en **VitePress**, bilingue
(anglais à la racine, français sous `/fr/`).

```bash
cd website
npm install
npm run dev       # http://localhost:5173
npm run build     # -> .vitepress/dist
npm run preview   # sert le build
```

## Arborescence

```
website/
├── .vitepress/
│   ├── config.mjs          # nav, sidebars (EN + FR), i18n, thème
│   └── theme/
│       ├── index.js        # thème par défaut + composants custom
│       ├── custom.css      # palette, hero, cartes
│       └── Screenshot.vue  # cadre de capture (placeholder si l'image manque)
├── index.md                # landing EN
├── guide/ dev/ reference/ contributing/     # doc EN
├── fr/                     # même arborescence, en français
└── public/screens/         # captures d'écran (voir SHOTLIST.md)
```

## Écrire une page

Chaque page existe en **deux exemplaires** : `guide/x.md` (EN) et
`fr/guide/x.md` (FR). Les deux doivent apparaître dans la sidebar
correspondante de `.vitepress/config.mjs`.

Une capture s'insère avec le composant global `<Screenshot>` :

```md
<Screenshot src="/screens/dashboard.png" caption="Le tableau de bord" />
```

Si le fichier n'existe pas encore, un cadre pointillé avec la légende est
affiché à la place — le site reste présentable avant que les captures soient
prises. La liste des captures attendues est dans
[`public/screens/SHOTLIST.md`](public/screens/SHOTLIST.md).

## SEO

Tout est piloté par deux constantes en tête de `config.mjs` :

```js
const SITE = "https://constant.rusche.fr";
const BASE = "/Quicktys/";
```

Si le site déménage, ces deux lignes sont les seules à changer — canoniques,
`hreflang`, URLs Open Graph et sitemap en découlent.

Ce qui est généré automatiquement, par page, dans `transformPageData` :

| | |
| --- | --- |
| `<link rel="canonical">` | l'URL absolue de la page |
| `hreflang` `en` / `fr` / `x-default` | les deux versions **plus** la page elle-même, sinon Google considère la paire incomplète |
| Open Graph + Twitter Card | titre, description, `og:image`, locale et locale alternative |
| JSON-LD | `SoftwareApplication` sur les deux accueils, `BreadcrumbList` ailleurs |

Ce qui est écrit à la main :

- **Une `description` unique par page**, dans le frontmatter. C'est ce que Google
  affiche sous le titre — une page sans description en hérite une générique et
  perd son intérêt dans les résultats. Toute nouvelle page doit en avoir une.
- Les deux **images de partage** `public/og.png` et `public/og-fr.png`
  (1200×630). Elles sont générées depuis un gabarit HTML, pas dessinées : voir
  l'historique de génération si elles doivent être refaites.

Le **sitemap** est émis à `/Quicktys/sitemap.xml`. `srcExclude` retire de la
compilation ce README et `public/**`, sinon ils deviennent des pages indexables.

### Le `robots.txt` n'est pas dans ce dépôt

Les robots ne lisent le `robots.txt` **qu'à la racine du domaine**. Le site vit
sous `/Quicktys/`, donc un `robots.txt` déposé ici ne serait jamais lu. La racine
`constant.rusche.fr` est servie par le dépôt du site perso — c'est **là** qu'il
faut ajouter le fichier, avec :

```
User-agent: *
Allow: /

Sitemap: https://constant.rusche.fr/Quicktys/sitemap.xml
```

À défaut, soumettre directement `https://constant.rusche.fr/Quicktys/sitemap.xml`
dans la Google Search Console. Sans `robots.txt`, tout reste crawlable de toute
façon ; seule la découverte automatique du sitemap manque.

## Déploiement

`.github/workflows/docs.yml` construit le site à chaque push sur `main` qui
touche `website/` et le publie sur **GitHub Pages**. Il faut activer Pages une
fois dans les réglages du dépôt : *Settings → Pages → Source : GitHub Actions*.

Le site est servi sous `/Quicktys/` (`base` dans `config.mjs`). Pour un domaine
dédié, mettre `base: '/'` et ajouter un `public/CNAME`.
