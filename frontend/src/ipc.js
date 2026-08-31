// Thin client over the Tauri command/event bridge.
//
// Degrades gracefully in a plain browser (`npm run dev` without Tauri): commands
// return sample data and events are no-ops, so the UI can be developed/built
// outside the desktop shell.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
// Browser-mock only: the runtime widget's HTML (the real app reads it from the
// bundle, embedded by the host into the widget manifest).
import pearNowPlayingHtml from "../../widgets/pear-now-playing/index.html?raw";
import pearQueueHtml from "../../widgets/pear-queue/index.html?raw";

const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call(cmd, args) {
  if (!inTauri) return mock(cmd, args);
  return invoke(cmd, args);
}

// Browser-only sample data so the UI renders without a host.
const MOCK_AUTOMATIONS = [
  {
    id: "demo",
    root: {
      kind: "block",
      body: [
        {
          kind: "action",
          plugin: "notify",
          action: "toast",
          config: { message: "Now: @pear.title \\@literal {{upper pear.album}} \\{{raw}}", level: "info" },
        },
        { kind: "action", plugin: "pear", action: "set-volume", config: { volume: 70 } },
        { kind: "wait-for", wait: "delay", ms: "1000" },
        { kind: "action", plugin: "pear", action: "play", config: {} },
      ],
    },
  },
];
const MOCK_PROFILES = [
  {
    id: "main",
    name: "Main",
    triggers: [
      {
        trigger: "pear.song-changed",
        filter: "",
        sequence: {
          kind: "block",
          body: [
            { kind: "action", plugin: "pear", action: "pause", config: {} },
            { kind: "wait-for", wait: "delay", ms: "500" },
            { kind: "action", plugin: "pear", action: "play", config: {} },
          ],
        },
      },
    ],
  },
];
// Locale bundles mirroring the host `core` namespace (lang/*.json), so language
// switching is fully demonstrable in the browser without a host.
const MOCK_LOCALES = {
  en: {
    core: {
      app: { name: "Quicktys" },
      menu: {
        dashboard: "Dashboard", profiles: "Profiles", automations: "Automations",
        webview: "Webview", variables: "Variables", settings: "Settings",
      },
      actions: { save: "Save", cancel: "Cancel", new: "New", delete: "Delete", close: "Close" },
      dashboard: {
        title: "Dashboard", welcome: "Welcome to Quicktys", subtitle: "Your streaming automation, rebuilt.",
        plugins: "Loaded plugins", modules: "Loaded modules", profiles: "Profiles",
        automations: "Automations", widgets: "Widgets", webview: "Webview",
        quickstart: "Open Automations to build a sequence, or Profiles to bind triggers to it.",
        edit: "Edit", done: "Done", addWidget: "Add widget", widget: "Widget",
        source: "Shows", size: "Size", remove: "Remove",
        empty: "Empty dashboard — click Edit, then Add widget.",
      },
      profiles: {
        title: "Profiles", new: "New profile", none: "No profiles yet.",
        selectHint: "Select a profile to edit it, or create one.",
        name: "Name", namePlaceholder: "My profile", activation: "Activation condition",
        activationHint: "Boolean expression over state. Empty means always active.",
        alwaysActive: "Always active", conditional: "Conditional", triggers: "Triggers",
        noTriggers: "No triggers bound. Add one to run an automation on an event.",
        addTrigger: "Add trigger", trigger: "Trigger", triggerPlaceholder: "e.g. twitch.chat",
        automation: "Automation", automationPlaceholder: "Pick an automation",
        filter: "Filter", filterPlaceholder: "optional", saved: "Profile saved", untitledTrigger: "New trigger",
      },
      webview: {
        title: "Webview", new: "New overlay", none: "No overlays yet.", untitled: "New overlay",
        deleteOverlay: "Delete overlay", searchWidgets: "Search widgets…", noWidget: "No match",
        dropHint: "Add a widget, then drag to place and resize it.",
        copy: "Copy URL", copied: "URL copied", urlHint: "Add this as a Browser source in OBS.",
        placeholder: "Browser-source overlays and remote interfaces will live here.",
      },
      variables: { title: "Variables", none: "No state variables yet.", refresh: "Refresh", name: "Name", value: "Value" },
      settings: {
        title: "Settings", language: "Language",
        languageHint: "Applies immediately across the whole app.",
        appearance: "Appearance", theme: "Theme", about: "About", version: "Version",
      },
      buttons: { group: "Buttons", clicked: "Button clicked", bind: "Bind action", edit: "Edit action", actionProfile: "Button · {label}" },
    },
    plugin: { pear: { triggers: { "song-changed": { name: "Song changed" } } } },
    widget: { button: { name: "Button", config: { label: "Label", icon: "Icon (pi/mdi class)", color: "Color" } } },
  },
  fr: {
    core: {
      app: { name: "Quicktys" },
      menu: {
        dashboard: "Tableau de bord", profiles: "Profils", automations: "Automations",
        webview: "Webview", variables: "Variables", settings: "Réglages",
      },
      actions: { save: "Enregistrer", cancel: "Annuler", new: "Nouveau", delete: "Supprimer", close: "Fermer" },
      dashboard: {
        title: "Tableau de bord", welcome: "Bienvenue dans Quicktys", subtitle: "Votre automatisation de stream, réinventée.",
        plugins: "Plugins chargés", modules: "Modules chargés", profiles: "Profils",
        automations: "Automations", widgets: "Widgets", webview: "Webview",
        quickstart: "Ouvrez Automations pour construire une séquence, ou Profils pour y lier des déclencheurs.",
        edit: "Éditer", done: "Terminé", addWidget: "Ajouter un widget", widget: "Widget",
        source: "Affiche", size: "Taille", remove: "Retirer",
        empty: "Tableau de bord vide — cliquez sur Éditer, puis Ajouter un widget.",
      },
      profiles: {
        title: "Profils", new: "Nouveau profil", none: "Aucun profil pour l'instant.",
        selectHint: "Sélectionnez un profil à modifier, ou créez-en un.",
        name: "Nom", namePlaceholder: "Mon profil", activation: "Condition d'activation",
        activationHint: "Expression booléenne sur l'état. Vide = toujours actif.",
        alwaysActive: "Toujours actif", conditional: "Conditionnel", triggers: "Déclencheurs",
        noTriggers: "Aucun déclencheur lié. Ajoutez-en un pour lancer une automation sur un événement.",
        addTrigger: "Ajouter un déclencheur", trigger: "Déclencheur", triggerPlaceholder: "ex. twitch.chat",
        automation: "Automation", automationPlaceholder: "Choisir une automation",
        filter: "Filtre", filterPlaceholder: "optionnel", saved: "Profil enregistré", untitledTrigger: "Nouveau déclencheur",
      },
      webview: {
        title: "Webview", new: "Nouvel overlay", none: "Aucun overlay.", untitled: "Nouvel overlay",
        deleteOverlay: "Supprimer l'overlay", searchWidgets: "Rechercher un widget…", noWidget: "Aucun résultat",
        dropHint: "Ajoutez un widget, puis glissez pour le placer et le redimensionner.",
        copy: "Copier l'URL", copied: "URL copiée", urlHint: "Ajoutez ceci comme source Navigateur dans OBS.",
        placeholder: "Les overlays source-navigateur et interfaces distantes vivront ici.",
      },
      variables: { title: "Variables", none: "Aucune variable d'état pour l'instant.", refresh: "Rafraîchir", name: "Nom", value: "Valeur" },
      settings: {
        title: "Réglages", language: "Langue",
        languageHint: "S'applique immédiatement à toute l'application.",
        appearance: "Apparence", theme: "Thème", about: "À propos", version: "Version",
      },
      buttons: { group: "Boutons", clicked: "Bouton cliqué", bind: "Lier une action", edit: "Modifier l'action", actionProfile: "Bouton · {label}" },
    },
    plugin: { pear: { triggers: { "song-changed": { name: "Changement de musique" } } } },
    widget: { button: { name: "Bouton", config: { label: "Libellé", icon: "Icône (classe pi/mdi)", color: "Couleur" } } },
  },
};

