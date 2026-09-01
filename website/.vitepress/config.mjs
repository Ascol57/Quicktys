import { defineConfig } from "vitepress";

const REPO = "https://github.com/Ascol57/Quicktys";

// Where the site is actually served. Verified live: ascol57.github.io 301s to
// this apex, and the project page resolves under /Quicktys/. Canonical URLs,
// hreflang alternates, Open Graph URLs and the sitemap are all derived from
// these two constants — change them here and nowhere else.
const SITE = "https://constant.rusche.fr";
const BASE = "/Quicktys/";

// The repository is private while the licence texts are under legal review, but
// the published site is public. Flip this to `true` the day the repo opens: it
// restores the "edit this page" links, which are pointless (and 404) until then.
// The in-prose links to source files stay as they are — they are correct URLs,
// they simply do not resolve for anyone yet; the notices on the landing page and
// the installation page say so.
const REPO_PUBLIC = false;

// ---------------------------------------------------------------------------
// Sidebars. Each locale gets the same structure with its own prefix + labels.
// A `p` prefix of "" is the English root, "/fr" the French locale.
// ---------------------------------------------------------------------------
const sidebar = (p, t) => ({
  [`${p}/guide/`]: [
    {
      text: t.gettingStarted,
      collapsed: false,
      items: [
        { text: t.whatIs, link: `${p}/guide/what-is-quicktys` },
        { text: t.installation, link: `${p}/guide/installation` },
        { text: t.firstAutomation, link: `${p}/guide/getting-started` },
        { text: t.interface, link: `${p}/guide/interface` },
      ],
    },
    {
      text: t.buildingAutomations,
      collapsed: false,
      items: [
        { text: t.profiles, link: `${p}/guide/profiles` },
        { text: t.automations, link: `${p}/guide/automations` },
        { text: t.variables, link: `${p}/guide/variables` },
      ],
    },
    {
      text: t.onScreen,
      collapsed: false,
      items: [
        { text: t.dashboard, link: `${p}/guide/dashboard` },
        { text: t.overlays, link: `${p}/guide/overlays` },
      ],
    },
    {
      text: t.extending,
      collapsed: false,
      items: [
        { text: t.marketplace, link: `${p}/guide/marketplace` },
        { text: t.accounts, link: `${p}/guide/accounts` },
        { text: t.integrations, link: `${p}/guide/integrations` },
        { text: t.troubleshooting, link: `${p}/guide/troubleshooting` },
      ],
    },
  ],
  [`${p}/dev/`]: [
    {
      text: t.devGuide,
      collapsed: false,
      items: [
        { text: t.devOverview, link: `${p}/dev/overview` },
        { text: t.devEnv, link: `${p}/dev/environment` },
      ],
    },
    {
      text: t.writingExtensions,
      collapsed: false,
      items: [
        { text: t.writePlugin, link: `${p}/dev/plugin` },
        { text: t.writeModule, link: `${p}/dev/module` },
        { text: t.writeWidget, link: `${p}/dev/widget` },
        { text: t.devI18n, link: `${p}/dev/i18n` },
        { text: t.devUi, link: `${p}/dev/ui` },
      ],
    },
    {
      text: t.shipping,
      collapsed: false,
      items: [{ text: t.packaging, link: `${p}/dev/packaging` }],
    },
  ],
  [`${p}/reference/`]: [
    {
      text: t.abi,
      collapsed: false,
      items: [
        { text: t.refWorlds, link: `${p}/reference/worlds` },
        { text: t.refCaps, link: `${p}/reference/capabilities` },
        { text: t.refSchema, link: `${p}/reference/schema` },
      ],
    },
    {
      text: t.engine,
      collapsed: false,
      items: [
        { text: t.refNodes, link: `${p}/reference/nodes` },
        { text: t.refTemplating, link: `${p}/reference/templating` },
      ],
    },
    {
      text: t.catalogue,
      collapsed: false,
      items: [
        { text: t.refExtensions, link: `${p}/reference/extensions` },
        { text: t.refCli, link: `${p}/reference/cli` },
        { text: t.refFiles, link: `${p}/reference/files` },
      ],
    },
  ],
  [`${p}/contributing/`]: [
    {
      text: t.contributing,
      collapsed: false,
      items: [
        { text: t.cArchitecture, link: `${p}/contributing/architecture` },
        { text: t.cBuilding, link: `${p}/contributing/building` },
        { text: t.cRoadmap, link: `${p}/contributing/roadmap` },
        { text: t.cLicense, link: `${p}/contributing/license` },
      ],
    },
  ],
});

