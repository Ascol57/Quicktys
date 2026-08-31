<!-- Recursive editor for one automation node — the block/flow editor. -->
<script setup>
import { computed, ref, watch } from "vue";
import draggable from "vuedraggable";
import Select from "primevue/select";
import InputText from "primevue/inputtext";
import InputNumber from "primevue/inputnumber";
import Button from "primevue/button";
import { useI18n } from "vue-i18n";
import SchemaForm from "./SchemaForm.vue";
import NodePalette from "./NodePalette.vue";
import TemplateInput from "./TemplateInput.vue";
import { buildVariables } from "../variables.js";
import { ipc } from "../ipc.js";
import {
  newNode,
  newTrack,
  newCase,
  nodeFromPick,
  label,
  meta,
  setLoopMode,
  setWaitMode,
  assignIds,
  stripIds,
} from "../nodes.js";

defineOptions({ name: "AutomationNode" });

const props = defineProps({
  node: { type: Object, required: true },
  plugins: { type: Array, default: () => [] },
});
const emit = defineEmits(["update:node", "remove", "duplicate"]);

const collapsed = ref(false);
const patch = (partial) => emit("update:node", { ...props.node, ...partial });

// State variables offered by the `@` autocomplete in every expression field
// (conditions, counts, iterables) — the same catalogue the action config fields
// use, so one syntax and one picker everywhere.
const { t, te } = useI18n();
const variables = computed(() => buildVariables(props.plugins ?? [], t, te));

// --- list helpers (body / branches) ---------------------------------------
const list = (key) => props.node[key] ?? [];
const updateAt = (key, i, v) => {
  const l = [...list(key)];
  l[i] = v;
  patch({ [key]: l });
};
const removeAt = (key, i) => {
  const l = [...list(key)];
  l.splice(i, 1);
  patch({ [key]: l });
};
const duplicateAt = (key, i) => {
  const copy = stripIds(list(key)[i]);
  assignIds(copy);
  const l = [...list(key)];
  l.splice(i + 1, 0, copy);
  patch({ [key]: l });
};
const addChild = (key, pick) => patch({ [key]: [...list(key), nodeFromPick(pick)] });

const proxy = (key) =>
  computed({ get: () => props.node[key] ?? [], set: (v) => patch({ [key]: v }) });
const bodyProxy = proxy("body");
const branchesProxy = proxy("branches");
const tracksProxy = proxy("tracks");
const casesProxy = proxy("cases");

// --- action: schema-driven config -----------------------------------------
const actionSchema = ref(null);
const pluginOptions = () => props.plugins.map((p) => ({ label: p.name, value: p.id }));
const actionOptions = () =>
  props.plugins.find((p) => p.id === props.node.plugin)?.actions ?? [];

async function loadSchema() {
  if (props.node.kind !== "action" || !props.node.plugin || !props.node.action) {
    actionSchema.value = null;
    return;
  }
  actionSchema.value = await ipc.actionSchema(props.node.plugin, props.node.action);
}
watch(() => [props.node.plugin, props.node.action], loadSchema, { immediate: true });

// --- switch case / timeline track edits ------------------------------------
function updateCaseField(i, partial) {
  const cases = [...list("cases")];
  cases[i] = { ...cases[i], ...partial };
  patch({ cases });
}
function equalsText(value) {
  return value === undefined ? "" : JSON.stringify(value);
}
function parseEquals(text) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}
function updateTrackField(i, partial) {
  const tracks = [...list("tracks")];
  tracks[i] = { ...tracks[i], ...partial };
  patch({ tracks });
}

const hasChildren = computed(() => !["break", "continue"].includes(props.node.kind));
</script>