// Browser-mock catalogue / installed / consent, so the Marketplace view renders
// without a host.
const MOCK_MARKET = [
  { id: "obs", kind: "module", version: "1.4.0", publisher: "acme", certified: true, capabilities: ["net"], requires: [], name: "OBS Studio", summary: "Drive OBS over websocket." },
  { id: "obs", kind: "plugin", version: "2.1.0", publisher: "acme", certified: false, capabilities: [], requires: [{ module: "obs", version: "^1" }], name: "OBS", summary: "Scenes, sources, filters, recording." },
  { id: "discord", kind: "plugin", version: "1.0.0", publisher: "acme", certified: false, capabilities: [], requires: [{ module: "http", version: "^1" }], name: "Discord", summary: "Post to a Discord webhook." },
  { id: "sound", kind: "module", version: "0.3.0", publisher: "acme", certified: true, capabilities: ["audio"], requires: [], name: "Sound", summary: "Play audio + text-to-speech." },
];
const MOCK_INSTALLED = [
  { id: "pear", kind: "module", version: "0.1.0", source: "dev", explicit: true, required_by: ["plugin/pear"] },
  { id: "pear", kind: "plugin", version: "0.1.0", source: "dev", explicit: true, required_by: [] },
];
const MOCK_CONSENT = [
  { id: "obs", name: "OBS Studio", version: "1.4.0", declared: ["net"], granted: ["net"], pending: [], reviewed: true },
  { id: "sound", name: "Sound", version: "0.3.0", declared: ["audio"], granted: [], pending: ["audio"], reviewed: false },
];