const EN = {
  gettingStarted: "Getting started",
  whatIs: "What is Quicktys?",
  installation: "Installation",
  firstAutomation: "Your first automation",
  interface: "The interface",
  buildingAutomations: "Building automations",
  profiles: "Profiles & triggers",
  automations: "Automations",
  variables: "Variables & templating",
  onScreen: "On screen",
  dashboard: "Dashboard",
  overlays: "Overlays",
  extending: "Going further",
  marketplace: "Marketplace",
  accounts: "Connecting accounts",
  integrations: "Integrations",
  troubleshooting: "Troubleshooting",
  devGuide: "Developer guide",
  devOverview: "Extension model",
  devEnv: "Dev environment",
  writingExtensions: "Writing an extension",
  writePlugin: "Write a plugin",
  writeModule: "Write a module",
  writeWidget: "Write a widget",
  devI18n: "Translations",
  devUi: "UI contributions",
  shipping: "Shipping",
  packaging: "Package & publish",
  abi: "Extension ABI",
  refWorlds: "Worlds & lifecycle",
  refCaps: "Host capabilities",
  refSchema: "Schema types",
  engine: "Engine",
  refNodes: "Automation nodes",
  refTemplating: "Templating helpers",
  catalogue: "Catalogue",
  refExtensions: "Shipped extensions",
  refCli: "Command-line tools",
  refFiles: "Files & data directory",
  contributing: "Contributing",
  cArchitecture: "Architecture",
  cBuilding: "Building from source",
  cRoadmap: "Roadmap",
  cLicense: "Licensing",
};

const FR = {
  gettingStarted: "Démarrer",
  whatIs: "Qu'est-ce que Quicktys ?",
  installation: "Installation",
  firstAutomation: "Première automatisation",
  interface: "L'interface",
  buildingAutomations: "Construire des automatisations",
  profiles: "Profils & triggers",
  automations: "Automatisations",
  variables: "Variables & templating",
  onScreen: "À l'écran",
  dashboard: "Tableau de bord",
  overlays: "Overlays",
  extending: "Aller plus loin",
  marketplace: "Marketplace",
  accounts: "Connecter ses comptes",
  integrations: "Intégrations",
  troubleshooting: "Dépannage",
  devGuide: "Guide développeur",
  devOverview: "Modèle d'extension",
  devEnv: "Environnement de dev",
  writingExtensions: "Écrire une extension",
  writePlugin: "Écrire un plugin",
  writeModule: "Écrire un module",
  writeWidget: "Écrire un widget",
  devI18n: "Traductions",
  devUi: "Contributions d'UI",
  shipping: "Distribuer",
  packaging: "Empaqueter & publier",
  abi: "ABI d'extension",
  refWorlds: "Worlds & cycle de vie",
  refCaps: "Capacités du host",
  refSchema: "Types de schéma",
  engine: "Moteur",
  refNodes: "Nœuds d'automatisation",
  refTemplating: "Helpers de templating",
  catalogue: "Catalogue",
  refExtensions: "Extensions livrées",
  refCli: "Outils en ligne de commande",
  refFiles: "Fichiers & dossier de données",
  contributing: "Contribuer",
  cArchitecture: "Architecture",
  cBuilding: "Compiler depuis les sources",
  cRoadmap: "Roadmap",
  cLicense: "Licences",
};


// --------------------------------------------------------------------------
// Structured data. Two shapes only: the landing describes the product, every
// documentation page describes where it sits. Anything more would be schema
// for schema's sake.
// --------------------------------------------------------------------------
const EN_SEO = {
  homeTitle: "Quicktys — stream automation, rebuilt in Rust",
  fallbackDescription:
    "Stream automation for OBS, Twitch and everything around them, built on Rust, Tauri and sandboxed WebAssembly extensions.",
  imageAlt: "Quicktys — stream automation, rebuilt in Rust",
  // Section label + the page the navbar's own link points at. Google requires
  // an `item` URL on every breadcrumb entry except the last, and there are no
  // section index pages — so a section resolves to the same entry point the
  // navigation uses, which is truthful rather than invented.
  sections: {
    guide: ["Guide", "guide/what-is-quicktys"],
    dev: ["Develop", "dev/overview"],
    reference: ["Reference", "reference/worlds"],
    contributing: ["Contributing", "contributing/architecture"],
  },
};

const FR_SEO = {
  homeTitle: "Quicktys — l'automatisation de stream, rebâtie en Rust",
  fallbackDescription:
    "L'automatisation de stream pour OBS, Twitch et tout ce qui gravite autour, bâtie sur Rust, Tauri et des extensions WebAssembly isolées.",
  imageAlt: "Quicktys — l'automatisation de stream, rebâtie en Rust",
  sections: {
    guide: ["Guide", "guide/what-is-quicktys"],
    dev: ["Développer", "dev/overview"],
    reference: ["Référence", "reference/worlds"],
    contributing: ["Contribuer", "contributing/architecture"],
  },
};

