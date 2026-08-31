<!-- Marketplace: browse & install from the registry (M6.6), manage installed
     extensions (M6.2), and review per-module capability consent (M6.7). All the
     data comes from the host backend; degrades to sample data in the browser. -->
<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import InputText from "primevue/inputtext";
import Button from "primevue/button";
import { useToast } from "primevue/usetoast";
import ExtensionSetup from "../ExtensionSetup.vue";
import { ipc } from "../../ipc.js";

const { t } = useI18n();
const toast = useToast();

const TABS = ["browse", "installed", "licenses", "consent"];
const tab = ref("browse");

// --- browse ---------------------------------------------------------------
const query = ref("");
const results = ref([]);
const busy = ref("");

// Only plugins and widgets are browsable — modules are pulled in automatically
// as dependencies, never installed directly.
const browse = computed(() => results.value.filter((e) => e.kind === "plugin" || e.kind === "widget"));

function cmpVersion(a, b) {
  const pa = String(a).split(".").map(Number);
  const pb = String(b).split(".").map(Number);
  for (let i = 0; i < 3; i += 1) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d > 0 ? 1 : -1;
  }
  return 0;
}

// Index lookup by slug, so we can inspect a plugin's required modules.
const bySlug = computed(() => {
  const m = {};
  for (const e of results.value) m[`${e.kind}/${e.id}`] = e;
  return m;
});

// Whether the user holds an entitlement covering (kind, id) (wildcards allowed).
function isEntitled(kind, id) {
  return entitlements.value.some(
    (e) => (e.kind === "*" || e.kind === kind) && (e.id === "*" || e.id === id),
  );
}

// Paid packages the entry pulls in (itself + its required modules) that the user
// isn't entitled to yet — what needs buying before it can be installed.
function lockedPaid(e) {
  const seen = new Set();
  const locked = [];
  const visit = (entry) => {
    if (!entry) return;
    const slug = `${entry.kind}/${entry.id}`;
    if (seen.has(slug)) return;
    seen.add(slug);
    if (entry.license?.model && entry.license.model !== "free" && !isEntitled(entry.kind, entry.id)) {
      locked.push({ kind: entry.kind, id: entry.id, sku: entry.license.sku || entry.id });
    }
    for (const r of entry.requires ?? []) visit(bySlug.value[`module/${r.module}`]);
  };
  visit(e);
  return locked;
}

// The install state of a browse entry: needs buying, fresh install, update, or done.
function state(e) {
  const locked = lockedPaid(e);
  if (locked.length) return { status: "buy", locked };
  const rec = installed.value.find((r) => r.kind === e.kind && r.id === e.id);
  if (!rec) return { status: "install" };
  if (cmpVersion(e.version, rec.version) > 0) return { status: "update", rec };
  return { status: "installed", rec };
}

// Buy → go to the Licenses tab, telling the user which SKUs to unlock.
function buy(e) {
  const skus = lockedPaid(e).map((l) => l.sku).join(", ");
  tab.value = "licenses";
  toast.add({ severity: "info", detail: t("core.marketplace.buyHint", { skus }), life: 6000 });
}

async function refresh() {
  busy.value = "refresh";
  try {
    const n = await ipc.marketplaceRefresh();
    toast.add({ severity: "success", detail: t("core.marketplace.refreshed", { n }), life: 2000 });
    await search();
  } catch (e) {
    toast.add({ severity: "error", detail: String(e), life: 4000 });
  } finally {
    busy.value = "";
  }
}
async function search() {
  results.value = (await ipc.marketplaceSearch(query.value)) ?? [];
}
async function install(entry) {
  busy.value = `${entry.kind}/${entry.id}`;
  try {
    const recs = (await ipc.installFromRegistry(entry.kind, entry.id)) ?? [];
    toast.add({ severity: "success", detail: t("core.marketplace.installed", { n: recs.length }), life: 2500 });
    await loadInstalled();
    // Post-install setup (settings + permissions) for what was just installed.
    if (recs.length) postInstall.value = recs;
  } catch (e) {
    const msg = String(e);
    // A paid package needs a license — send the user to the Licenses tab.
    if (/entitlement|license|paid package/i.test(msg)) {
      tab.value = "licenses";
      toast.add({ severity: "warn", detail: t("core.marketplace.licenses.needed"), life: 6000 });
    } else {
      toast.add({ severity: "error", detail: msg, life: 5000 });
    }
  } finally {
    busy.value = "";
  }
}
// Records to run the post-install setup wizard on (null = closed).
const postInstall = ref(null);