function mock(cmd, args) {
  switch (cmd) {
    case "list_plugins":
      return [
        {
          id: "pear",
          name: "Pear Desktop",
          version: "0.1.0",
          icon: "pi pi-play",
          color: "#7ec27e",
          actions: ["play", "pause", "toggle", "next", "previous", "like", "set-volume", "seek-to", "set-queue-index"],
          triggers: ["song-changed"],
          state: ["title", "artist", "artists", "cover", "url", "album", "video-id", "lyrics", "playlist"],
        },
        {
          id: "notify",
          name: "Notify",
          version: "0.1.0",
          icon: "mdi mdi-bell",
          color: "#e0a458",
          actions: ["log", "toast"],
          triggers: [],
          state: [],
        },
      ];
    case "list_modules":
      return [
        { id: "pear", name: "Pear Desktop", version: "0.1.0", services: ["play", "pause", "poll"] },
        {
          id: "webview",
          name: "Webview",
          version: "0.1.0",
          services: ["list-overlays", "get-overlay", "upsert-overlay", "delete-overlay"],
          view: { icon: "pi pi-desktop", label: "core.menu.webview", entry: "index.html" },
        },
      ];
    case "list_widgets":
      return [
        {
          id: "info",
          kind: "widget",
          name: "Info box",
          icon: "pi pi-hashtag",
          template: "info",
          config: {
            type: "Object",
            properties: {
              source: {
                type: "String",
                enum: ["plugins", "modules", "profiles", "automations", "widgets", "webview"],
              },
            },
          },
        },
        {
          id: "button",
          kind: "widget",
          name: "Button",
          icon: "pi pi-bolt",
          template: "button",
          config: {
            type: "Object",
            properties: {
              label: { type: "String", default: "Button" },
              icon: { type: "String", default: "" },
              color: { type: "String", default: "#7c5cff" },
            },
          },
        },
        {
          id: "pear-now-playing",
          kind: "widget",
          name: "Now Playing (Pear)",
          icon: "pi pi-play-circle",
          runtime: "html",
          html: pearNowPlayingHtml,
          config: { type: "Object", properties: {} },
        },
        {
          id: "pear-queue",
          kind: "widget",
          name: "Queue (Pear)",
          icon: "pi pi-list",
          runtime: "html",
          html: pearQueueHtml,
          defaultW: 2,
          defaultH: 3,
          config: { type: "Object", properties: {} },
        },
      ];
    case "action_schema":
      if (args?.plugin === "notify")
        return {
          type: "Object",
          properties: {
            message: { type: "String", template: true },
            level: { type: "String" },
          },
        };
      return { type: "Object", properties: { volume: { type: "Number", min: 0, max: 100 } } };
    case "list_resources":
      if (args?.kind === "profile") return MOCK_PROFILES;
      if (args?.kind === "automation") return MOCK_AUTOMATIONS;
      return [];
    case "upsert_resource": {
      // In-memory persistence so create/edit/delete work in the browser demo.
      const store = args?.kind === "automation" ? MOCK_AUTOMATIONS : MOCK_PROFILES;
      const i = store.findIndex((r) => r.id === args.value.id);
      if (i === -1) store.push(args.value);
      else store[i] = args.value;
      return null;
    }
    case "delete_resource": {
      const store = args?.kind === "automation" ? MOCK_AUTOMATIONS : MOCK_PROFILES;
      const i = store.findIndex((r) => r.id === args.id);
      if (i !== -1) store.splice(i, 1);
      return null;
    }
    case "marketplace_refresh":
      return MOCK_MARKET.length;
    case "marketplace_search":
      return MOCK_MARKET.filter(
        (p) =>
          (!args?.kind || p.kind === args.kind) &&
          (!args?.query || `${p.id} ${p.name ?? ""}`.toLowerCase().includes(args.query.toLowerCase())),
      );
    case "list_installed":
      return MOCK_INSTALLED;
    case "consent_status":
      return MOCK_CONSENT;
    case "list_updates":
      return [];
    case "install_from_registry":
    case "uninstall_package":
    case "sideload_package":
    case "purge_orphans":
    case "grant_capabilities":
    case "revoke_capability":
      return [];
    case "boot_status":
      return { done: true, index: 0, total: 0, current: "" };
    case "set_locale":
    case "run_automation":
    case "run_sequence":
    case "emit_trigger":
      return null;
    case "available_locales":
      return ["en", "fr"];
    case "locale_bundle":
      return MOCK_LOCALES[args?.locale] ?? MOCK_LOCALES.en;
    case "get_state":
      return {
        "pear.title": "Nightcall",
        "pear.artist": "Kavinsky & Lovefoxxx",
        "pear.artists": ["Kavinsky", "Lovefoxxx"],
        "pear.album": "OutRun",
        "pear.cover": "http://img/cover.jpg",
        "pear.url": "http://music/watch?v=abc",
        "pear.playing": true,
        "pear.queue": [
          { title: "Nightcall", artist: "Kavinsky & Lovefoxxx", thumbnail: "", videoId: "abc", selected: true },
          { title: "Odd Look", artist: "Kavinsky", thumbnail: "", videoId: "def", selected: false },
          { title: "Protovision", artist: "Kavinsky", thumbnail: "", videoId: "ghi", selected: false },
          { title: "Roadgame", artist: "Kavinsky", thumbnail: "", videoId: "jkl", selected: false },
        ],
      };
    default:
      return [];
  }
}

