<!-- Renders a MODULE-contributed view/page (its `entry` HTML) in a sandboxed
     iframe served from its own origin via the `moduleui://` scheme (own
     document/CSP → inline scripts run). The page drives its OWN module through a
     request/response bridge; it can never reach another module.

       host → page : { source:"quicktys", type:"init", moduleId, locale, widgets }
                     { source:"quicktys", type:"result", reqId, ok, value|error }
       page → host : { source:"quicktys-view", type:"ready" }
                     { source:"quicktys-view", type:"call", reqId, service, args }
                     { source:"quicktys-view", type:"copy", text }
                     { source:"quicktys-view", type:"open", section } -->
<script setup>
import { computed, inject, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "primevue/usetoast";
import { ipc } from "../ipc.js";

const { t, te } = useI18n();

const props = defineProps({
  moduleId: { type: String, required: true },
  entry: { type: String, default: "index.html" },
});

const locale = inject("locale", ref("en"));
const open = inject("open-section", () => {});
const toast = useToast();

// Native custom scheme (own origin, no inherited CSP). Windows serves custom
// schemes as `http://<scheme>.localhost/…`.
function uiUrl() {
  const windowsStyle = location.protocol === "http:" && location.hostname.endsWith(".localhost");
  const base = windowsStyle ? "http://moduleui.localhost" : "moduleui://localhost";
  return `${base}/${props.moduleId}/${props.entry}`;
}
const src = computed(() => (ipc.isNative ? uiUrl() : null));

const frame = ref(null);
function post(msg) {
  frame.value?.contentWindow?.postMessage(msg, "*");
}

const tr = (key, fallback) => (te(key) ? t(key) : fallback);

async function sendInit() {
  const raw = (await ipc.listWidgets()) ?? [];
  // Widget manifests carry i18n KEYS, never translated strings (ARCHITECTURE
  // §10), and a sandboxed module page has no access to the app's i18n. So
  // resolve the display labels here: without them a view rendering a widget's
  // settings would have to show raw schema keys (`fontFamily`, `warnAt`).
  const widgets = raw.map((w) => {
    const configLabels = {};
    const optionLabels = {};
    for (const [key, schema] of Object.entries(w.config?.properties ?? {})) {
      configLabels[key] = tr(`widget.${w.id}.config.${key}`, key);
      if (Array.isArray(schema.enum)) {
        optionLabels[key] = Object.fromEntries(
          schema.enum.map((v) => [v, tr(`widget.${w.id}.options.${key}.${v}`, String(v))]),
        );
      }
    }
    return { ...w, label: tr(`widget.${w.id}.name`, w.name || w.id), configLabels, optionLabels };
  });
  post({ source: "quicktys", type: "init", moduleId: props.moduleId, locale: locale.value, widgets });
}

async function onMessage(e) {
  if (frame.value && e.source !== frame.value.contentWindow) return;
  const d = e.data;
  if (!d || d.source !== "quicktys-view") return;
  switch (d.type) {
    case "ready":
      sendInit();
      break;
    case "call": {
      // Scoped: always this module — the page cannot target another.
      try {
        const value = await ipc.callModule(props.moduleId, d.service, d.args ?? {});
        post({ source: "quicktys", type: "result", reqId: d.reqId, ok: true, value });
      } catch (err) {
        post({ source: "quicktys", type: "result", reqId: d.reqId, ok: false, error: String(err) });
      }
      break;
    }
    case "copy":
      try {
        await navigator.clipboard.writeText(d.text ?? "");
        toast.add({ severity: "success", detail: d.text ?? "", life: 1500 });
      } catch {
        /* clipboard blocked */
      }
      break;
    case "open":
      if (d.section) open(d.section);
      break;
  }
}

onMounted(() => window.addEventListener("message", onMessage));
onUnmounted(() => window.removeEventListener("message", onMessage));
</script>

<template>
  <div class="viewframe">
    <iframe v-if="src" ref="frame" class="vf" sandbox="allow-scripts" :src="src" />
    <div v-else class="vf-placeholder">
      <i class="pi pi-desktop" />
      <p>This page is provided by the <strong>{{ moduleId }}</strong> extension.</p>
      <p class="hint">Open the desktop app to use it (custom scheme unavailable in the browser preview).</p>
    </div>
  </div>
</template>

<style scoped>
.viewframe,
.vf {
  width: 100%;
  height: 100%;
  border: 0;
  display: block;
}
.vf {
  background: #141414;
}
.vf-placeholder {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  opacity: 0.7;
}
.vf-placeholder i {
  font-size: 3rem;
  color: #444;
}
.vf-placeholder .hint {
  font-size: 0.85rem;
  opacity: 0.7;
}
</style>