// --- installed ------------------------------------------------------------
const installed = ref([]);
const updates = ref([]);
const sideloadPath = ref("");

async function loadInstalled() {
  installed.value = (await ipc.listInstalled()) ?? [];
  updates.value = (await ipc.listUpdates()) ?? [];
}
function updateFor(rec) {
  return updates.value.find((u) => u.id === rec.id && u.kind === rec.kind);
}
async function uninstall(rec) {
  const orphans = (await ipc.uninstallPackage(`${rec.kind}/${rec.id}`)) ?? [];
  if (orphans.length) toast.add({ severity: "info", detail: t("core.marketplace.orphans", { n: orphans.length }), life: 3000 });
  await loadInstalled();
}
async function purge() {
  const removed = (await ipc.purgeOrphans()) ?? [];
  toast.add({ severity: "success", detail: t("core.marketplace.purged", { n: removed.length }), life: 2500 });
  await loadInstalled();
}
async function sideload() {
  if (!sideloadPath.value.trim()) return;
  try {
    await ipc.sideloadPackage(sideloadPath.value.trim());
    sideloadPath.value = "";
    toast.add({ severity: "success", detail: t("core.marketplace.sideloaded"), life: 2500 });
    await loadInstalled();
  } catch (e) {
    toast.add({ severity: "error", detail: String(e), life: 5000 });
  }
}

// --- consent --------------------------------------------------------------
const consent = ref([]);
async function loadConsent() {
  consent.value = (await ipc.consentStatus()) ?? [];
}
async function toggleCap(mod, cap) {
  const granted = mod.granted.includes(cap);
  if (granted) await ipc.revokeCapability(mod.id, cap);
  else await ipc.grantCapabilities(mod.id, [...mod.granted, cap]);
  await loadConsent();
}

// --- licenses / entitlements (M6.8) --------------------------------------
const licenseInput = ref("");
const entitlements = ref([]);
async function loadLicenses() {
  entitlements.value = (await ipc.listEntitlements()) ?? [];
}
async function submitLicense() {
  const key = licenseInput.value.trim();
  if (!key) return;
  try {
    const ent = await ipc.enterLicenseKey(key);
    licenseInput.value = "";
    toast.add({ severity: "success", detail: t("core.marketplace.licenses.added", { sku: ent?.sku ?? "" }), life: 3000 });
    await loadLicenses();
  } catch (e) {
    toast.add({ severity: "error", detail: String(e), life: 6000 });
  }
}

async function loadAll() {
  await Promise.all([search(), loadInstalled(), loadConsent(), loadLicenses()]);
}

let unlisten = null;
onMounted(async () => {
  await loadAll();
  unlisten = await ipc.onEvent((e) => {
    if (e?.type === "extensionsChanged") {
      loadInstalled();
      loadLicenses(); // a new license flips locked "Buy" cards to "Install"
    } else if (e?.type === "consentChanged") loadConsent();
  });
});
onUnmounted(() => unlisten && unlisten());
</script>

