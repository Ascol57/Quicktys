<!-- Post-install setup wizard: after installing extension(s), walk the user through
       1. Settings — a form for each installed extension that declares one.
       2. Permissions — validate each installed module's system capabilities.
     Phases that have nothing to show are skipped; if both are empty the wizard
     closes immediately. -->
<script setup>
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import Button from "primevue/button";
import { useToast } from "primevue/usetoast";
import SchemaForm from "./SchemaForm.vue";
import { ipc } from "../ipc.js";

// `records`: the install outcomes, `[{ id, kind, version }, …]`.
const props = defineProps({ records: { type: Array, required: true } });
const emit = defineEmits(["done"]);
const { t } = useI18n();
const toast = useToast();

const visible = ref(true);
const loading = ref(true);
const phaseIdx = ref(0);
const settingsTargets = ref([]); // [{ id, kind, name, schema, model }]
const permTargets = ref([]); // consent_status rows for installed modules

const phases = computed(() => {
  const p = [];
  if (settingsTargets.value.length) p.push("settings");
  if (permTargets.value.length) p.push("permissions");
  return p;
});
const phase = computed(() => phases.value[phaseIdx.value]);
const isLast = computed(() => phaseIdx.value >= phases.value.length - 1);

onMounted(async () => {
  const slugs = new Set(props.records.map((r) => `${r.kind}/${r.id}`));

  // Settings: installed extensions that declare a schema.
  const plugins = (await ipc.listPlugins()) ?? [];
  const modules = (await ipc.listModules()) ?? [];
  const byId = {};
  plugins.forEach((p) => (byId[`plugin/${p.id}`] = { ...p, kind: "plugin" }));
  modules.forEach((m) => (byId[`module/${m.id}`] = { ...m, kind: "module" }));
  const targets = [];
  for (const slug of slugs) {
    const x = byId[slug];
    if (x?.settings) {
      targets.push({ id: x.id, kind: x.kind, name: x.name, schema: x.settings, model: (await ipc.getSettings(x.id)) ?? {} });
    }
  }
  settingsTargets.value = targets;

  // Permissions: installed modules that declare system caps.
  const consent = (await ipc.consentStatus()) ?? [];
  permTargets.value = consent.filter((c) => slugs.has(`module/${c.id}`) && c.declared.length);

  loading.value = false;
  if (!phases.value.length) finish(); // nothing to configure
});

async function saveSettings() {
  for (const tgt of settingsTargets.value) {
    try {
      await ipc.setSettings(tgt.id, tgt.model);
    } catch (e) {
      toast.add({ severity: "error", detail: `${tgt.id}: ${e}`, life: 5000 });
    }
  }
}

async function toggleCap(mod, cap) {
  if (mod.granted.includes(cap)) await ipc.revokeCapability(mod.id, cap);
  else await ipc.grantCapabilities(mod.id, [...mod.granted, cap]);
  permTargets.value = (await ipc.consentStatus()).filter((c) => permTargets.value.some((p) => p.id === c.id));
}

async function next() {
  if (phase.value === "settings") await saveSettings();
  if (isLast.value) finish();
  else phaseIdx.value += 1;
}
function back() {
  if (phaseIdx.value > 0) phaseIdx.value -= 1;
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
    :header="t('core.setup.title')"
    :style="{ width: '34rem', maxWidth: '92vw' }"
    @update:visible="(v) => !v && emit('done')"
  >
    <div v-if="phases.length > 1" class="steps">
      <span v-for="(p, i) in phases" :key="p" class="dot" :class="{ on: i <= phaseIdx }" />
    </div>

    <p v-if="loading" class="muted"><i class="pi pi-spin pi-spinner" /> {{ t("core.setup.loading") }}</p>

    <!-- Phase: settings -->
    <div v-else-if="phase === 'settings'">
      <p class="muted">{{ t("core.setup.settingsIntro") }}</p>
      <div v-for="tgt in settingsTargets" :key="`${tgt.kind}/${tgt.id}`" class="block">
        <h4>{{ tgt.name || tgt.id }}</h4>
        <SchemaForm :schema="tgt.schema" v-model="tgt.model" />
      </div>
    </div>

    <!-- Phase: permissions -->
    <div v-else-if="phase === 'permissions'">
      <p class="muted">{{ t("core.setup.permsIntro") }}</p>
      <div v-for="m in permTargets" :key="m.id" class="block">
        <h4>{{ m.name || m.id }}</h4>
        <div class="caps">
          <button
            v-for="c in m.declared"
            :key="c"
            class="captoggle"
            :class="{ on: m.granted.includes(c) }"
            @click="toggleCap(m, c)"
          >
            <i :class="m.granted.includes(c) ? 'pi pi-check-circle' : 'pi pi-circle'" /> {{ c }}
          </button>
        </div>
      </div>
    </div>

    <template #footer>
      <Button v-if="phaseIdx > 0" text :label="t('core.setup.back')" icon="pi pi-arrow-left" @click="back" />
      <Button text :label="t('core.setup.skip')" @click="finish" />
      <Button :label="isLast ? t('core.setup.finish') : t('core.setup.next')" :disabled="loading" @click="next" />
    </template>
  </Dialog>
</template>

<style scoped>
.steps { display: flex; gap: 6px; justify-content: center; margin-bottom: 14px; }
.dot { width: 8px; height: 8px; border-radius: 50%; background: var(--p-content-border-color, #444); }
.dot.on { background: var(--p-primary-color, #7c9); }
.muted { color: var(--p-text-muted-color, #999); margin-bottom: 10px; }
.block { border: 1px solid var(--p-content-border-color, #333); border-radius: 8px; padding: 12px; margin-bottom: 10px; }
.block h4 { margin: 0 0 8px; }
.caps { display: flex; gap: 8px; flex-wrap: wrap; }
.captoggle { display: inline-flex; align-items: center; gap: 6px; background: none; border: 1px solid var(--p-content-border-color, #444); color: var(--p-text-muted-color, #999); border-radius: 14px; padding: 4px 12px; cursor: pointer; }
.captoggle.on { color: #7dd3a0; border-color: #2e6b4a; }
</style>
