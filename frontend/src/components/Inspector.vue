<!-- Right-hand inspector: edits the selected node (CastMate-style). -->
<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import SchemaForm from "./SchemaForm.vue";
import AutomationNode from "./AutomationNode.vue";
import { ipc } from "../ipc.js";
import { meta, label } from "../nodes.js";
import { actionLabel, pluginLabel } from "../labels.js";

const { t, te } = useI18n();

const props = defineProps({
  node: { type: Object, default: null },
  plugins: { type: Array, default: () => [] },
});
const emit = defineEmits(["update:node", "remove"]);
const patch = (partial) => emit("update:node", { ...props.node, ...partial });

const schema = ref(null);

const actionPlugin = computed(() =>
  props.node?.kind === "action" ? props.plugins.find((p) => p.id === props.node.plugin) : null,
);
const titleIcon = computed(() =>
  props.node ? (actionPlugin.value?.icon ?? meta(props.node).icon) : "",
);
const titleColor = computed(() =>
  props.node ? (actionPlugin.value?.color ?? meta(props.node).color) : "",
);
const titleText = computed(() => {
  if (!props.node) return "";
  if (props.node.kind === "action") {
    const name = pluginLabel(actionPlugin.value ?? { id: props.node.plugin }, t, te);
    const act = props.node.action ? actionLabel(props.node.plugin, props.node.action, t, te) : "(pick action)";
    return `${name} · ${act}`;
  }
  return label(props.node);
});

async function loadSchema() {
  if (props.node?.kind === "action" && props.node.plugin && props.node.action) {
    schema.value = await ipc.actionSchema(props.node.plugin, props.node.action);
  } else {
    schema.value = null;
  }
}
watch(() => [props.node?._id, props.node?.plugin, props.node?.action], loadSchema, {
  immediate: true,
});
</script>

<template>
  <aside class="inspector">
    <template v-if="node">
      <div class="title">
        <i :class="titleIcon" :style="{ color: titleColor }" />
        {{ titleText }}
      </div>

      <template v-if="node.kind === 'action'">
        <SchemaForm
          v-if="schema"
          :schema="schema"
          :modelValue="node.config"
          @update:modelValue="patch({ config: $event })"
        />
        <p v-else class="muted">This action has no configuration.</p>
      </template>

      <!-- Flow / structural nodes: the full nested editor. -->
      <AutomationNode
        v-else
        :node="node"
        :plugins="plugins"
        @update:node="emit('update:node', $event)"
        @remove="emit('remove')"
        @duplicate="() => {}"
      />
    </template>

    <p v-else class="muted">Select a node to edit it.</p>
  </aside>
</template>

<style scoped>
.inspector {
  width: 360px;
  flex: 0 0 360px;
  border-left: 1px solid #2a2a2a;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  overflow: auto;
}
.title {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  font-size: 1.1rem;
  font-weight: 600;
  text-align: center;
  padding-bottom: 0.25rem;
  border-bottom: 1px solid #2a2a2a;
}
.muted {
  opacity: 0.55;
}
</style>
