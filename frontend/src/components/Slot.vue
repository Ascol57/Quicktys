<!-- A named UI slot (e.g. `webview.toolbar`). Renders the contributions plugins
     declared for this slot: a built-in **button** (runs the plugin's action on
     click, with the slot's `context` merged into the action config), or a
     **custom** HTML fragment the plugin ships (in a sandboxed iframe, fed the
     same context). Place `<Slot id="..." :context="{...}"/>` wherever plugins
     should be able to inject UI. -->
<script setup>
import { computed, inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import Button from "primevue/button";
import SlotFrame from "./SlotFrame.vue";
import { ipc } from "../ipc.js";

const props = defineProps({
  id: { type: String, required: true },
  context: { type: Object, default: () => ({}) },
});
const { t, te } = useI18n();
const plugins = inject("plugins", ref([]));

// { plugin, decl } for every contribution targeting this slot.
const contributions = computed(() =>
  (plugins.value ?? []).flatMap((p) =>
    (p.ui ?? []).filter((u) => u.slot === props.id).map((decl) => ({ plugin: p.id, decl })),
  ),
);

const label = (decl) => (decl.label && te(decl.label) ? t(decl.label) : decl.label ?? "");

// Click a button contribution → run its action with the slot context merged in.
function run(plugin, decl) {
  ipc.runSequence({
    kind: "action",
    plugin,
    action: decl.action,
    config: { ...(decl.config ?? {}), ...props.context },
  });
}
</script>

<template>
  <span v-if="contributions.length" class="slot">
    <template v-for="(c, i) in contributions" :key="i">
      <Button
        v-if="c.decl.action"
        size="small"
        severity="secondary"
        :title="label(c.decl)"
        @click="run(c.plugin, c.decl)"
      >
        <i v-if="c.decl.icon" :class="c.decl.icon" />
        <span v-if="label(c.decl)">{{ label(c.decl) }}</span>
      </Button>
      <SlotFrame
        v-else-if="c.decl.entry"
        :plugin="c.plugin"
        :entry="c.decl.entry"
        :context="context"
      />
    </template>
  </span>
</template>

<style scoped>
.slot {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
</style>
