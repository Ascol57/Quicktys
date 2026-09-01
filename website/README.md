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

## Déploiement

`.github/workflows/docs.yml` construit le site à chaque push sur `main` qui
touche `website/` et le publie sur **GitHub Pages**. Il faut activer Pages une
fois dans les réglages du dépôt : *Settings → Pages → Source : GitHub Actions*.

Le site est servi sous `/Quicktys/` (`base` dans `config.mjs`). Pour un domaine
dédié, mettre `base: '/'` et ajouter un `public/CNAME`.
