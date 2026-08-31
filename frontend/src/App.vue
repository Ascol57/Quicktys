<!-- App shell (CastMate-style): a top menu bar, a resizable left navigation rail,
     and a tabbed workspace on the right. Sidebar sections open singleton tabs. -->
<script setup>
import { computed, onMounted, onUnmounted, provide, ref } from "vue";
import { useI18n } from "vue-i18n";
import Toast from "primevue/toast";
import { useToast } from "primevue/usetoast";
import Sidebar from "./components/Sidebar.vue";
import OnboardingDialog from "./components/OnboardingDialog.vue";
import DashboardView from "./components/views/DashboardView.vue";
import ProfileView from "./components/views/ProfileView.vue";
import ViewFrame from "./components/ViewFrame.vue";
import LoadingOverlay from "./components/LoadingOverlay.vue";
import AutomationsView from "./components/views/AutomationsView.vue";
import VariablesView from "./components/views/VariablesView.vue";
import MarketplaceView from "./components/views/MarketplaceView.vue";
import SettingsView from "./components/views/SettingsView.vue";
import { tabInfo } from "./nav.js";
import { useProfiles } from "./stores/profiles.js";
import { useSettings } from "./stores/settings.js";
import { useDashboard } from "./stores/dashboard.js";
import { ipc } from "./ipc.js";
import { switchLocale } from "./i18n.js";

const { t, te } = useI18n();
const profilesStore = useProfiles();
const dashboard = useDashboard();
const settings = useSettings();
const toast = useToast();

// Toasts pushed by plugins (`reactivity.notify` → AppEvent::Toast).
const LEVELS = { info: "info", success: "success", warn: "warn", warning: "warn", error: "error" };
// First-run onboarding: propose installing some plugins. Gated on a local flag
// so it appears only on the very first launch (dismissing/installing sets it).
const ONBOARDED_KEY = "quicktys.onboarded";
const showOnboarding = ref(false);
function finishOnboarding() {
  try {
    localStorage.setItem(ONBOARDED_KEY, "1");
  } catch {
    /* localStorage unavailable — just close */
  }
  showOnboarding.value = false;
}

// --- boot loading screen ---------------------------------------------------
// The host loads extension WASM one after another after the window shows; we
// cover the app with a progress overlay until it emits `BootReady`. Everything
// that needs extensions (below) awaits `bootDone`.
const booting = ref(true);
const bootInfo = ref({ index: 0, total: 0, current: "" });
let resolveBoot;
const bootDone = new Promise((res) => (resolveBoot = res));
function finishBoot() {
  booting.value = false;
  resolveBoot?.();
  resolveBoot = null;
}

let unlistenToast = null;
onMounted(async () => {
  try {
    showOnboarding.value = !localStorage.getItem(ONBOARDED_KEY);
  } catch {
    showOnboarding.value = false;
  }
  unlistenToast = await ipc.onEvent((e) => {
    if (e?.type === "toast") {
      toast.add({ severity: LEVELS[e.level] ?? "info", detail: e.message, life: 3000 });
    } else if (e?.type === "stateChanged" && OBS_KEYS.has(e.key)) {
      // Keep the OBS autocomplete lists (scenes/sources/filters/hotkeys) live.
      obsLists.value = { ...obsLists.value, [e.key]: asList(e.value) };
    } else if (e?.type === "bootProgress") {
      booting.value = true;
      bootInfo.value = { index: e.index, total: e.total, current: e.id };
    } else if (e?.type === "bootReady") {
      finishBoot();
    }
  });
  // Subscribed first (above), so a `BootReady` between now and here isn't missed;
  // then reconcile with the current status (covers a fully-booted app on mount).
  try {
    const bs = await ipc.bootStatus();
    if (bs?.done) finishBoot();
    else bootInfo.value = { index: bs?.index ?? 0, total: bs?.total ?? 0, current: bs?.current ?? "" };
  } catch {
    finishBoot(); // fail open — never trap the UI behind the overlay
  }
});
onUnmounted(() => unlistenToast && unlistenToast());

