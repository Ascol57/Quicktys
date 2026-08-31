<!-- First-run onboarding, in two phases:
       1. Welcome — a short intro.
       2. Plugins — pick from the marketplace catalogue and install (each plugin
          pulls its required modules automatically, M6.6).
     Shown once (App.vue gates it on a localStorage flag). `pear` is offered but
     not pre-selected. -->
<script setup>
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import Button from "primevue/button";
import { useToast } from "primevue/usetoast";
import ExtensionSetup from "./ExtensionSetup.vue";
import { ipc } from "../ipc.js";

const emit = defineEmits(["done"]);
const { t } = useI18n();
const toast = useToast();

// Plugins not offered in onboarding (installable later from the Marketplace).
const HIDDEN = new Set(["discord", "pear", "input"]);

const visible = ref(true);
const phase = ref(1);
const loading = ref(true);
const installing = ref(false);
const items = ref([]);

onMounted(load);

async function load() {
  loading.value = true;
  // Refresh the registry (ignore failure — e.g. none configured), then list plugins.
  try {
    await ipc.marketplaceRefresh();
  } catch {
    /* no registry / offline — items stays empty, we show the hint in phase 2 */
  }
  const plugins = (await ipc.marketplaceSearch("", "plugin")) ?? [];
  items.value = plugins
    .filter((p) => !HIDDEN.has(p.id))
    .map((p) => ({ ...p, selected: true }));
  loading.value = false;
}

const setupRecords = ref(null);

async function installSelected() {
  const chosen = items.value.filter((i) => i.selected);
  if (!chosen.length) return finish();
  installing.value = true;
  let ok = 0;
  const records = [];
  for (const p of chosen) {
    try {
      const recs = (await ipc.installFromRegistry(p.kind, p.id)) ?? [];
      records.push(...recs);
      ok += 1;
    } catch (e) {
      toast.add({ severity: "error", detail: `${p.id}: ${e}`, life: 5000 });
    }
  }
  installing.value = false;
  if (ok) toast.add({ severity: "success", detail: t("core.onboarding.done", { n: ok }), life: 3000 });
  // Hand off to the post-install setup wizard (settings + permissions), then done.
  if (records.length) {
    visible.value = false;
    setupRecords.value = records;
  } else {
    finish();
  }
}

function finish() {
  visible.value = false;
  emit("done");
}
</script>

<template>
  <Dialog
    v-model:visible="visible"
    modal
    :closable="!installing"
    :header="t(phase === 1 ? 'core.onboarding.title' : 'core.onboarding.pickTitle')"
    :style="{ width: '34rem', maxWidth: '92vw' }"
    @update:visible="(v) => !v && emit('done')"
  >
    <div class="steps">
      <span class="dot" :class="{ on: phase >= 1 }" />
      <span class="dot" :class="{ on: phase >= 2 }" />
    </div>

    <!-- Phase 1: welcome -->
    <div v-if="phase === 1">
      <p class="lead">{{ t("core.onboarding.welcome") }}</p>
      <p class="muted">{{ t("core.onboarding.intro") }}</p>
    </div>

    <!-- Phase 2: pick plugins -->
    <div v-else>
      <p v-if="loading" class="muted"><i class="pi pi-spin pi-spinner" /> {{ t("core.onboarding.loading") }}</p>
      <p v-else-if="!items.length" class="muted">{{ t("core.onboarding.empty") }}</p>
      <ul v-else class="list">
        <li v-for="p in items" :key="`${p.kind}/${p.id}`" class="item" :class="{ on: p.selected }" @click="p.selected = !p.selected">
          <i :class="p.selected ? 'pi pi-check-square' : 'pi pi-stop'" />
          <div class="body">
            <div class="name">
              {{ p.name || p.id }}
              <span v-if="p.certified" class="cert"><i class="pi pi-verified" /></span>
            </div>
            <div v-if="p.summary" class="summary">{{ p.summary }}</div>
            <div v-if="p.requires?.length" class="deps">
              {{ t("core.onboarding.pulls") }} {{ p.requires.map((r) => r.module).join(", ") }}
            </div>
          </div>
        </li>
      </ul>
    </div>

    <template #footer>
      <template v-if="phase === 1">
        <Button text :label="t('core.onboarding.skip')" @click="finish" />
        <Button :label="t('core.onboarding.next')" icon="pi pi-arrow-right" icon-pos="right" @click="phase = 2" />
      </template>
      <template v-else>
        <Button text :label="t('core.onboarding.back')" icon="pi pi-arrow-left" :disabled="installing" @click="phase = 1" />
        <Button
          :label="installing ? t('core.onboarding.installing') : t('core.onboarding.install')"
          :loading="installing"
          :disabled="loading || !items.some((i) => i.selected)"
          @click="installSelected"
        />
      </template>
    </template>
  </Dialog>

  <!-- After installing, hand off to the settings + permissions setup wizard. -->
  <ExtensionSetup v-if="setupRecords" :records="setupRecords" @done="finish" />
</template>

<style scoped>
.steps { display: flex; gap: 6px; justify-content: center; margin-bottom: 14px; }
.dot { width: 8px; height: 8px; border-radius: 50%; background: var(--p-content-border-color, #444); }
.dot.on { background: var(--p-primary-color, #7c9); }
.lead { margin: 0 0 10px; font-size: 1.05em; }
.muted { color: var(--p-text-muted-color, #999); }
.list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; max-height: 46vh; overflow-y: auto; }
.item { display: flex; gap: 10px; align-items: flex-start; padding: 10px; border: 1px solid var(--p-content-border-color, #333); border-radius: 8px; cursor: pointer; }
.item.on { border-color: var(--p-primary-color, #7c9); }
.item > i { margin-top: 2px; color: var(--p-primary-color, #7c9); }
.name { font-weight: 600; display: flex; align-items: center; gap: 6px; }
.cert { color: #7dd3a0; }
.summary { color: var(--p-text-muted-color, #bbb); font-size: 0.9em; margin-top: 2px; }
.deps { color: var(--p-text-muted-color, #888); font-size: 0.8em; margin-top: 3px; }
</style>