<template>
  <div class="market">
    <ExtensionSetup v-if="postInstall" :records="postInstall" @done="postInstall = null" />
    <div class="tabs">
      <button v-for="id in TABS" :key="id" class="tab" :class="{ active: tab === id }" @click="tab = id">
        {{ t(`core.marketplace.tab.${id}`) }}
      </button>
    </div>

    <!-- Browse -->
    <section v-if="tab === 'browse'" class="pane">
      <div class="bar">
        <InputText v-model="query" :placeholder="t('core.marketplace.searchPlaceholder')" class="grow" @keyup.enter="search" />
        <Button size="small" :label="t('core.marketplace.search')" @click="search" />
        <Button size="small" text icon="pi pi-refresh" :loading="busy === 'refresh'" :label="t('core.marketplace.refresh')" @click="refresh" />
      </div>
      <p v-if="!browse.length" class="empty">{{ t("core.marketplace.emptyBrowse") }}</p>
      <div v-for="e in browse" :key="`${e.kind}/${e.id}/${e.version}`" class="card row">
        <div class="info">
          <div class="head">
            <span class="name">{{ e.name || e.id }}</span>
            <span class="kind">{{ e.kind }}</span>
            <span class="ver">v{{ e.version }}</span>
            <span v-if="e.certified" class="badge cert" :title="t('core.marketplace.certified')"><i class="pi pi-verified" /> {{ t("core.marketplace.certified") }}</span>
          </div>
          <p v-if="e.summary" class="summary">{{ e.summary }}</p>
          <div v-if="e.requires?.length" class="meta">
            <span v-for="r in e.requires" :key="r.module" class="badge req">{{ r.module }} {{ r.version }}</span>
          </div>
        </div>
        <!-- Dynamic action: buy (locked paid dep) / install / update / installed -->
        <div class="action">
          <Button
            v-if="state(e).status === 'buy'"
            size="small"
            severity="warn"
            icon="pi pi-shopping-cart"
            :label="t('core.marketplace.buy')"
            @click="buy(e)"
          />
          <Button
            v-else-if="state(e).status === 'install'"
            size="small"
            :label="t('core.marketplace.install')"
            :loading="busy === `${e.kind}/${e.id}`"
            @click="install(e)"
          />
          <template v-else-if="state(e).status === 'update'">
            <Button
              size="small"
              :label="t('core.marketplace.update', { v: e.version })"
              :loading="busy === `${e.kind}/${e.id}`"
              @click="install(e)"
            />
            <Button size="small" text severity="danger" icon="pi pi-times" :title="t('core.marketplace.uninstall')" @click="uninstall(state(e).rec)" />
          </template>
          <template v-else>
            <span class="installed"><i class="pi pi-check-circle" /> {{ t("core.marketplace.installedBadge") }}</span>
            <Button size="small" text severity="danger" icon="pi pi-times" :title="t('core.marketplace.uninstall')" @click="uninstall(state(e).rec)" />
          </template>
        </div>
      </div>
    </section>

    <!-- Installed -->
    <section v-else-if="tab === 'installed'" class="pane">
      <div class="bar">
        <InputText v-model="sideloadPath" :placeholder="t('core.marketplace.sideloadPlaceholder')" class="grow" @keyup.enter="sideload" />
        <Button size="small" text icon="pi pi-upload" :label="t('core.marketplace.sideload')" @click="sideload" />
        <Button size="small" text icon="pi pi-trash" :label="t('core.marketplace.purge')" @click="purge" />
      </div>
      <p v-if="!installed.length" class="empty">{{ t("core.marketplace.emptyInstalled") }}</p>
      <div v-for="r in installed" :key="`${r.kind}/${r.id}`" class="card row">
        <div class="head">
          <span class="name">{{ r.id }}</span>
          <span class="kind">{{ r.kind }}</span>
          <span class="ver">v{{ r.version }}</span>
          <span class="badge src">{{ r.source }}</span>
          <span v-if="!r.explicit" class="badge dep" :title="t('core.marketplace.dependency')">{{ t("core.marketplace.dependency") }}</span>
        </div>
        <div class="actions">
          <Button v-if="updateFor(r)" size="small" :label="t('core.marketplace.update', { v: updateFor(r).version })" @click="install(updateFor(r))" />
          <Button size="small" text severity="danger" icon="pi pi-times" :label="t('core.marketplace.uninstall')" @click="uninstall(r)" />
        </div>
      </div>
    </section>

    <!-- Licenses / entitlements (M6.8) -->
    <section v-else-if="tab === 'licenses'" class="pane">
      <p class="hint">{{ t("core.marketplace.licenses.intro") }}</p>
      <textarea
        v-model="licenseInput"
        class="licensebox"
        rows="3"
        spellcheck="false"
        :placeholder="t('core.marketplace.licenses.placeholder')"
      />
      <div class="bar">
        <Button size="small" :label="t('core.marketplace.licenses.add')" :disabled="!licenseInput.trim()" @click="submitLicense" />
      </div>
      <p v-if="!entitlements.length" class="empty">{{ t("core.marketplace.licenses.none") }}</p>
      <div v-for="(e, i) in entitlements" :key="i" class="card row">
        <div class="info">
          <div class="head">
            <span class="name">{{ e.sku }}</span>
            <span class="kind">{{ e.kind }}/{{ e.id }}</span>
          </div>
          <div class="meta">
            <span class="badge cap">{{ e.model }}</span>
            <span v-if="e.not_after" class="badge req">{{ t("core.marketplace.licenses.until") }} {{ e.not_after }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- Consent -->
    <section v-else class="pane">
      <p class="hint">{{ t("core.marketplace.consentHint") }}</p>
      <p v-if="!consent.length" class="empty">{{ t("core.marketplace.emptyConsent") }}</p>
      <div v-for="m in consent" :key="m.id" class="card">
        <div class="head">
          <span class="name">{{ m.name || m.id }}</span>
          <span class="ver">v{{ m.version }}</span>
          <span v-if="m.pending.length" class="badge pending"><i class="pi pi-exclamation-triangle" /> {{ t("core.marketplace.pending") }}</span>
        </div>
        <div class="caps">
          <button
            v-for="c in m.declared"
            :key="c"
            class="captoggle"
            :class="{ on: m.granted.includes(c), warn: m.pending.includes(c) }"
            @click="toggleCap(m, c)"
          >
            <i :class="m.granted.includes(c) ? 'pi pi-check-circle' : 'pi pi-circle'" /> {{ c }}
          </button>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.market { display: flex; flex-direction: column; height: 100%; }
.tabs { display: flex; gap: 4px; border-bottom: 1px solid var(--p-content-border-color, #333); padding: 0 4px; }
.tab { background: none; border: none; color: var(--p-text-muted-color, #999); padding: 10px 14px; cursor: pointer; border-bottom: 2px solid transparent; }
.tab.active { color: var(--p-text-color, #eee); border-bottom-color: var(--p-primary-color, #7c9); }
.pane { padding: 14px; overflow-y: auto; }
.bar { display: flex; gap: 8px; margin-bottom: 14px; align-items: center; }
.grow { flex: 1; }
.empty, .hint { color: var(--p-text-muted-color, #999); font-size: 0.9em; }
.card { border: 1px solid var(--p-content-border-color, #333); border-radius: 8px; padding: 12px; margin-bottom: 10px; }
.card.row { display: flex; align-items: center; justify-content: space-between; }
.head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.name { font-weight: 600; }
.kind, .ver { color: var(--p-text-muted-color, #999); font-size: 0.85em; }
.summary { margin: 6px 0; color: var(--p-text-muted-color, #bbb); font-size: 0.9em; }
.meta { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 10px; }
.actions { display: flex; gap: 6px; }
.info { flex: 1; min-width: 0; }
.action { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
.installed { color: #7dd3a0; display: inline-flex; align-items: center; gap: 5px; font-size: 0.9em; white-space: nowrap; }
.licensebox { width: 100%; box-sizing: border-box; resize: vertical; font-family: monospace; font-size: 0.8em; padding: 8px; border-radius: 8px; border: 1px solid var(--p-content-border-color, #333); background: var(--p-content-background, #1a1a1a); color: inherit; margin-bottom: 8px; word-break: break-all; }
.badge { font-size: 0.72em; padding: 2px 7px; border-radius: 10px; border: 1px solid transparent; }
.badge.cert { color: #7dd3a0; border-color: #2e6b4a; }
.badge.cap { color: #e0a458; border-color: #6b4a2e; }
.badge.req { color: #8ab4f8; border-color: #2e4a6b; }
.badge.src { color: #999; border-color: #444; }
.badge.dep { color: #b98; border-color: #654; }
.badge.pending { color: #f0b429; border-color: #6b5a1e; }
.caps { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
.captoggle { display: inline-flex; align-items: center; gap: 6px; background: none; border: 1px solid var(--p-content-border-color, #444); color: var(--p-text-muted-color, #999); border-radius: 14px; padding: 4px 12px; cursor: pointer; }
.captoggle.on { color: #7dd3a0; border-color: #2e6b4a; }
.captoggle.warn { border-color: #6b5a1e; }
</style>