// Section views (leaf tabs). Dynamic `profile:<id>` tabs resolve to ProfileView;
// `view:<moduleId>` tabs are pages contributed by extensions (rendered in a
// sandboxed ViewFrame). The `webview` tab is now one of these, not hardcoded.
const VIEWS = {
  dashboard: DashboardView,
  automations: AutomationsView,
  variables: VariablesView,
  marketplace: MarketplaceView,
  settings: SettingsView,
};

// Views contributed by modules (from list_modules) — [{ id, icon, label, entry, name }].
const contributedViews = ref([]);
const viewById = (id) => contributedViews.value.find((v) => v.id === id) ?? null;
provide("contributed-views", contributedViews);

// Resolve a tab id to what to render + how to label it.
function resolveTab(id) {
  const info = tabInfo(id);
  if (info?.profileId !== undefined) {
    return {
      component: ProfileView,
      props: { profileId: info.profileId },
      icon: info.icon,
      title: profilesStore.name(info.profileId),
      closable: true,
    };
  }
  if (id.startsWith("view:")) {
    const v = viewById(id.slice("view:".length));
    return {
      component: ViewFrame,
      props: { moduleId: v?.id ?? "", entry: v?.entry ?? "index.html" },
      icon: v?.icon ?? "pi pi-circle",
      title: v?.label && te(v.label) ? t(v.label) : v?.name ?? id,
      closable: true,
    };
  }
  return {
    component: VIEWS[id],
    props: {},
    icon: info?.icon ?? "pi pi-circle",
    title: info?.labelKey ? t(info.labelKey) : id,
    closable: info?.closable ?? true,
  };
}
const isOpenable = (id) =>
  id.startsWith("profile:") || (id.startsWith("view:") && !!viewById(id.slice(5))) || !!VIEWS[id];

// --- shared app context (provided to views) --------------------------------
const plugins = ref([]);
const locales = ref(["en"]);
const locale = ref("en");

// Names of the alert widgets the user has placed (dashboard + overlays), so the
// Alerts action can autocomplete which alert to fire. Overlays are a host
// resource; the dashboard comes from the store (reactive).
const overlayAlertNames = ref([]);
async function refreshOverlayAlerts() {
  const overlays = (await ipc.callModuleSafe("webview", "list-overlays", {})) ?? [];
  overlayAlertNames.value = overlays
    .flatMap((o) => o.widgets ?? [])
    .filter((w) => w.type === "alert")
    .map((w) => w.config?.name)
    .filter(Boolean);
}
// Overlays are module-owned; the webview editor calls this after mutations.
provide("refresh-overlays", refreshOverlayAlerts);
const alertNames = computed(() => {
  const fromDash = dashboard.widgets
    .filter((w) => w.type === "alert")
    .map((w) => w.config?.name)
    .filter(Boolean);
  return [...new Set([...fromDash, ...overlayAlertNames.value])];
});

// Live OBS lists (scenes / sources) for action autocomplete — kept current from
// the `obs.scenes` / `obs.inputs` reactive state the OBS plugin publishes.
const asList = (v) => (Array.isArray(v) ? v.filter((x) => typeof x === "string") : []);
// obs.<key> reactive lists that feed `suggest` autocomplete, keyed by suggest id.
const OBS_SUGGEST = {
  "obs-scene": "obs.scenes",
  "obs-input": "obs.inputs",
  "obs-filter": "obs.filters",
  "obs-hotkey": "obs.hotkeys",
  "obs-transition": "obs.transitions",
};
const OBS_KEYS = new Set(Object.values(OBS_SUGGEST));
const obsLists = ref({});

// Installed TTS voices/languages (from the sound module), for the tts-voice field.
const ttsVoices = ref([]);