<template>
  <div class="node" :style="{ '--accent': meta(node).color }">
    <div class="node__head">
      <i class="drag pi pi-bars" title="drag to reorder" />
      <i :class="meta(node).icon" class="node__icon" />
      <span class="node__title">{{ label(node) }}</span>
      <span class="spacer" />
      <Button
        v-if="hasChildren"
        :icon="collapsed ? 'pi pi-chevron-down' : 'pi pi-chevron-up'"
        text
        rounded
        size="small"
        severity="secondary"
        @click="collapsed = !collapsed"
      />
      <Button
        icon="pi pi-copy"
        text
        rounded
        size="small"
        severity="secondary"
        @click="emit('duplicate')"
      />
      <Button
        icon="pi pi-times"
        text
        rounded
        size="small"
        severity="secondary"
        @click="emit('remove')"
      />
    </div>

    <div v-show="!collapsed" class="node__body">
      <!-- ACTION -->
      <template v-if="node.kind === 'action'">
        <div class="row">
          <Select
            :modelValue="node.plugin"
            :options="pluginOptions()"
            option-label="label"
            option-value="value"
            placeholder="Plugin"
            @update:modelValue="patch({ plugin: $event, action: '' })"
          />
          <Select
            :modelValue="node.action"
            :options="actionOptions()"
            placeholder="Action"
            @update:modelValue="patch({ action: $event })"
          />
        </div>
        <SchemaForm
          v-if="actionSchema"
          :schema="actionSchema"
          :modelValue="node.config"
          @update:modelValue="patch({ config: $event })"
        />
      </template>

      <!-- LET / SET -->
      <template v-else-if="node.kind === 'let'">
        <div class="row">
          <span><label>Name</label><InputText :modelValue="node.name" @update:modelValue="patch({ name: $event })" /></span>
          <span><label>Value (expr)</label><TemplateInput expression :variables="variables" :modelValue="node.value" @update:modelValue="patch({ value: $event })" /></span>
        </div>
      </template>
      <template v-else-if="node.kind === 'set'">
        <div class="row">
          <span><label>Target</label><InputText :modelValue="node.target" @update:modelValue="patch({ target: $event })" /></span>
          <span><label>Value (expr)</label><TemplateInput expression :variables="variables" :modelValue="node.value" @update:modelValue="patch({ value: $event })" /></span>
        </div>
      </template>

      <!-- RETURN -->
      <template v-else-if="node.kind === 'return'">
        <label>Value (expr, optional)</label>
        <TemplateInput expression :variables="variables" :modelValue="node.value ?? ''" @update:modelValue="patch({ value: $event || undefined })" />
      </template>

      <!-- WAIT-FOR -->
      <template v-else-if="node.kind === 'wait-for'">
        <Select
          :modelValue="node.wait"
          :options="['delay', 'until', 'event']"
          @update:modelValue="emit('update:node', setWaitMode(node, $event))"
        />
        <template v-if="node.wait === 'delay'">
          <label>Duration (ms, expr)</label>
          <TemplateInput expression :variables="variables" :modelValue="node.ms" @update:modelValue="patch({ ms: $event })" />
        </template>
        <template v-else-if="node.wait === 'until'">
          <label>Condition (expr)</label>
          <TemplateInput expression :variables="variables" :modelValue="node.cond" @update:modelValue="patch({ cond: $event })" />
          <label>Timeout ms (optional)</label>
          <InputNumber :modelValue="node.timeout_ms" @update:modelValue="patch({ timeout_ms: $event ?? undefined })" />
        </template>
        <template v-else>
          <label>Trigger id</label>
          <InputText :modelValue="node.trigger" @update:modelValue="patch({ trigger: $event })" />
          <label>Timeout ms (optional)</label>
          <InputNumber :modelValue="node.timeout_ms" @update:modelValue="patch({ timeout_ms: $event ?? undefined })" />
        </template>
      </template>

      <!-- IF -->
      <template v-else-if="node.kind === 'if'">
        <label>Condition (expr)</label>
        <TemplateInput expression :variables="variables" :modelValue="node.cond" @update:modelValue="patch({ cond: $event })" />
        <div class="branch"><span class="branch__label">then</span>
          <AutomationNode :node="node.then" :plugins="plugins" @update:node="patch({ then: $event })" @remove="patch({ then: newNode('block') })" />
        </div>
        <div class="branch"><span class="branch__label">else</span>
          <AutomationNode v-if="node.otherwise" :node="node.otherwise" :plugins="plugins" @update:node="patch({ otherwise: $event })" @remove="patch({ otherwise: undefined })" />
          <Button v-else label="add else" icon="pi pi-plus" text size="small" @click="patch({ otherwise: newNode('block') })" />
        </div>
      </template>

      <!-- SWITCH -->
      <template v-else-if="node.kind === 'switch'">
        <label>Value (expr)</label>
        <TemplateInput expression :variables="variables" :modelValue="node.value" @update:modelValue="patch({ value: $event })" />
        <draggable v-model="casesProxy" item-key="_id" handle=".drag" :animation="150" class="children">
          <template #item="{ element, index }">
            <div class="case">
              <div class="row">
                <span><label>equals (JSON)</label><InputText class="mono" :modelValue="equalsText(element.equals)" @update:modelValue="updateCaseField(index, { equals: parseEquals($event) })" /></span>
                <Button icon="pi pi-times" text rounded size="small" severity="secondary" @click="removeAt('cases', index)" />
              </div>
              <AutomationNode :node="element.body" :plugins="plugins" @update:node="updateCaseField(index, { body: $event })" @remove="updateCaseField(index, { body: newNode('block') })" />
            </div>
          </template>
        </draggable>
        <Button label="add case" icon="pi pi-plus" text size="small" @click="patch({ cases: [...list('cases'), newCase()] })" />
        <div class="branch"><span class="branch__label">default</span>
          <AutomationNode v-if="node.default" :node="node.default" :plugins="plugins" @update:node="patch({ default: $event })" @remove="patch({ default: undefined })" />
          <Button v-else label="add default" icon="pi pi-plus" text size="small" @click="patch({ default: newNode('block') })" />
        </div>
      </template>

      <!-- LOOP -->
      <template v-else-if="node.kind === 'loop'">
        <Select :modelValue="node.loop" :options="['repeat', 'while', 'for-each']" @update:modelValue="emit('update:node', setLoopMode(node, $event))" />
        <div class="row">
          <template v-if="node.loop === 'repeat'">
            <span><label>Count (expr)</label><TemplateInput expression :variables="variables" :modelValue="node.count" @update:modelValue="patch({ count: $event })" /></span>
          </template>
          <template v-else-if="node.loop === 'while'">
            <span><label>Condition (expr)</label><TemplateInput expression :variables="variables" :modelValue="node.cond" @update:modelValue="patch({ cond: $event })" /></span>
          </template>
          <template v-else>
            <span><label>Var</label><InputText :modelValue="node.var" @update:modelValue="patch({ var: $event })" /></span>
            <span><label>List (expr)</label><TemplateInput expression :variables="variables" :modelValue="node.list" @update:modelValue="patch({ list: $event })" /></span>
          </template>
          <span><label>Max</label><InputNumber :modelValue="node.max" @update:modelValue="patch({ max: $event })" /></span>
        </div>
        <div class="branch"><span class="branch__label">body</span>
          <AutomationNode :node="node.body" :plugins="plugins" @update:node="patch({ body: $event })" @remove="patch({ body: newNode('block') })" />
        </div>
      </template>

      <!-- TRY -->
      <template v-else-if="node.kind === 'try'">
        <div class="branch"><span class="branch__label">body</span>
          <AutomationNode :node="node.body" :plugins="plugins" @update:node="patch({ body: $event })" @remove="patch({ body: newNode('block') })" />
        </div>
        <div class="branch"><span class="branch__label">catch</span>
          <AutomationNode v-if="node.catch" :node="node.catch" :plugins="plugins" @update:node="patch({ catch: $event })" @remove="patch({ catch: undefined })" />
          <Button v-else label="add catch" icon="pi pi-plus" text size="small" @click="patch({ catch: newNode('block') })" />
        </div>
        <div class="row">
          <span><label>Retry max</label><InputNumber :modelValue="node.retry?.max ?? 0" @update:modelValue="patch({ retry: $event ? { max: $event, delay_ms: node.retry?.delay_ms ?? 0 } : undefined })" /></span>
          <span v-if="node.retry"><label>Retry delay ms</label><InputNumber :modelValue="node.retry.delay_ms" @update:modelValue="patch({ retry: { ...node.retry, delay_ms: $event } })" /></span>
        </div>
      </template>

      <!-- PARALLEL -->
      <template v-else-if="node.kind === 'parallel'">
        <Select :modelValue="node.join" :options="['all', 'any', 'race']" @update:modelValue="patch({ join: $event })" />
        <draggable v-model="branchesProxy" item-key="_id" handle=".drag" :animation="150" class="children">
          <template #item="{ element, index }">
            <AutomationNode :node="element" :plugins="plugins" @update:node="updateAt('branches', index, $event)" @remove="removeAt('branches', index)" @duplicate="duplicateAt('branches', index)" />
          </template>
        </draggable>
        <NodePalette :plugins="plugins" @pick="addChild('branches', $event)" />
      </template>

      <!-- TIMELINE -->
      <template v-else-if="node.kind === 'timeline'">
        <draggable v-model="tracksProxy" item-key="_id" handle=".drag" :animation="150" class="children">
          <template #item="{ element, index }">
            <div class="track">
              <div class="row">
                <i class="drag pi pi-bars" />
                <span><label>offset ms</label><InputNumber :modelValue="element.offset_ms" @update:modelValue="updateTrackField(index, { offset_ms: $event })" /></span>
                <Button icon="pi pi-times" text rounded size="small" severity="secondary" @click="removeAt('tracks', index)" />
              </div>
              <AutomationNode :node="element.body" :plugins="plugins" @update:node="updateTrackField(index, { body: $event })" @remove="updateTrackField(index, { body: newNode('block') })" />
            </div>
          </template>
        </draggable>
        <Button label="add track" icon="pi pi-plus" text size="small" @click="patch({ tracks: [...list('tracks'), newTrack()] })" />
      </template>

      <!-- BLOCK -->
      <template v-else-if="node.kind === 'block'">
        <draggable v-model="bodyProxy" item-key="_id" handle=".drag" :animation="150" class="children">
          <template #item="{ element, index }">
            <AutomationNode :node="element" :plugins="plugins" @update:node="updateAt('body', index, $event)" @remove="removeAt('body', index)" @duplicate="duplicateAt('body', index)" />
          </template>
        </draggable>
        <NodePalette :plugins="plugins" @pick="addChild('body', $event)" />
      </template>
    </div>
  </div>
