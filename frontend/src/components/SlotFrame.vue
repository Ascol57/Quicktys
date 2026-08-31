<!-- Renders a plugin's custom slot UI (`entry` HTML) in a sandboxed iframe served
     from its own origin via the `pluginui://` scheme (own document/CSP → inline
     scripts run). Bridges it like a widget:
       host → ui : { source:"quicktys", type:"context", context }
       ui → host : { source:"quicktys-widget", type:"ready" }
                   { source:"quicktys-widget", type:"action", plugin, action, config } -->
<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ipc } from "../ipc.js";

const props = defineProps({
  plugin: { type: String, required: true },
  entry: { type: String, required: true },
  context: { type: Object, default: () => ({}) },
});

// Native custom scheme (own origin, no inherited CSP). Windows serves custom
// schemes as `http://<scheme>.localhost/…`.
function uiUrl() {
  const windowsStyle = location.protocol === "http:" && location.hostname.endsWith(".localhost");
  const base = windowsStyle ? "http://pluginui.localhost" : "pluginui://localhost";
  return `${base}/${props.plugin}/${props.entry}`;
}
const src = computed(() => (ipc.isNative ? uiUrl() : null));

const frame = ref(null);
function postContext() {
  frame.value?.contentWindow?.postMessage(
    { source: "quicktys", type: "context", context: props.context ?? {} },
    "*",
  );
}
function onMessage(e) {
  if (frame.value && e.source !== frame.value.contentWindow) return;
  const d = e.data;
  if (!d || d.source !== "quicktys-widget") return;
  if (d.type === "ready") {
    postContext();
  } else if (d.type === "action" && d.plugin && d.action) {
    ipc.runSequence({ kind: "action", plugin: d.plugin, action: d.action, config: d.config ?? {} });
  }
}
onMounted(() => {
  window.addEventListener("message", onMessage);
  postContext();
});
watch(() => props.context, postContext, { deep: true });
onUnmounted(() => window.removeEventListener("message", onMessage));
</script>

<template>
  <iframe ref="frame" class="sf" sandbox="allow-scripts" :src="src || undefined" />
</template>

<style scoped>
.sf {
  border: 0;
  width: 100%;
  height: 100%;
  min-height: 2rem;
  display: block;
  background: transparent;
}
</style>
