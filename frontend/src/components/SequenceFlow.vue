<!-- Recursive editor for a sequence of nodes (a horizontal chain of cards).
     `parallel` renders as stacked editable branch lanes; `timeline` as tracks
     offset along the time axis. Insertion "+" drop-points sit between cards.
     Cards drag within/across lanes (group). Clean-room design informed by
     CastMate's sequence/stack/timeline model. -->
<script setup>
import { inject, ref } from "vue";
import draggable from "vuedraggable";
import Popover from "primevue/popover";
import NodeCard from "./NodeCard.vue";
import NodePalette from "./NodePalette.vue";
import { newNode, newTrack, meta, nodeFromPick } from "../nodes.js";

defineOptions({ name: "SequenceFlow" });

const props = defineProps({
  nodes: { type: Array, required: true },
  plugins: { type: Array, default: () => [] },
});

const editor = inject("automation-editor");
const pxPerMs = inject("time-px-per-ms", ref(0.12));

// --- insertion drop-points (palette popover) ------------------------------
const insertPop = ref(null);
const pendingIndex = ref(0);
function openInsert(index, ev) {
  pendingIndex.value = index;
  insertPop.value.toggle(ev);
}
function insert(pick) {
  props.nodes.splice(pendingIndex.value, 0, nodeFromPick(pick));
  insertPop.value.hide();
}

function removeAt(i) {
  const [removed] = props.nodes.splice(i, 1);
  if (removed && editor.isSelected(removed._id)) editor.select(null);
}

// --- parallel / timeline helpers ------------------------------------------
const branchNodes = (branch) => (branch.kind === "block" ? branch.body : null);
const trackNodes = (track) => (track.body?.kind === "block" ? track.body.body : null);
function addBranch(node) {
  node.branches.push(newNode("block"));
}
</script>

<template>
  <div class="flow">
    <draggable
      :list="nodes"
      item-key="_id"
      :group="{ name: 'seq' }"
      handle=".card__header"
      :animation="150"
      class="flow__row"
    >
      <template #item="{ element, index }">
        <div class="item">
          <span class="insert" title="Insert here" @click.stop="openInsert(index, $event)"
            ><i class="pi pi-plus"
          /></span>

          <!-- PARALLEL -->
          <div
            v-if="element.kind === 'parallel'"
            class="block"
            :class="{ selected: editor.isSelected(element._id), testing: editor.isTesting(element._id) }"
            :style="{ '--c': meta(element).color }"
            :data-node-id="element._id"
          >
            <div class="block__header card__header" @click.stop="editor.select(element._id)">
              <i :class="meta(element).icon" />
              <span class="block__title">Parallel · {{ element.join }}</span>
              <i class="block__close pi pi-times" @click.stop="removeAt(index)" />
            </div>
            <div class="block__lanes">
              <div v-for="(branch, bi) in element.branches" :key="branch._id ?? bi" class="lane">
                <SequenceFlow v-if="branchNodes(branch)" :nodes="branchNodes(branch)" :plugins="plugins" />
                <i class="lane__remove pi pi-times" @click.stop="element.branches.splice(bi, 1)" />
              </div>
              <button class="add-lane" @click.stop="addBranch(element)"><i class="pi pi-plus" /> branch</button>
            </div>
          </div>

          <!-- TIMELINE -->
          <div
            v-else-if="element.kind === 'timeline'"
            class="block"
            :class="{ selected: editor.isSelected(element._id), testing: editor.isTesting(element._id) }"
            :style="{ '--c': meta(element).color }"
            :data-node-id="element._id"
          >
            <div class="block__header card__header" @click.stop="editor.select(element._id)">
              <i :class="meta(element).icon" />
              <span class="block__title">Timeline</span>
              <i class="block__close pi pi-times" @click.stop="removeAt(index)" />
            </div>
            <div class="block__lanes">
              <div v-for="(track, ti) in element.tracks" :key="track._id ?? ti" class="track">
                <div class="track__meta">
                  <input
                    class="track__offset"
                    type="number"
                    min="0"
                    step="100"
                    :value="track.offset_ms"
                    @input="track.offset_ms = Number($event.target.value)"
                  />
                  <span class="track__unit">ms</span>
                  <i class="lane__remove pi pi-times" @click.stop="element.tracks.splice(ti, 1)" />
                </div>
                <div class="track__lane" :style="{ paddingLeft: `${track.offset_ms * pxPerMs.value}px` }">
                  <SequenceFlow v-if="trackNodes(track)" :nodes="trackNodes(track)" :plugins="plugins" />
                </div>
              </div>
              <button class="add-lane" @click.stop="element.tracks.push(newTrack())"><i class="pi pi-plus" /> track</button>
            </div>
          </div>

          <!-- Regular / flow node -->
          <NodeCard
            v-else
            :node="element"
            :plugins="plugins"
            :selected="editor.isSelected(element._id)"
            @select="editor.select(element._id)"
            @remove="removeAt(index)"
          />
        </div>
      </template>
    </draggable>

    <span class="insert insert--end" title="Add node" @click.stop="openInsert(nodes.length, $event)">
      <i class="pi pi-plus" />
    </span>

    <Popover ref="insertPop">
      <NodePalette :plugins="plugins" @pick="insert" />
    </Popover>
  </div>
