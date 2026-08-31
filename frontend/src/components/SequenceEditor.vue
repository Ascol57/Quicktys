<!-- Reusable sequence editor: the time-grid canvas + the node inspector, with
     multi-selection and a duration-aware client-side test animation. Edits the
     `root` block node in place (the parent owns it). Used by the automation
     editor and by each profile trigger. Emits `test` so the parent can actually
     run the sequence. -->
<script setup>
import { computed, onMounted, onUnmounted, provide, ref } from "vue";
import AutomationCanvas from "./AutomationCanvas.vue";
import Inspector from "./Inspector.vue";
import { findNodeById, applyNode, collectNodes, removeIds, assignIds } from "../nodes.js";

const props = defineProps({
  root: { type: Object, required: true },
  plugins: { type: Array, default: () => [] },
  triggerName: { type: String, default: "When triggered" },
  triggerIcon: { type: String, default: "pi pi-bolt" },
  running: Boolean,
});
const emit = defineEmits(["test"]);

const selection = ref(new Set());
const selectedNode = computed(() =>
  selection.value.size === 1 ? findNodeById(props.root, [...selection.value][0]) : null,
);
function updateSelected(newNode) {
  if (selectedNode.value) applyNode(selectedNode.value, newNode);
}

// --- duration-aware test animation (client-side glow) ----------------------
const testing = ref(new Set());
let testToken = 0;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const clampMs = (ms) => Math.min(Math.max(ms, 0), 30000);
const ACTION_MS = 250;
const waitMs = (n) =>
  n.wait === "delay" ? clampMs(Number(n.ms) || 0) : clampMs(Number(n.timeout_ms) || 800);
const loopCount = (n) => {
  const c = Number(n.count);
  return Number.isFinite(c) ? Math.min(Math.max(c, 1), 20) : 3;
};
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
    default:
      await sleep(120);
  }
  testing.value.delete(id);
}
function animateTest() {
  const token = ++testToken;
  testing.value = new Set();
  playBlock(props.root, token).then(() => {
    if (token === testToken) testing.value = new Set();
  });
}
function onTest() {
  animateTest();
  emit("test");
}
// Expose so a parent can stop/clear the glow after the real run resolves.
defineExpose({
  stopTest: () => {
    testToken++;
    testing.value = new Set();
  },
});

provide("automation-editor", {
  select: (id) => (selection.value = id ? new Set([id]) : new Set()),
  setSelection: (ids) => (selection.value = new Set(ids)),
  isSelected: (id) => selection.value.has(id),
  isTesting: (id) => testing.value.has(id),
});

// --- clipboard / delete ----------------------------------------------------
const clipboard = ref([]);
function del() {
  if (!selection.value.size) return;
  removeIds({ root: props.root }, selection.value);
  selection.value = new Set();
}
function copy() {
  if (selection.value.size) clipboard.value = collectNodes({ root: props.root }, selection.value);
}
function paste() {
  if (!clipboard.value.length) return;
  const nodes = JSON.parse(JSON.stringify(clipboard.value));
  nodes.forEach(assignIds);
  props.root.body.push(...nodes);
}
function onKey(e) {
  const tag = e.target?.tagName;
  const inField = tag === "INPUT" || tag === "TEXTAREA" || e.target?.isContentEditable;
  if (inField) return;
  const ctrl = e.ctrlKey || e.metaKey;
  if (ctrl && e.key.toLowerCase() === "c") copy();
  else if (ctrl && e.key.toLowerCase() === "v") paste();
  else if (e.key === "Delete" || e.key === "Backspace") {
    e.preventDefault();
    del();
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="seq-editor">
    <AutomationCanvas
      :root="root"
      :plugins="plugins"
      :running="running"
      :trigger-name="triggerName"
      :trigger-icon="triggerIcon"
      @test="onTest"
    />
    <Inspector :node="selectedNode" :plugins="plugins" @update:node="updateSelected" />
  </div>
</template>

<style scoped>
.seq-editor {
  display: flex;
  flex: 1;
  min-height: 0;
  min-width: 0;
}
</style>