export const ipc = {
  // True inside the desktop shell: the host emits real execution telemetry, so
  // the editor drives its "now running" highlight from live events rather than a
  // client-side timing estimate.
  isNative: inTauri,
  // Boot-time extension loading progress (drives the startup loading screen).
  bootStatus: () => call("boot_status"),
  listPlugins: () => call("list_plugins"),
  listModules: () => call("list_modules"),
  listWidgets: () => call("list_widgets"),
  actionSchema: (plugin, action) => call("action_schema", { plugin, action }),
  listResources: (kind) => call("list_resources", { kind }),
  getResource: (kind, id) => call("get_resource", { kind, id }),
  upsertResource: (kind, value) => call("upsert_resource", { kind, value }),
  deleteResource: (kind, id) => call("delete_resource", { kind, id }),
  runAutomation: (id, context) => call("run_automation", { id, context }),
  runSequence: (root) => call("run_sequence", { root }),
  callModule: (module, service, args = {}) => call("call_module", { module, service, args }),
  // Tolerant variant for optional/read-only calls: a module that isn't installed
  // (e.g. `webview`, `sound`) returns null instead of rejecting, so a missing
  // extension never breaks boot or a view's mount.
  callModuleSafe: async (module, service, args = {}) => {
    try {
      return await call("call_module", { module, service, args });
    } catch {
      return null;
    }
  },
  getState: () => call("get_state"),
  availableLocales: () => call("available_locales"),
  localeBundle: (locale) => call("locale_bundle", { locale }),
  setLocale: (locale) => call("set_locale", { locale }),
  // --- marketplace (M6.6) + install (M6.2) ---
  marketplaceRefresh: () => call("marketplace_refresh"),
  marketplaceSearch: (query, kind = null) => call("marketplace_search", { query, kind }),
  installFromRegistry: (kind, id) => call("install_from_registry", { kind, id }),
  listUpdates: () => call("list_updates"),
  listInstalled: () => call("list_installed"),
  sideloadPackage: (path) => call("sideload_package", { path }),
  uninstallPackage: (slug) => call("uninstall_package", { slug }),
  purgeOrphans: () => call("purge_orphans"),
  // --- capability consent (M6.7) ---
  consentStatus: () => call("consent_status"),
  grantCapabilities: (id, caps) => call("grant_capabilities", { id, caps }),
  revokeCapability: (id, cap) => call("revoke_capability", { id, cap }),
  // --- extension settings ---
  getSettings: (id) => call("get_settings", { id }),
  setSettings: (id, values) => call("set_settings", { id, values }),
  // --- entitlements / licensing (M6.8) ---
  enterLicenseKey: (key) => call("enter_license_key", { key }),
  listEntitlements: () => call("list_entitlements"),
  // Fire a trigger on the bus (dashboard button widget → button.clicked).
  emitTrigger: (trigger, context = {}) => call("emit_trigger", { trigger, context }),
  onEvent: (handler) =>
    inTauri ? listen("app://event", (e) => handler(e.payload)) : Promise.resolve(() => {}),
};