// The app exposes suggestion lists to schema fields declaring `suggest: "<src>"`.
provide("field-suggestions", (source) => {
  if (source === "alert") return alertNames.value;
  if (source === "tts-voice") return ttsVoices.value;
  const stateKey = OBS_SUGGEST[source];
  if (stateKey) return obsLists.value[stateKey] ?? [];
  return [];
});

onMounted(async () => {
  // Extensions load in the background after the window shows — wait for the
  // loader to finish so plugin/module/widget lists are complete.
  await bootDone;
  plugins.value = await ipc.listPlugins();
  // Views contributed by modules (a module's manifest may declare `view`).
  contributedViews.value = ((await ipc.listModules()) ?? [])
    .filter((m) => m.view)
    .map((m) => ({ id: m.id, name: m.name, icon: m.view.icon, label: m.view.label, entry: m.view.entry || "index.html" }));
  const available = await ipc.availableLocales();
  if (available?.length) locales.value = available;
  await profilesStore.load();
  await refreshOverlayAlerts();
  // Seed OBS autocomplete lists from current state (they update live after).
  const st = (await ipc.getState()) ?? {};
  const seed = {};
  for (const key of OBS_KEYS) seed[key] = asList(st[key]);
  obsLists.value = seed;
  // Installed TTS voices (for the Sound "Text to speech" voice field).
  ttsVoices.value = asList(await ipc.callModuleSafe("sound", "voices", {}));
  // Push the saved Pear / OBS endpoints to their modules before the first poll
  // (3s), so their connections target the configured endpoints from the start.
  settings.applyPear();
  settings.applyObs();
  // Deep-link: `?locale=fr` presets the language, `#automations` / `#profile:main`
  // opens that tab.
  const wanted = new URLSearchParams(location.search).get("locale");
  if (wanted && locales.value.includes(wanted)) await changeLocale(wanted);
  const initial = location.hash.slice(1);
  if (initial && isOpenable(initial)) open(initial);
});

async function changeLocale(code) {
  await switchLocale(code);
  locale.value = code;
}

provide("plugins", plugins);
provide("locales", locales);
provide("locale", locale);
provide("change-locale", changeLocale);
provide("open-section", open);

// --- workspace tabs --------------------------------------------------------
const tabs = ref(["dashboard"]);
const activeId = ref("dashboard");

function open(id) {
  if (!isOpenable(id)) return;
  if (!tabs.value.includes(id)) tabs.value.push(id);
  activeId.value = id;
}
function close(id) {
  if (resolveTab(id).closable === false) return;
  const i = tabs.value.indexOf(id);
  if (i === -1) return;
  tabs.value.splice(i, 1);
  if (activeId.value === id) {
    activeId.value = tabs.value[Math.max(0, i - 1)] ?? tabs.value[0] ?? "dashboard";
    if (!tabs.value.length) tabs.value = ["dashboard"];
  }
}
const openTabs = computed(() => tabs.value.map((id) => ({ id, ...resolveTab(id) })));

// --- resizable sidebar -----------------------------------------------------
const MIN = 170;
const MAX = 460;
const sidebarWidth = ref(
  Math.min(MAX, Math.max(MIN, Number(localStorage.getItem("qt.sidebarWidth")) || 232)),
);
let dragging = false;
function startResize() {
  dragging = true;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  window.addEventListener("mousemove", onResize);
  window.addEventListener("mouseup", stopResize);
}
function onResize(e) {
  if (!dragging) return;
  sidebarWidth.value = Math.min(MAX, Math.max(MIN, e.clientX));
}
function stopResize() {
  dragging = false;
  document.body.style.cursor = "";
  document.body.style.userSelect = "";
  localStorage.setItem("qt.sidebarWidth", String(sidebarWidth.value));
  window.removeEventListener("mousemove", onResize);
  window.removeEventListener("mouseup", stopResize);
}
</script>

