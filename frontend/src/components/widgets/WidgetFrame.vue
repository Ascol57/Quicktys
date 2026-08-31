<!-- Runtime widget host: renders a widget bundle's `index.html` in a sandboxed
     iframe and bridges it to the app.
       host → widget : { source:"quicktys", type:"state", state }
                       { source:"quicktys", type:"config", config }
       widget → host : { source:"quicktys-widget", type:"ready" }
                       { source:"quicktys-widget", type:"action", plugin, action, config } -->
<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ipc } from "../../ipc.js";
import { plain } from "../../plain.js";

const props = defineProps({
  widgetId: { type: String, default: "" },
  html: { type: String, default: "" },
  // Per-instance widget config (its settings) — forwarded to the iframe.
  config: { type: Object, default: () => ({}) },
  // Editor preview: render the widget's separate `preview.html` (a self-demo)
  // instead of `index.html`. The browser-source page never sets this, so an OBS
  // overlay always uses the real, idle widget.
  preview: { type: Boolean, default: false },
  // Embedded preview page (browser mock only); native loads it via `widget://`.
  previewHtml: { type: String, default: "" },
});

// Native: load from the `widget://` custom scheme so the widget gets its own
// document/CSP (inline scripts run). Windows serves custom schemes as
// `http://<scheme>.localhost/…` (detected: the app itself is then on a
// `*.localhost` http origin), elsewhere as `<scheme>://localhost/…`. Browser:
// no such host, use the embedded HTML via srcdoc (dev has no strict CSP).
function widgetUrl(id, file) {
  const windowsStyle =
    location.protocol === "http:" && location.hostname.endsWith(".localhost");
  const base = windowsStyle ? `http://widget.localhost/${id}/` : `widget://localhost/${id}/`;
  return base + (file || "");
}
// Preview only takes effect if the widget actually ships a preview page.
const usePreview = computed(() => props.preview && !!props.previewHtml);
const src = computed(() =>
  ipc.isNative && props.widgetId
    ? widgetUrl(props.widgetId, usePreview.value ? "preview.html" : "")
    : null,
);
// Browser mock: srcdoc from the embedded preview page or the regular html.
const doc = computed(() => (usePreview.value ? props.previewHtml : props.html));

const frame = ref(null);
let state = {};
let unlisten = null;

function postState() {
  frame.value?.contentWindow?.postMessage(
    { source: "quicktys", type: "state", state: plain(state) },
    "*",
  );
}
function postConfig() {
  frame.value?.contentWindow?.postMessage(
    { source: "quicktys", type: "config", config: plain(props.config ?? {}) },
    "*",
  );
}
function onMessage(e) {
  if (frame.value && e.source !== frame.value.contentWindow) return; // only our iframe
  const d = e.data;
  if (!d || d.source !== "quicktys-widget") return;
  if (d.type === "ready") {
    postConfig();
    postState();
  } else if (d.type === "action" && d.plugin && d.action) {
    // Run the requested action as a one-node sequence.
    ipc.runSequence({ kind: "action", plugin: d.plugin, action: d.action, config: d.config ?? {} });
  }
}

onMounted(async () => {
  state = (await ipc.getState()) ?? {};
  window.addEventListener("message", onMessage);
  unlisten = await ipc.onEvent((ev) => {
    if (ev?.type === "stateChanged") {
      state = { ...state, [ev.key]: ev.value };
      postState();
    }
  });
  postConfig();
  postState();
});
// Live-update the widget when its settings change (dashboard/overlay editor).
watch(() => props.config, postConfig, { deep: true });
onUnmounted(() => {
  window.removeEventListener("message", onMessage);
  if (unlisten) unlisten();
});
</script>

<template>
  <iframe
    ref="frame"
    class="wf"
    sandbox="allow-scripts"
    :src="src || undefined"
    :srcdoc="src ? undefined : doc"
  />
</template>

<style scoped>
.wf {
  width: 100%;
  height: 100%;
  min-height: 0; /* allow small widgets (e.g. compact overlay boxes) */
  border: 0;
  display: block;
  background: #1e1e1e; /* match the card; iframes paint white by default */
}
</style>
