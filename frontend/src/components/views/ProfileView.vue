<!-- One profile, CastMate-style: a Triggers strip (Add Trigger), the selected
     trigger's config + its inline sequence edited directly on the canvas, and an
     Activation condition. Fully functional over the `profile` resource. -->
<script setup>
import { computed, inject, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import InputText from "primevue/inputtext";
import Select from "primevue/select";
import Button from "primevue/button";
import SequenceEditor from "../SequenceEditor.vue";
import TemplateInput from "../TemplateInput.vue";
import { buildVariables } from "../../variables.js";
import { useProfiles } from "../../stores/profiles.js";
import { ipc } from "../../ipc.js";
import { assignIds, stripIds, newNode } from "../../nodes.js";

const props = defineProps({ profileId: { type: String, required: true } });

const { t, te } = useI18n();
const plugins = inject("plugins", ref([]));
// State variables for the filter field's `@` autocomplete — same catalogue as
// the action config fields, so `@obs.scene` is offered here too.
const variables = computed(() => buildVariables(plugins.value, t, te));
const store = useProfiles();

const draft = ref(null);
const selectedIndex = ref(0);
const running = ref(false);
const savedFlash = ref(false);

// Presentation for a trigger id ("plugin.trigger"): plugin icon/colour + the
// translated trigger name — mirroring how actions are shown.
function triggerMeta(id) {
  // Built-in: dashboard button widgets fire `button.clicked` (no plugin).
  if (id === "button.clicked") {
    return {
      name: t("core.buttons.clicked"),
      icon: "pi pi-bolt",
      color: "#7c5cff",
      pluginName: t("core.buttons.group"),
    };
  }
  const dot = (id || "").indexOf(".");
  const pid = dot >= 0 ? id.slice(0, dot) : "";
  const trig = dot >= 0 ? id.slice(dot + 1) : id;
  const plugin = plugins.value.find((p) => p.id === pid);
  const key = `plugin.${pid}.triggers.${trig}.name`;
  return {
    name: te(key) ? t(key) : trig || t("core.profiles.untitledTrigger"),
    icon: plugin?.icon || "pi pi-bolt",
    color: plugin?.color || "#9a5fd0",
    pluginName: plugin?.name || pid,
  };
}
// Triggers grouped by plugin, for the picker (like the action palette). A
// built-in "Buttons" group exposes the dashboard button-widget event.
const triggerGroups = computed(() => [
  ...plugins.value
    .filter((p) => (p.triggers ?? []).length)
    .map((p) => ({
      label: p.name,
      items: (p.triggers ?? []).map((tr) => {
        const id = `${p.id}.${tr}`;
        return { id, name: triggerMeta(id).name, icon: p.icon, color: p.color };
      }),
    })),
  {
    label: t("core.buttons.group"),
    items: [{ id: "button.clicked", name: t("core.buttons.clicked"), icon: "pi pi-bolt", color: "#7c5cff" }],
  },
]);
const firstTriggerId = () => triggerGroups.value[0]?.items[0]?.id ?? "";

// Load a working copy; give every trigger sequence a block root + editor ids.
function loadDraft() {
  const found = store.byId(props.profileId);
  const clone = found
    ? JSON.parse(JSON.stringify(found))
    : { id: props.profileId, name: props.profileId, triggers: [] };
  clone.triggers = (clone.triggers ?? []).map((tr) => {
    const seq = tr.sequence && tr.sequence.kind === "block" ? tr.sequence : newNode("block");
    assignIds(seq);
    return { ...tr, sequence: seq };
  });
  draft.value = clone;
  selectedIndex.value = clone.triggers.length ? 0 : -1;
}
watch(() => [props.profileId, store.list.length], loadDraft, { immediate: true });

const selected = computed(() =>
  selectedIndex.value >= 0 ? draft.value?.triggers[selectedIndex.value] : null,
);
const triggerLabel = (tr) => triggerMeta(tr.trigger).name;

function addTrigger() {
  draft.value.triggers.push({ trigger: firstTriggerId(), filter: "", sequence: newNode("block") });
  selectedIndex.value = draft.value.triggers.length - 1;
}
function removeTrigger(i) {
  draft.value.triggers.splice(i, 1);
  if (selectedIndex.value >= draft.value.triggers.length) selectedIndex.value = draft.value.triggers.length - 1;
}

async function save() {
  const d = draft.value;
  const payload = {
    id: d.id,
    name: d.name || d.id,
    triggers: d.triggers.map((tr) => ({
      trigger: tr.trigger || "",
      sequence: stripIds(tr.sequence),
      ...(tr.filter && tr.filter.trim() ? { filter: tr.filter } : {}),
    })),
    ...(d.activation && d.activation.trim() ? { activation: d.activation } : {}),
  };
  await store.save(payload);
  savedFlash.value = true;
  setTimeout(() => (savedFlash.value = false), 1500);
}
async function remove() {
  await store.remove(draft.value.id);
}
async function testSelected() {
  if (!selected.value) return;
  running.value = true;
  try {
    await ipc.runSequence(stripIds(selected.value.sequence));
  } finally {
    // Let the client-side glow finish on its own.
    setTimeout(() => (running.value = false), 300);
  }
}
</script>

<template>
  <div v-if="draft" class="profile">
    <!-- header -->
    <header class="head">
      <InputText v-model="draft.name" :placeholder="t('core.profiles.namePlaceholder')" class="name" />
      <div class="spacer" />
      <span v-if="savedFlash" class="saved"><i class="pi pi-check" /> {{ t("core.profiles.saved") }}</span>
      <Button size="small" severity="danger" text @click="remove">
        <i class="pi pi-trash" /> {{ t("core.actions.delete") }}
      </Button>
      <Button size="small" @click="save"><i class="pi pi-save" /> {{ t("core.actions.save") }}</Button>
    </header>

    <!-- triggers strip -->
    <div class="triggers">
      <span class="section-label">{{ t("core.profiles.triggers") }}</span>
      <button
        v-for="(tr, i) in draft.triggers"
        :key="i"
        class="chip"
        :class="{ active: i === selectedIndex }"
        :style="{ '--c': triggerMeta(tr.trigger).color }"
        @click="selectedIndex = i"
      >
        <i class="chip__icon" :class="triggerMeta(tr.trigger).icon" />
        <span class="chip__text">
          <span class="chip__plugin">{{ triggerMeta(tr.trigger).pluginName }}</span>
          <span class="chip__name">{{ triggerLabel(tr) }}</span>
        </span>
        <i class="chip__x pi pi-times" @click.stop="removeTrigger(i)" />
      </button>
      <Button size="small" text @click="addTrigger">
        <i class="pi pi-plus" /> {{ t("core.profiles.addTrigger") }}
      </Button>
    </div>

    <!-- selected trigger: config + inline sequence -->
    <template v-if="selected">
      <div class="trigger-config">
        <div class="field">
          <label>{{ t("core.profiles.trigger") }}</label>
          <Select
            v-model="selected.trigger"
            :options="triggerGroups"
            optionLabel="name"
            optionValue="id"
            optionGroupLabel="label"
            optionGroupChildren="items"
            :placeholder="t('core.profiles.triggerPlaceholder')"
            class="trigger-select"
          >
            <template #value="{ value }">
              <div v-if="value" class="opt">
                <i :class="triggerMeta(value).icon" :style="{ color: triggerMeta(value).color }" />
                <span>{{ triggerMeta(value).name }}</span>
              </div>
              <span v-else class="muted">{{ t("core.profiles.triggerPlaceholder") }}</span>
            </template>
            <template #option="{ option }">
              <div class="opt">
                <i :class="option.icon" :style="{ color: option.color }" />
                <span>{{ option.name }}</span>
              </div>
            </template>
          </Select>
        </div>
        <div class="field grow">
          <label>{{ t("core.profiles.filter") }}</label>
          <TemplateInput
            expression
            :variables="variables"
            :modelValue="selected.filter ?? ''"
            :placeholder="t('core.profiles.filterPlaceholder')"
            @update:modelValue="selected.filter = $event"
          />
        </div>
      </div>

      <SequenceEditor
        :key="selectedIndex"
        class="canvas"
        :root="selected.sequence"
        :plugins="plugins"
        :running="running"
        :trigger-name="triggerLabel(selected)"
        :trigger-icon="triggerMeta(selected.trigger).icon"
        @test="testSelected"
      />
    </template>
    <p v-else class="muted pad">{{ t("core.profiles.noTriggers") }}</p>

    <!-- activation -->
    <footer class="activation">
      <span class="section-label">{{ t("core.profiles.activation") }}</span>
      <InputText
        v-model="draft.activation"
        :placeholder="t('core.profiles.alwaysActive')"
        class="mono grow-input"
      />
      <small class="muted">{{ t("core.profiles.activationHint") }}</small>
    </footer>
  </div>
</template>

<style scoped>
.profile {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.7rem 1rem;
  border-bottom: 1px solid #2a2a2a;
}
.name {
  font-size: 1.05rem;
  font-weight: 600;
  min-width: 220px;
}
.spacer {
  flex: 1;
}
.saved {
  color: #7ec27e;
  font-size: 0.8rem;
  display: flex;
  align-items: center;
  gap: 0.3rem;
}
.triggers {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  border-bottom: 1px solid #232323;
  flex-wrap: wrap;
}
.section-label {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  opacity: 0.5;
  margin-right: 0.25rem;
}
.chip {
  --c: #9a5fd0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.3rem 0.55rem;
  background: #1f1f1f;
  border: 1px solid #313131;
  border-left: 3px solid var(--c);
  border-radius: 7px;
  color: #ddd;
  cursor: pointer;
  text-align: left;
}
.chip.active {
  border-color: var(--c);
  border-left-color: var(--c);
  background: color-mix(in srgb, var(--c) 20%, #1f1f1f);
  color: #fff;
}
.chip__icon {
  font-size: 1rem;
  color: var(--c);
}
.chip__text {
  display: flex;
  flex-direction: column;
  line-height: 1.1;
}
.chip__plugin {
  font-size: 0.62rem;
  opacity: 0.7;
}
.chip__name {
  font-size: 0.84rem;
  font-weight: 600;
}
.chip__x {
  font-size: 0.7rem;
  opacity: 0.4;
  margin-left: 0.2rem;
}
.chip__x:hover {
  opacity: 1;
}
.opt {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.trigger-select {
  min-width: 15rem;
}
.trigger-config {
  display: flex;
  gap: 1rem;
  padding: 0.6rem 1rem;
  border-bottom: 1px solid #232323;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.field label {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  opacity: 0.5;
}
.field.grow {
  flex: 1;
}
.mono {
  font-family: ui-monospace, monospace;
}
.canvas {
  flex: 1;
  min-height: 0;
}
.activation {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.6rem 1rem;
  border-top: 1px solid #2a2a2a;
  background: #191919;
}
.grow-input {
  flex: 1;
  max-width: 420px;
}
.muted {
  opacity: 0.55;
}
.pad {
  padding: 1.5rem;
}
</style>