<template>
  <div class="app">
    <Toast position="bottom-right" />
    <LoadingOverlay v-if="booting" :info="bootInfo" />
    <OnboardingDialog v-if="showOnboarding && !booting" @done="finishOnboarding" />
    <!-- top menu bar -->
    <header class="topbar">
      <div class="brand"><i class="pi pi-bolt" /> {{ t("core.app.name") }}</div>
      <nav class="menu">
        <span class="menu__item" @click="open('automations')">{{ t("core.menu.automations") }}</span>
        <span class="menu__item" @click="open('settings')">{{ t("core.menu.settings") }}</span>
      </nav>
      <div class="topbar__right">
        <span class="muted">{{ plugins.length }} plugin(s)</span>
      </div>
    </header>

    <!-- body: sidebar | handle | workspace -->
    <div class="body">
      <aside class="rail" :style="{ width: sidebarWidth + 'px' }">
        <Sidebar :active="activeId" @select="open" />
      </aside>
      <div class="handle" @mousedown.prevent="startResize" />

      <main class="workspace">
        <div class="tabbar">
          <div
            v-for="tab in openTabs"
            :key="tab.id"
            class="tab"
            :class="{ active: tab.id === activeId }"
            @click="activeId = tab.id"
          >
            <i :class="tab.icon" />
            <span>{{ tab.title }}</span>
            <i
              v-if="tab.closable !== false"
              class="tab__close pi pi-times"
              @click.stop="close(tab.id)"
            />
          </div>
        </div>

        <div class="view">
          <KeepAlive>
            <component :is="resolveTab(activeId).component" v-bind="resolveTab(activeId).props" :key="activeId" />
          </KeepAlive>
        </div>
      </main>
    </div>
  </div>
</template>

<style>
html,
body,
#app {
  height: 100%;
  margin: 0;
}
body {
  font-family: system-ui, sans-serif;
  background: #161616;
  color: #eee;
}
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

/* top bar */
.topbar {
  display: flex;
  align-items: center;
  gap: 1.5rem;
  padding: 0.5rem 1.1rem;
  border-bottom: 1px solid #2a2a2a;
  flex: 0 0 auto;
  background: #1b1b1b;
}
.brand {
  font-weight: 700;
  font-size: 1.05rem;
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.brand i {
  color: #7ec27e;
}
.menu {
  display: flex;
  gap: 0.35rem;
}
.menu__item {
  padding: 0.25rem 0.6rem;
  border-radius: 5px;
  font-size: 0.88rem;
  opacity: 0.8;
  cursor: pointer;
}
.menu__item:hover {
  background: #262626;
  opacity: 1;
}
.topbar__right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 1rem;
}

/* body layout */
.body {
  flex: 1;
  min-height: 0;
  display: flex;
}
.rail {
  flex: 0 0 auto;
  background: #191919;
  border-right: 1px solid #232323;
  min-height: 0;
}
.handle {
  width: 5px;
  flex: 0 0 auto;
  cursor: col-resize;
  background: transparent;
}
.handle:hover {
  background: #7ec27e55;
}

/* workspace + tabs */
.workspace {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.tabbar {
  display: flex;
  align-items: stretch;
  gap: 2px;
  padding: 0 0.4rem;
  background: #1b1b1b;
  border-bottom: 1px solid #2a2a2a;
  flex: 0 0 auto;
  overflow-x: auto;
}
.tab {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.5rem 0.85rem;
  font-size: 0.85rem;
  color: #bcbcbc;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  white-space: nowrap;
}
.tab:hover {
  color: #fff;
}
.tab.active {
  color: #fff;
  border-bottom-color: #7ec27e;
  background: #202020;
}
.tab__close {
  font-size: 0.7rem;
  opacity: 0.5;
  border-radius: 3px;
  padding: 2px;
}
.tab__close:hover {
  opacity: 1;
  background: #333;
}
.view {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.muted {
  opacity: 0.6;
  font-size: 0.85rem;
}
</style>
