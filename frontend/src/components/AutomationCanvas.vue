<!-- Pannable/zoomable time-grid canvas: time ruler, and the main sequence
     (trigger + recursive flow). Rubber-band multi-select on empty drag; pan on
     middle-mouse; Ctrl+wheel to zoom the time axis. Clean-room design informed
     by CastMate's automation edit area. -->
<script setup>
import { computed, inject, provide, ref } from "vue";
import TriggerCard from "./TriggerCard.vue";
import SequenceFlow from "./SequenceFlow.vue";
import NodePalette from "./NodePalette.vue";
import { nodeFromPick } from "../nodes.js";

const props = defineProps({
  root: { type: Object, required: true },
  plugins: { type: Array, default: () => [] },
  running: Boolean,
  triggerName: { type: String, default: "When triggered" },
  triggerIcon: { type: String, default: "pi pi-bolt" },
});
const emit = defineEmits(["test"]);

const editor = inject("automation-editor");

const canvasEl = ref(null);
const pan = ref({ x: 40, y: 56 });

// Zoom: pixels per second (grid cell). Time offsets/widths scale with it.
const scale = ref(120);
const pxPerMs = computed(() => scale.value / 1000);
provide("time-px-per-ms", pxPerMs);

function onWheel(e) {
  if (!e.ctrlKey) return;
  e.preventDefault();
  const f = e.deltaY < 0 ? 1.1 : 0.9;
  scale.value = Math.min(480, Math.max(40, scale.value * f));
}

// --- time ruler ------------------------------------------------------------
const ticks = computed(() => {
  const cell = scale.value;
  const kMin = Math.floor(-pan.value.x / cell) - 1;
  return Array.from({ length: 80 }, (_, i) => kMin + i).map((k) => ({
    k,
    left: pan.value.x + k * cell,
  }));
});

// --- pan (middle mouse) / rubber-band select (left drag on empty) ----------
const panning = ref(false);
const selecting = ref(false);
const selRect = ref({ l: 0, t: 0, w: 0, h: 0 });
let panOrigin = null;
let selFrom = null;
let baseRect = null;

function onDown(e) {
  menu.value.open = false;
  if (e.button === 1) {
    panning.value = true;
    panOrigin = { x: e.clientX - pan.value.x, y: e.clientY - pan.value.y };
    e.preventDefault();
    return;
  }
  if (e.button !== 0 || e.target !== e.currentTarget) return;
  selecting.value = true;
  baseRect = canvasEl.value.getBoundingClientRect();
  selFrom = { x: e.clientX, y: e.clientY };
  selRect.value = { l: e.clientX - baseRect.left, t: e.clientY - baseRect.top, w: 0, h: 0 };
}
function onMove(e) {
  if (panning.value) {
    pan.value = { x: e.clientX - panOrigin.x, y: e.clientY - panOrigin.y };
  } else if (selecting.value) {
    selRect.value = {
      l: Math.min(selFrom.x, e.clientX) - baseRect.left,
      t: Math.min(selFrom.y, e.clientY) - baseRect.top,
      w: Math.abs(e.clientX - selFrom.x),
      h: Math.abs(e.clientY - selFrom.y),
    };
  }
}
function onUp(e) {
  if (selecting.value) finishSelect(e);
  panning.value = false;
  selecting.value = false;
}
function finishSelect(e) {
  const sel = {
    l: Math.min(selFrom.x, e.clientX),
    t: Math.min(selFrom.y, e.clientY),
    r: Math.max(selFrom.x, e.clientX),
    b: Math.max(selFrom.y, e.clientY),
  };
  if (sel.r - sel.l < 4 && sel.b - sel.t < 4) {
    editor.setSelection([]); // click empty = deselect
    return;
  }
  const ids = [];
  for (const el of canvasEl.value.querySelectorAll("[data-node-id]")) {
    const r = el.getBoundingClientRect();
    if (r.left < sel.r && r.right > sel.l && r.top < sel.b && r.bottom > sel.t) {
      ids.push(el.dataset.nodeId);
    }
  }
  editor.setSelection(ids);
}

// --- right-click context menu (at the cursor) → append a node -------------
const menu = ref({ open: false, x: 0, y: 0 });
function onContextMenu(e) {
  const rect = canvasEl.value.getBoundingClientRect();
  menu.value = { open: true, x: e.clientX - rect.left, y: e.clientY - rect.top };
}
function addPick(pick) {
  props.root.body.push(nodeFromPick(pick));
  menu.value.open = false;
}
</script>

<template>
  <div
    ref="canvasEl"
    class="canvas"
    :class="{ panning }"
    :style="{
      backgroundSize: `${scale}px ${scale}px, ${scale}px ${scale}px`,
      backgroundPosition: `${pan.x}px ${pan.y}px, ${pan.x}px ${pan.y}px`,
    }"
    @mousedown="onDown"
    @mousemove="onMove"
    @mouseup="onUp"
    @mouseleave="onUp"
    @wheel="onWheel"
    @contextmenu.prevent="onContextMenu"
  >
    <div class="ruler">
      <div v-for="t in ticks" :key="t.k" class="tick" :style="{ left: `${t.left}px` }">{{ t.k }}s</div>
    </div>

    <div
      v-if="selecting"
      class="rubber"
      :style="{ left: `${selRect.l}px`, top: `${selRect.t}px`, width: `${selRect.w}px`, height: `${selRect.h}px` }"
    />

    <div class="content" :style="{ transform: `translate(${pan.x}px, ${pan.y}px)` }">
      <div class="seq">
        <TriggerCard
          :name="triggerName"
          :icon="triggerIcon"
          :running="running"
          @test="emit('test')"
        />
        <SequenceFlow :nodes="root.body" :plugins="plugins" />
      </div>
    </div>

    <div
      v-if="menu.open"
      class="ctx-menu"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
      @mousedown.stop
      @contextmenu.prevent.stop
    >
      <NodePalette :plugins="plugins" @pick="addPick" />
    </div>
  </div>
</template>

<style scoped>
.canvas {
  position: relative;
  flex: 1;
  overflow: hidden;
  background-color: #161616;
  background-image:
    linear-gradient(#242424 1px, transparent 1px),
    linear-gradient(90deg, #242424 1px, transparent 1px);
  cursor: default;
}
.canvas.panning {
  cursor: grabbing;
}
.ruler {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 20px;
  background: #1b1b1b;
  border-bottom: 1px solid #2c2c2c;
  overflow: hidden;
  pointer-events: none;
  z-index: 2;
}
.tick {
  position: absolute;
  top: 0;
  height: 20px;
  line-height: 20px;
  padding-left: 4px;
  border-left: 1px solid #333;
  font-size: 0.68rem;
  color: #888;
}
.rubber {
  position: absolute;
  border: 1px dashed #8dc1a9;
  background: rgba(141, 193, 169, 0.1);
  z-index: 3;
  pointer-events: none;
}
.content {
  position: absolute;
  top: 0;
  left: 0;
}
.seq {
  display: flex;
  flex-direction: row;
  align-items: flex-start;
  gap: 6px;
}
.ctx-menu {
  position: absolute;
  z-index: 20;
  background: #1e1e1e;
  border: 1px solid #383838;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  overflow: hidden;
}
</style>