const softwareLd = (L, url, image) => ({
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "Quicktys",
  description: L.fallbackDescription,
  url,
  image,
  applicationCategory: "MultimediaApplication",
  operatingSystem: "Windows, macOS, Linux",
  author: { "@type": "Person", name: "Ascol57" },
  offers: { "@type": "Offer", price: "0", priceCurrency: "EUR" },
  license: "https://www.gnu.org/licenses/agpl-3.0.html",
  softwareHelp: url,
});

const breadcrumbLd = (bare, isFr, url, pageData) => {
  const L = isFr ? FR_SEO : EN_SEO;
  const home = SITE + BASE + (isFr ? "fr/" : "");
  const prefix = BASE + (isFr ? "fr/" : "");
  const section = L.sections[bare.split("/")[0]];
  const items = [{ "@type": "ListItem", position: 1, name: "Quicktys", item: home }];
  // Skip the section level when the page *is* the section's entry point, which
  // would otherwise produce two consecutive entries with the same URL.
  if (section && section[1] !== bare) {
    items.push({
      "@type": "ListItem",
      position: 2,
      name: section[0],
      item: SITE + prefix + section[1],
    });
  }
  items.push({
    "@type": "ListItem",
    position: items.length + 1,
    name: pageData.frontmatter.title || pageData.title,
    item: url,
  });
  return { "@context": "https://schema.org", "@type": "BreadcrumbList", itemListElement: items };
};