</template>

<style scoped>
.node {
  border: 1px solid #2f2f2f;
  border-left: 3px solid var(--accent, #888);
  border-radius: 6px;
  margin: 0.3rem 0;
  background: #232323;
}
.node__head {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.5rem;
}
.node__icon {
  color: var(--accent);
}
.node__title {
  font-weight: 600;
  font-size: 0.85rem;
}
.spacer {
  flex: 1;
}
.drag {
  cursor: grab;
  opacity: 0.4;
}
.drag:hover {
  opacity: 0.8;
}
.node__body {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  padding: 0 0.5rem 0.5rem 0.5rem;
}
.row {
  display: flex;
  gap: 0.5rem;
  align-items: flex-end;
}
.row > span {
  display: flex;
  flex-direction: column;
}
label {
  font-size: 0.72rem;
  opacity: 0.7;
}
.mono :deep(input),
.mono {
  font-family: ui-monospace, monospace;
}
.branch {
  margin-left: 0.6rem;
  padding-left: 0.5rem;
  border-left: 1px dashed #3a3a3a;
}
.branch__label {
  font-size: 0.7rem;
  opacity: 0.55;
  font-style: italic;
}
.case,
.track {
  border: 1px solid #2a2a2a;
  border-radius: 5px;
  padding: 0.3rem 0.4rem;
  margin: 0.25rem 0;
}
.children {
  display: flex;
  flex-direction: column;
}
</style>
