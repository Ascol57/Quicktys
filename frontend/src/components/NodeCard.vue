<!-- An action/flow node as a fixed-height card in a horizontal sequence.
     Design studied from CastMate (clean-room): header = darker shade (drag
     handle) + name, body = base shade with a compact config preview. The full
     editable config lives in the side Inspector when selected. -->
<script setup>
import { computed, inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import { meta, label } from "../nodes.js";
import { actionLabel, pluginLabel } from "../labels.js";

const { t, te } = useI18n();

const props = defineProps({
  node: { type: Object, required: true },
  plugins: { type: Array, default: () => [] },
  selected: Boolean,
});
defineEmits(["select", "remove"]);

const editor = inject("automation-editor", null);
const pxPerMs = inject("time-px-per-ms", ref(0.12));

const testing = computed(() => editor?.isTesting(props.node._id) ?? false);

// Width scales with duration for time-based nodes (a `wait-for` delay).
const widthStyle = computed(() => {
  if (props.node.kind === "wait-for" && props.node.wait === "delay") {
    const ms = Number(props.node.ms) || 0;
    return { width: `${Math.max(122, ms * pxPerMs.value)}px` };
  }
  return {};
});

const plugin = computed(() =>
  props.node.kind === "action" ? props.plugins.find((p) => p.id === props.node.plugin) : null,
);
const accent = computed(() => plugin.value?.color || meta(props.node).color);
const icon = computed(() => plugin.value?.icon || meta(props.node).icon);
const title = computed(() =>
  props.node.kind === "action"
    ? (props.node.action ? actionLabel(props.node.plugin, props.node.action, t, te) : "(pick action)")
    : label(props.node),
);
// For action nodes, which plugin the action belongs to.
const pluginName = computed(() =>
  props.node.kind === "action" ? pluginLabel(plugin.value ?? { id: props.node.plugin }, t, te) : null,
);

const preview = computed(() => {
  const n = props.node;
  if (n.kind === "action") {
    return Object.values(n.config ?? {}).map((v) =>
      typeof v === "object" ? JSON.stringify(v) : String(v),
    );
  }
  switch (n.kind) {
    case "if":
      return [n.cond];
    case "switch":
      return [n.value, `${(n.cases ?? []).length} case(s)`];
    case "loop":
      return [n.loop === "for-each" ? `${n.var} ∈ ${n.list}` : `${n.loop} ${n.count ?? n.cond ?? ""}`];
    case "wait-for":
      return [n.wait === "delay" ? `${n.ms} ms` : `${n.cond ?? n.trigger ?? ""}`];
    case "parallel":
      return [n.join, `${(n.branches ?? []).length} branch(es)`];
    case "timeline":
      return [`${(n.tracks ?? []).length} track(s)`];
    case "let":
      return [`${n.name} = ${n.value}`];
    case "set":
      return [`${n.target} = ${n.value}`];
    case "block":
      return [`${(n.body ?? []).length} step(s)`];
    default:
      return [];
  }
});
</script>

<template>
  <div
    class="card"
    :class="{ selected, testing }"
    :style="{ '--c': accent, ...widthStyle }"
    :data-node-id="node._id"
    @click.stop="$emit('select')"
  >
    <div class="card__header">
      <i :class="icon" class="card__icon" />
      <div class="card__names">
        <span v-if="pluginName" class="card__plugin">{{ pluginName }}</span>
        <span class="card__title">{{ title }}</span>
      </div>
      <i class="card__close pi pi-times" @click.stop="$emit('remove')" />
    </div>
    <div class="card__body">
      <div v-for="(line, i) in preview" :key="i" class="card__line">{{ line }}</div>
    </div>
  </div>
</template>

<style scoped>
.card {
  --darker: color-mix(in srgb, var(--c) 58%, #000);
  --lighter: color-mix(in srgb, var(--c) 72%, #fff);
  height: 100px;
  min-width: 122px;
  max-width: 200px;
  flex: 0 0 auto;
  border-radius: 6px;
  border: 2px solid var(--lighter);
  background: var(--c);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  cursor: pointer;
  transition: border-color 0.15s;
}
.card.selected {
  border-color: #fff;
}
.card.testing {
  border-color: #ffe08a;
  box-shadow: 0 0 0 2px #ffe08a, 0 0 14px 2px rgba(255, 224, 138, 0.6);
  animation: testing-pulse 0.8s ease-in-out infinite alternate;
}
@keyframes testing-pulse {
  from {
    box-shadow: 0 0 0 2px #ffe08a, 0 0 8px 1px rgba(255, 224, 138, 0.4);
  }
  to {
    box-shadow: 0 0 0 2px #ffe08a, 0 0 18px 4px rgba(255, 224, 138, 0.75);
  }
}
.card__header {
  min-height: 1.5rem;
  display: flex;
  align-items: flex-start;
  gap: 0.3rem;
  padding: 0.2rem 0.3rem;
  background: var(--darker);
  color: #f4f4f4;
  cursor: grab;
}
.card__icon {
  margin-top: 0.12rem;
}
.card__names {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  line-height: 1.15;
}
.card__plugin {
  font-size: 0.6rem;
  font-weight: 500;
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.card__title {
  font-size: 0.82rem;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.card__close {
  font-size: 0.7rem;
  opacity: 0.6;
}
.card__close:hover {
  opacity: 1;
}
.card__body {
  flex: 1;
  padding: 0.3rem;
  font-size: 0.72rem;
  color: rgba(0, 0, 0, 0.78);
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  align-items: center;
  justify-content: center;
  text-align: center;
  overflow: hidden;
}
.card__line {
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