export default defineConfig({
  title: "Quicktys",
  base: BASE,
  cleanUrls: true,
  // The dev readme and the screenshot checklist are working documents, not
  // pages: without this they compile into the site and land in the sitemap.
  srcExclude: ["README.md", "public/**"],
  lastUpdated: true,
  ignoreDeadLinks: [/^https?:\/\/localhost/, /^https?:\/\/127\.0\.0\.1/],

  sitemap: { hostname: SITE + BASE },

  head: [
    ["link", { rel: "icon", href: BASE + "logo.svg", type: "image/svg+xml" }],
    ["meta", { name: "theme-color", content: "#7c5cff" }],
    ["meta", { name: "author", content: "Ascol57" }],
    // Display + utility faces. Inter (body) already ships with VitePress.
    ["link", { rel: "preconnect", href: "https://fonts.googleapis.com" }],
    ["link", { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "" }],
    [
      "link",
      {
        rel: "stylesheet",
        href:
          "https://fonts.googleapis.com/css2?family=Familjen+Grotesk:wght@400;500;600;700" +
          "&family=IBM+Plex+Mono:wght@400;500;600&display=swap",
      },
    ],
  ],

  markdown: {
    // Shiki has no Rhai grammar. Rhai's surface syntax is Rust-shaped (`let`,
    // `//`, `fn`), so highlight it as Rust while keeping the fences honest.
    // Shiki ships neither a Rhai nor a WIT grammar. Both are Rust-shaped enough
    // (`fn`, `record`, `//`) that Rust highlighting reads correctly, and the
    // fences stay honest about what the code actually is.
    languageAlias: { rhai: "rust", wit: "rust" },

    // Quicktys' own templating syntax is `{{ }}` — which is also Vue's
    // interpolation. Fenced blocks are already wrapped in `v-pre` by VitePress;
    // inline code is not guaranteed to be, so force it. Without this,
    // `` `{{ pear.title }}` `` in prose renders as an empty string.
    config: (md) => {
      md.renderer.rules.code_inline = (tokens, idx) =>
        `<code v-pre>${md.utils.escapeHtml(tokens[idx].content)}</code>`;
    },
  },

  themeConfig: {
    logo: "/logo.svg",
    search: { provider: "local" },
    socialLinks: [{ icon: "github", link: REPO }],
    outline: [2, 3],
  },


  // ---------------------------------------------------------------------------
  // Per-page SEO head. VitePress emits <title> and <meta name="description">
  // from the frontmatter on its own; everything that needs to know the page's
  // absolute URL — canonical, hreflang alternates, Open Graph, JSON-LD — has to
  // be built here, because only this hook knows which file is being rendered.
  // ---------------------------------------------------------------------------
  transformPageData(pageData) {
    const rel = pageData.relativePath;
    const isFr = rel === "fr/index.md" || rel.startsWith("fr/");

    // 'guide/installation.md' -> 'guide/installation' ; 'index.md' -> ''
    const bare = rel
      .replace(/^fr\//, "")
      .replace(/\.md$/, "")
      .replace(/(^|\/)index$/, "$1");

    const enUrl = SITE + BASE + bare;
    const frUrl = SITE + BASE + "fr/" + bare;
    const url = isFr ? frUrl : enUrl;
    const isHome = bare === "";

    const L = isFr ? FR_SEO : EN_SEO;
    const title = isHome
      ? L.homeTitle
      : `${pageData.frontmatter.title || pageData.title} — Quicktys`;
    const description =
      pageData.frontmatter.description || L.fallbackDescription;
    const image = SITE + BASE + (isFr ? "og-fr.png" : "og.png");

    const head = [
      ["link", { rel: "canonical", href: url }],
      // A bilingual site has to declare both sides of every pair, and each page
      // must also point at itself — otherwise Google treats the set as partial.
      ["link", { rel: "alternate", hreflang: "en", href: enUrl }],
      ["link", { rel: "alternate", hreflang: "fr", href: frUrl }],
      ["link", { rel: "alternate", hreflang: "x-default", href: enUrl }],

      ["meta", { property: "og:type", content: isHome ? "website" : "article" }],
      ["meta", { property: "og:site_name", content: "Quicktys" }],
      ["meta", { property: "og:url", content: url }],
      ["meta", { property: "og:title", content: title }],
      ["meta", { property: "og:description", content: description }],
      ["meta", { property: "og:image", content: image }],
      ["meta", { property: "og:image:width", content: "1200" }],
      ["meta", { property: "og:image:height", content: "630" }],
      ["meta", { property: "og:image:alt", content: L.imageAlt }],
      ["meta", { property: "og:locale", content: isFr ? "fr_FR" : "en_US" }],
      [
        "meta",
        { property: "og:locale:alternate", content: isFr ? "en_US" : "fr_FR" },
      ],

      ["meta", { name: "twitter:card", content: "summary_large_image" }],
      ["meta", { name: "twitter:title", content: title }],
      ["meta", { name: "twitter:description", content: description }],
      ["meta", { name: "twitter:image", content: image }],
    ];

    head.push([
      "script",
      { type: "application/ld+json" },
      JSON.stringify(isHome ? softwareLd(L, url, image) : breadcrumbLd(bare, isFr, url, pageData)),
    ]);

    pageData.frontmatter.head = [...(pageData.frontmatter.head || []), ...head];
  },

  locales: {
    root: {
      label: "English",
      lang: "en-US",
      description: "Stream automation built on Rust, Tauri and WebAssembly extensions.",
      themeConfig: {
        nav: [
          { text: "Guide", link: "/guide/what-is-quicktys", activeMatch: "/guide/" },
          { text: "Develop", link: "/dev/overview", activeMatch: "/dev/" },
          { text: "Reference", link: "/reference/worlds", activeMatch: "/reference/" },
          { text: "Contributing", link: "/contributing/architecture", activeMatch: "/contributing/" },
        ],
        sidebar: sidebar("", EN),
        ...(REPO_PUBLIC && {
          editLink: {
            pattern: `${REPO}/edit/main/website/:path`,
            text: "Edit this page on GitHub",
          },
        }),
        footer: {
          message: "Core AGPL-3.0 + Quicktys Extension Exception. Shipped extensions © Ascol57.",
          copyright: "© 2026 Ascol57",
        },
        docFooter: { prev: "Previous", next: "Next" },
        darkModeSwitchLabel: "Theme",
        returnToTopLabel: "Back to top",
        lastUpdatedText: "Last updated",
      },
    },
    fr: {
      label: "Français",
      lang: "fr-FR",
      link: "/fr/",
      description: "L'automatisation de stream, rebâtie sur Rust, Tauri et des extensions WebAssembly.",
      themeConfig: {
        nav: [
          { text: "Guide", link: "/fr/guide/what-is-quicktys", activeMatch: "/fr/guide/" },
          { text: "Développer", link: "/fr/dev/overview", activeMatch: "/fr/dev/" },
          { text: "Référence", link: "/fr/reference/worlds", activeMatch: "/fr/reference/" },
          { text: "Contribuer", link: "/fr/contributing/architecture", activeMatch: "/fr/contributing/" },
        ],
        sidebar: sidebar("/fr", FR),
        ...(REPO_PUBLIC && {
          editLink: {
            pattern: `${REPO}/edit/main/website/:path`,
            text: "Modifier cette page sur GitHub",
          },
        }),
        footer: {
          message: "Cœur AGPL-3.0 + exception d'extension Quicktys. Extensions livrées © Ascol57.",
          copyright: "© 2026 Ascol57",
        },
        docFooter: { prev: "Précédent", next: "Suivant" },
        darkModeSwitchLabel: "Thème",
        returnToTopLabel: "Haut de page",
        lastUpdatedText: "Dernière mise à jour",
        outlineTitle: "Sur cette page",
      },
    },
  },
});
