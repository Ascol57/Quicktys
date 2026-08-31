<!-- Searchable, categorised palette: flow-control nodes plus one category per
     plugin with its actions. Emits `pick` — either {kind} for a flow node, or
     {kind:'action', plugin, action} for a plugin action. Clean-room design
     informed by CastMate's filterable action palette. -->
<script setup>
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { PALETTE } from "../nodes.js";
import { actionLabel, pluginLabel } from "../labels.js";

const props = defineProps({
  plugins: { type: Array, default: () => [] },
});
const emit = defineEmits(["pick"]);
const { t, te } = useI18n();

const query = ref("");
const searchEl = ref(null);
onMounted(() => searchEl.value?.focus());

const entries = computed(() => {
  const list = [];
  for (const p of PALETTE) {
    list.push({
      id: `flow:${p.kind}`,
      label: p.label,
      icon: p.icon,
      color: p.color,
      category: p.category,
      extra: p.keywords ?? "",
      pick: { kind: p.kind },
    });
  }
  for (const plugin of props.plugins) {
    for (const action of plugin.actions ?? []) {
      list.push({
        id: `act:${plugin.id}:${action}`,
        label: actionLabel(plugin.id, action, t, te),
        icon: plugin.icon,
        color: plugin.color,
        category: pluginLabel(plugin, t, te),
        extra: `${plugin.id} ${action}`,
        pick: { kind: "action", plugin: plugin.id, action },
      });
    }
  }
  return list;
});

const filtered = computed(() => {
  const s = query.value.trim().toLowerCase();
  if (!s) return entries.value;
  return entries.value.filter(
    (e) =>
      e.label.toLowerCase().includes(s) ||
      e.category.toLowerCase().includes(s) ||
      (e.extra ?? "").toLowerCase().includes(s),
  );
});

const groups = computed(() => {
  const map = new Map();
  for (const e of filtered.value) {
    if (!map.has(e.category)) map.set(e.category, []);
    map.get(e.category).push(e);
  }
  return [...map.entries()].map(([category, items]) => ({ category, items }));
});

function choose(entry) {
  emit("pick", entry.pick);
}
function onEnter() {
  if (filtered.value.length) choose(filtered.value[0]);
}
</script>

<template>
  <div class="np">
    <div class="np__search">
      <i class="pi pi-search" />
      <input
        ref="searchEl"
        v-model="query"
        placeholder="Search actions & nodes…"
        @keydown.stop
        @keyup.enter="onEnter"
      />
    </div>
    <div class="np__list">
      <template v-for="g in groups" :key="g.category">
        <div class="np__cat">{{ g.category }}</div>
        <button v-for="e in g.items" :key="e.id" class="np__item" @click="choose(e)">
          <i :class="e.icon" :style="{ color: e.color }" />
          <span>{{ e.label }}</span>
        </button>
      </template>
      <div v-if="!filtered.length" class="np__empty">No match</div>
    </div>
  </div>
</template>

<style scoped>
.np {
  width: 15rem;
  display: flex;
  flex-direction: column;
}
.np__search {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.5rem;
  border-bottom: 1px solid #333;
  color: #888;
}
.np__search input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #eee;
  font-size: 0.85rem;
}
.np__list {
  max-height: 16rem;
  overflow-y: auto;
  padding: 0.25rem;
}
.np__cat {
  font-size: 0.66rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  opacity: 0.5;
  margin: 0.4rem 0.25rem 0.15rem;
  border-bottom: 1px solid #2c2c2c;
}
.np__item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  background: transparent;
  border: none;
  color: #eee;
  cursor: pointer;
  padding: 0.3rem 0.4rem;
  border-radius: 4px;
  font-size: 0.82rem;
  text-align: left;
}
.np__item:hover {
  background: #2c2c2c;
}
.np__empty {
  padding: 0.6rem;
  opacity: 0.5;
  font-size: 0.8rem;
  text-align: center;
}
</style>
