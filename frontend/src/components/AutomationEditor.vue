<!-- Automation editor: toolbar + grid canvas + inspector, with undo/redo,
     multi-selection, clipboard and a test-run preview. -->
<script setup>
import { computed, onMounted, onUnmounted, provide, ref, watch } from "vue";
import Select from "primevue/select";
import InputText from "primevue/inputtext";
import Button from "primevue/button";
import AutomationCanvas from "./AutomationCanvas.vue";
import Inspector from "./Inspector.vue";
import { ipc } from "../ipc.js";
import {
  newNode,
  assignIds,
  stripIds,
  findNodeById,
  applyNode,
  collectNodes,
  removeIds,
} from "../nodes.js";

defineProps({ plugins: { type: Array, default: () => [] } });

const automations = ref([]);
const selectedAutomationId = ref(null);
const current = ref(null);
const selection = ref(new Set()); // selected node ids (multi)
const result = ref(null);
const running = ref(false);

const selectedNode = computed(() => {
  if (!current.value || selection.value.size !== 1) return null;
  return findNodeById(current.value.root, [...selection.value][0]);
});

// --- test-run preview: light up nodes for their real duration --------------
// A set of currently-"running" node ids (several at once for parallel/timeline).
const testing = ref(new Set());
let testToken = 0;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const clampMs = (ms) => Math.min(Math.max(ms, 0), 30000);

const ACTION_MS = 250; // actions are ~instant (quick network calls)
function waitMs(node) {
  if (node.wait === "delay") return clampMs(Number(node.ms) || 0);
  return clampMs(Number(node.timeout_ms) || 800);
}
function loopCount(node) {
  const n = Number(node.count);
  return Number.isFinite(n) ? Math.min(Math.max(n, 1), 20) : 3;
}

async function playBlock(block, token) {
  for (const child of block?.body ?? []) {
    if (token !== testToken) return;
    await playNode(child, token);
  }
}
async function playBranch(branch, token) {
  if (branch?.kind === "block") await playBlock(branch, token);
  else if (branch) await playNode(branch, token);
}
async function playNode(node, token) {
  if (token !== testToken || !node) return;
  const id = node._id;
  testing.value.add(id);
  switch (node.kind) {
    case "wait-for":
      await sleep(waitMs(node));
      break;
    case "parallel":
      await Promise.all((node.branches ?? []).map((b) => playBranch(b, token)));
      break;
    case "timeline":
      await Promise.all(
        (node.tracks ?? []).map(async (t) => {
          await sleep(clampMs(Number(t.offset_ms) || 0));
          await playBranch(t.body, token);
        }),
      );
      break;
    case "loop": {
      const n = loopCount(node);
      for (let i = 0; i < n && token === testToken; i++) await playBranch(node.body, token);
      break;
    }
    case "if":
      await playBranch(node.then, token);
      break;
    case "switch":
      await playBranch(node.default, token);
      break;
    case "try":
      await playBranch(node.body, token);
      break;
    case "block":
      testing.value.delete(id);
      await playBlock(node, token);
      return;
    case "action":
      await sleep(ACTION_MS);
      break;
    default: // let / set / return / break / continue
      await sleep(120);
  }
  testing.value.delete(id);
}
async function animateTest() {
  const token = ++testToken;
  testing.value = new Set();
  if (current.value) await playBlock(current.value.root, token);
  if (token === testToken) testing.value = new Set();
}

// --- selection API for the recursive canvas --------------------------------
provide("automation-editor", {
  select: (id) => (selection.value = id ? new Set([id]) : new Set()),
  setSelection: (ids) => (selection.value = new Set(ids)),
  isSelected: (id) => selection.value.has(id),
  isTesting: (id) => testing.value.has(id),
});

// --- history (undo/redo) ---------------------------------------------------
const history = ref([]);
const hIndex = ref(-1);
let restoring = false;
let recordTimer = null;

function record() {
  if (restoring || !current.value) return;
  const snap = JSON.stringify(current.value);
  if (history.value[hIndex.value] === snap) return;
  history.value = history.value.slice(0, hIndex.value + 1);
  history.value.push(snap);
  hIndex.value = history.value.length - 1;
}
function resetHistory() {
  history.value = current.value ? [JSON.stringify(current.value)] : [];
  hIndex.value = history.value.length - 1;
}
watch(
  current,
  () => {
    clearTimeout(recordTimer);
    recordTimer = setTimeout(record, 350);
  },
  { deep: true },
);
function restore() {
  restoring = true;
  current.value = JSON.parse(history.value[hIndex.value]);
  setTimeout(() => (restoring = false), 0);
}
function undo() {
  if (hIndex.value > 0) {
    hIndex.value--;
    restore();
  }
}
function redo() {
  if (hIndex.value < history.value.length - 1) {
    hIndex.value++;
    restore();
  }
}

// --- clipboard -------------------------------------------------------------
const clipboard = ref([]);
function copy() {
  if (!current.value || !selection.value.size) return;
  clipboard.value = collectNodes(current.value, selection.value);
}
function del() {
  if (!current.value || !selection.value.size) return;
  removeIds(current.value, selection.value);
  selection.value = new Set();
}
function cut() {
  copy();
  del();
}
function paste() {
  if (!current.value || !clipboard.value.length) return;
  const nodes = JSON.parse(JSON.stringify(clipboard.value));
  nodes.forEach(assignIds);
  current.value.root.body.push(...nodes);
}