</template>

<style scoped>
.flow {
  display: flex;
  flex-direction: row;
  align-items: flex-start;
  gap: 0;
}
.flow__row {
  display: flex;
  flex-direction: row;
  align-items: flex-start;
  min-height: 100px;
}
.item {
  position: relative;
  display: flex;
  align-items: flex-start;
}

/* Insertion drop-points */
.insert {
  width: 16px;
  align-self: stretch;
  min-height: 100px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #8dc1a9;
  opacity: 0;
  cursor: pointer;
  font-size: 0.7rem;
  transition: opacity 0.12s;
}
.item:hover > .insert,
.insert:hover {
  opacity: 0.9;
}
.insert:hover {
  background: color-mix(in srgb, #8dc1a9 18%, transparent);
  border-radius: 4px;
}
.insert--end {
  opacity: 0.5;
  align-self: center;
  min-height: auto;
  width: 26px;
  height: 26px;
}
.insert--end:hover {
  opacity: 1;
}

/* Parallel / timeline containers */
.block {
  --darker: color-mix(in srgb, var(--c) 58%, #000);
  --lighter: color-mix(in srgb, var(--c) 72%, #fff);
  border: 2px solid var(--lighter);
  border-radius: 6px;
  background: color-mix(in srgb, var(--c) 16%, #161616);
  display: flex;
  flex-direction: column;
  transition: border-color 0.15s;
}
.block.selected {
  border-color: #fff;
}
.block.testing {
  border-color: #ffe08a;
  box-shadow: 0 0 0 2px #ffe08a, 0 0 14px 2px rgba(255, 224, 138, 0.55);
  animation: block-testing-pulse 0.8s ease-in-out infinite alternate;
}
@keyframes block-testing-pulse {
  from {
    box-shadow: 0 0 0 2px #ffe08a, 0 0 8px 1px rgba(255, 224, 138, 0.35);
  }
  to {
    box-shadow: 0 0 0 2px #ffe08a, 0 0 18px 4px rgba(255, 224, 138, 0.7);
  }
}
.block__header {
  height: 1.5rem;
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0 0.4rem;
  background: var(--darker);
  color: #f4f4f4;
  font-weight: 600;
  font-size: 0.8rem;
  cursor: grab;
}
.block__title {
  flex: 1;
  white-space: nowrap;
}
.block__close {
  opacity: 0.6;
  font-size: 0.7rem;
  cursor: pointer;
}
.block__close:hover {
  opacity: 1;
}
.block__lanes {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
}
.lane {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 4px;
  padding-left: 8px;
  border-left: 3px solid var(--lighter);
  min-height: 100px;
}
.lane__remove {
  opacity: 0.35;
  font-size: 0.7rem;
  cursor: pointer;
}
.lane__remove:hover {
  opacity: 0.9;
}
.track {
  border-left: 3px solid var(--lighter);
  padding-left: 8px;
}
.track__meta {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.72rem;
  opacity: 0.8;
  margin-bottom: 2px;
}
.track__offset {
  width: 4.5rem;
  background: #1c1c1c;
  border: 1px solid #333;
  border-radius: 4px;
  color: #eee;
  padding: 0.1rem 0.3rem;
  font-size: 0.72rem;
}
.add-lane {
  align-self: flex-start;
  background: transparent;
  border: 1px dashed var(--lighter);
  color: #ccc;
  border-radius: 5px;
  padding: 0.2rem 0.5rem;
  font-size: 0.72rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 0.3rem;
}
.add-lane:hover {
  background: color-mix(in srgb, var(--c) 20%, transparent);
}
</style>
