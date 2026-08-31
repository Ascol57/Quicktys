<!-- Live reactive state (read-only for now): the host's state store, refreshed
     on demand and updated live via `stateChanged` events. -->
<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Button from "primevue/button";
import { ipc } from "../../ipc.js";

const { t } = useI18n();
const state = ref({});
let unlisten = null;

async function refresh() {
  state.value = (await ipc.getState()) ?? {};
}
onMounted(async () => {
  await refresh();
  unlisten = await ipc.onEvent((e) => {
    if (e?.type === "stateChanged") state.value = { ...state.value, [e.key]: e.value };
  });
});
onUnmounted(() => unlisten && unlisten());

const rows = computed(() =>
  Object.entries(state.value).map(([name, value]) => ({
    name,
    value: typeof value === "object" ? JSON.stringify(value) : String(value),
  })),
);
</script>

<template>
  <div class="vars">
    <div class="head">
      <h2>{{ t("core.variables.title") }}</h2>
      <Button size="small" text @click="refresh"><i class="pi pi-refresh" /> {{ t("core.variables.refresh") }}</Button>
    </div>

    <p v-if="!rows.length" class="muted">{{ t("core.variables.none") }}</p>
    <table v-else class="tbl">
      <thead>
        <tr>
          <th>{{ t("core.variables.name") }}</th>
          <th>{{ t("core.variables.value") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="r in rows" :key="r.name">
          <td class="mono">{{ r.name }}</td>
          <td class="mono val">{{ r.value }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.vars {
  padding: 1.5rem;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1rem;
}
.head h2 {
  margin: 0;
}
.tbl {
  width: 100%;
  border-collapse: collapse;
  max-width: 720px;
}
.tbl th {
  text-align: left;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  opacity: 0.5;
  padding: 0.4rem 0.6rem;
  border-bottom: 1px solid #2a2a2a;
}
.tbl td {
  padding: 0.4rem 0.6rem;
  border-bottom: 1px solid #202020;
}
.mono {
  font-family: ui-monospace, monospace;
  font-size: 0.85rem;
}
.val {
  color: #7ec27e;
}
.muted {
  opacity: 0.55;
}
</style>