// --- resource lifecycle ----------------------------------------------------
async function refresh() {
  automations.value = await ipc.listResources("automation");
  if (!current.value && automations.value.length) openAutomation(automations.value[0].id);
}
// In the desktop shell the host emits real per-node telemetry: light the card
// that is actually executing (path [i] → the i-th top-level node). The saved
// automation strips client ids but preserves order, so [i] maps back cleanly.
let unlistenProgress = null;
function onHostEvent(e) {
  if (e?.type !== "nodeProgress" || !current.value || e.automation !== current.value.id) return;
  const node = current.value.root.body[e.path?.[0]];
  if (!node) return;
  if (e.running) testing.value.add(node._id);
  else testing.value.delete(node._id);
}
onMounted(async () => {
  refresh();
  window.addEventListener("keydown", onKey);
  unlistenProgress = await ipc.onEvent(onHostEvent);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  if (unlistenProgress) unlistenProgress();
});

function normalizeRoot(root) {
  return root && root.kind === "block" ? root : newNode("block");
}
function openAutomation(id) {
  selectedAutomationId.value = id;
  const found = automations.value.find((a) => a.id === id);
  const clone = found ? JSON.parse(JSON.stringify(found)) : null;
  if (clone) {
    clone.root = normalizeRoot(clone.root);
    assignIds(clone.root);
  }
  current.value = clone;
  selection.value = new Set(clone?.root.body[0] ? [clone.root.body[0]._id] : []);
  result.value = null;
  resetHistory();
}
function createNew() {
  current.value = { id: `automation-${automations.value.length + 1}`, root: newNode("block") };
  selectedAutomationId.value = null;
  selection.value = new Set();
  result.value = null;
  resetHistory();
}
function updateSelected(newNode) {
  if (selectedNode.value) applyNode(selectedNode.value, newNode);
}

async function save() {
  await ipc.upsertResource("automation", stripIds(current.value));
  await refresh();
  selectedAutomationId.value = current.value.id;
}
async function run() {
  running.value = true;
  // Native: real host telemetry (onHostEvent) drives the glow. Browser: no host,
  // so fall back to the client-side duration estimate.
  if (!ipc.isNative) animateTest();
  try {
    await save();
    result.value = await ipc.runAutomation(current.value.id, {});
  } catch (e) {
    result.value = { error: String(e) };
  } finally {
    running.value = false;
    testToken++; // stop any client animation
    testing.value = new Set(); // clear leftover highlights
  }
}

function onKey(e) {
  const tag = e.target?.tagName;
  const inField = tag === "INPUT" || tag === "TEXTAREA" || e.target?.isContentEditable;
  const ctrl = e.ctrlKey || e.metaKey;
  if (ctrl && e.key.toLowerCase() === "z") {
    if (inField) return;
    e.preventDefault();
    e.shiftKey ? redo() : undo();
  } else if (ctrl && e.key.toLowerCase() === "y") {
    if (inField) return;
    e.preventDefault();
    redo();
  } else if (ctrl && e.key.toLowerCase() === "c" && !inField) {
    copy();
  } else if (ctrl && e.key.toLowerCase() === "x" && !inField) {
    cut();
  } else if (ctrl && e.key.toLowerCase() === "v" && !inField) {
    paste();
  } else if ((e.key === "Delete" || e.key === "Backspace") && !inField) {
    e.preventDefault();
    del();
  }
}
</script>

<template>
  <div class="editor">
    <div class="toolbar">
      <Select
        :modelValue="selectedAutomationId"
        :options="automations"
        option-label="id"
        option-value="id"
        placeholder="Select automation"
        class="pick"
        @update:modelValue="openAutomation"
      />
      <Button icon="pi pi-plus" label="New" text @click="createNew" />
      <Button icon="pi pi-undo" text severity="secondary" :disabled="hIndex <= 0" @click="undo" />
      <Button
        icon="pi pi-refresh"
        text
        severity="secondary"
        :disabled="hIndex >= history.length - 1"
        @click="redo"
      />
      <span class="spacer" />
      <template v-if="current">
        <InputText :modelValue="current.id" class="id" @update:modelValue="current.id = $event" />
        <Button icon="pi pi-save" label="Save" severity="secondary" @click="save" />
        <Button icon="pi pi-play" label="Run" :loading="running" @click="run" />
      </template>
    </div>

    <div v-if="current" class="workspace">
      <AutomationCanvas :root="current.root" :plugins="plugins" :running="running" @test="run" />
      <Inspector :node="selectedNode" :plugins="plugins" @update:node="updateSelected" />
    </div>
    <p v-else class="muted empty">No automation. Create one to get started.</p>

    <pre v-if="result !== null" class="result">{{ result }}</pre>
  </div>
</template>

<style scoped>
.editor {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.6rem 1rem;
  border-bottom: 1px solid #2a2a2a;
}
.pick {
  min-width: 13rem;
}
.id {
  width: 13rem;
}
.spacer {
  flex: 1;
}
.workspace {
  display: flex;
  flex: 1;
  min-height: 0;
}
.empty {
  padding: 2rem;
  opacity: 0.55;
}
.result {
  border-top: 1px solid #2a2a2a;
  margin: 0;
  padding: 0.6rem 1rem;
  background: #111;
  font-size: 0.8rem;
}
</style>
