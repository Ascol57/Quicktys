<!-- Customizable dashboard: a grid of widgets you can add, reorder (drag),
     resize and configure in Edit mode. Layout persists (localStorage). The only
     widget type so far is `info` — a box showing a count for a chosen source. -->
<script setup>
import { computed, inject, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Button from "primevue/button";
import Popover from "primevue/popover";
import Dialog from "primevue/dialog";
import InfoWidget from "../widgets/InfoWidget.vue";
import ButtonWidget from "../widgets/ButtonWidget.vue";
import WidgetFrame from "../widgets/WidgetFrame.vue";
import WidgetConfigForm from "../widgets/WidgetConfigForm.vue";
import { useDashboard, GRID } from "../../stores/dashboard.js";
import { useProfiles } from "../../stores/profiles.js";
import { ipc } from "../../ipc.js";

const { t, te } = useI18n();
const plugins = inject("plugins", ref([]));
const open = inject("open-section", () => {});

const board = useDashboard();
const profilesStore = useProfiles();
const modules = ref([]);
const automations = ref([]);
const overlays = ref([]);
const catalog = ref([]); // widget bundles (list_widgets)

onMounted(async () => {
  await profilesStore.load();
  modules.value = await ipc.listModules();
  automations.value = await ipc.listResources("automation");
  overlays.value = (await ipc.callModuleSafe("webview", "list-overlays", {})) ?? [];
  catalog.value = await ipc.listWidgets();
});

// Built-in renderers keyed by a widget's `template`. Widget metadata + config
// schema come from the bundle (list_widgets); the render component is in-app.
const RENDERERS = { info: InfoWidget, button: ButtonWidget };
const def = (type) => catalog.value.find((w) => w.id === type) ?? null;
const templateOf = (type) => def(type)?.template;
const rendererFor = (type) => RENDERERS[templateOf(type)] ?? InfoWidget;
const isButton = (type) => templateOf(type) === "button";

// Presentation (icon/colour) for each info source; available keys come from the
// widget bundle's config schema enum.
const SOURCE_LOOK = {
  plugins: { icon: "pi pi-box", color: "#7ec27e" },
  modules: { icon: "pi pi-server", color: "#5fa8d3" },
  profiles: { icon: "pi pi-sitemap", color: "#9a5fd0" },
  automations: { icon: "pi pi-bolt", color: "#e0a458" },
  widgets: { icon: "pi pi-desktop", color: "#d3695f" },
  webview: { icon: "pi pi-window-maximize", color: "#c98bdb" },
};
function sourceOptions(type) {
  const keys = def(type)?.config?.properties?.source?.enum ?? Object.keys(SOURCE_LOOK);
  return keys.map((k) => ({ value: k, label: t(`core.dashboard.${k}`) }));
}

const counts = computed(() => ({
  plugins: plugins.value.length,
  modules: modules.value.length,
  profiles: profilesStore.list.length,
  automations: automations.value.length,
  widgets: catalog.value.length,
  webview: overlays.value.length,
}));

// Default config from a widget's schema: explicit `default`, else first enum
// value, else a per-type default.
function defaultConfig(w) {
  const cfg = {};
  for (const [key, s] of Object.entries(w.config?.properties ?? {})) {
    if (s.default !== undefined) cfg[key] = s.default;
    else if (Array.isArray(s.enum)) cfg[key] = s.enum[0];
    else {
      const t = String(s.type || "").toLowerCase();
      cfg[key] = t === "boolean" ? true : t === "number" || t === "integer" ? 0 : "";
    }
  }
  return cfg;
}

// The widget id currently flashing after a click (button widget feedback).
const firedId = ref(null);

function widgetProps(w) {
  if (isButton(w.type)) {
    return {
      label: w.config?.label || "Button",
      icon: w.config?.icon || "",
      color: w.config?.color || "#7c5cff",
      fired: firedId.value === w.id,
    };
  }
  const key = w.config?.source ?? "plugins";
  const look = SOURCE_LOOK[key] ?? SOURCE_LOOK.plugins;
  return { icon: look.icon, color: look.color, count: counts.value[key] ?? 0, label: t(`core.dashboard.${key}`) };
}

// --- edit mode -------------------------------------------------------------
const editing = ref(
  typeof location !== "undefined" && new URLSearchParams(location.search).has("edit"),
);
const isRuntime = (type) => !!def(type)?.html;
function addWidget(type) {
  const w = def(type) ?? catalog.value[0];
  if (w) board.add(w.id, defaultConfig(w), w.defaultW ?? (w.html ? 2 : 1), w.defaultH ?? 1);
}

// --- "Add widget" picker (searchable menu, like the action/trigger palette) --
const pickerPop = ref(null);
const query = ref("");
const widgetLabel = (id) => (te(`widget.${id}.name`) ? t(`widget.${id}.name`) : def(id)?.name ?? id);
const widgetDesc = (id) => (te(`widget.${id}.description`) ? t(`widget.${id}.description`) : "");
const filteredCatalog = computed(() => {
  const q = query.value.trim().toLowerCase();
  return catalog.value.filter(
    (w) => !q || widgetLabel(w.id).toLowerCase().includes(q) || w.id.toLowerCase().includes(q),
  );
});
function openPicker(e) {
  query.value = "";
  pickerPop.value.toggle(e);
}
function pick(w) {
  addWidget(w.id);
  pickerPop.value.hide();
}
// --- per-widget settings modal --------------------------------------------
const widgetProperties = (type) => def(type)?.config?.properties ?? {};
const hasSettings = (type) => Object.keys(widgetProperties(type)).length > 0;
function propOptions(type, key, schema) {
  // `source` gets translated labels; other enums use their raw values.
  if (key === "source") return sourceOptions(type);
  return (schema.enum ?? []).map((v) => ({ value: v, label: String(v) }));
}
function propLabel(type, key) {
  const k = `widget.${type}.config.${key}`;
  return te(k) ? t(k) : key;
}

const settingsW = ref(null); // the widget currently being configured
function openSettings(w) {
  settingsW.value = w;
}
function updateSettings(config) {
  if (settingsW.value) board.update(settingsW.value.id, { config });
}

// --- grid geometry (widgets are freely placed; drag/resize snap to cells) ---
const COLS = GRID.cols;
const ROWS = GRID.rows;
const GAP = 16; // must match .grid gap
// Where each widget sits on the grid: `x / span w`, `y / span h`.
function cellStyle(w) {
  return { gridColumn: `${w.x ?? 1} / span ${w.w ?? 1}`, gridRow: `${w.y ?? 1} / span ${w.h ?? 1}` };
}
// Snap step for one cell, from the grid's actual size (rows are `1fr`, so the
// row step follows the available height).
function steps(e) {
  const r = e.target.closest(".grid").getBoundingClientRect();
  return { col: (r.width + GAP) / COLS, row: (r.height + GAP) / ROWS };
}

// Drag a widget to a new cell (snaps; clamped so it stays inside the grid).
let move = null;
function startMove(e, w) {
  const s = steps(e);
  move = { w, x: e.clientX, y: e.clientY, startX: w.x ?? 1, startY: w.y ?? 1, ...s };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", endMove);
}
function onMove(e) {
  if (!move) return;
  const dx = Math.round((e.clientX - move.x) / move.col);
  const dy = Math.round((e.clientY - move.y) / move.row);
  move.w.x = Math.min(COLS - (move.w.w ?? 1) + 1, Math.max(1, move.startX + dx));
  move.w.y = Math.min(ROWS - (move.w.h ?? 1) + 1, Math.max(1, move.startY + dy));
}
function endMove() {
  window.removeEventListener("mousemove", onMove);
  window.removeEventListener("mouseup", endMove);
  move = null;
  board.persist();
}

// Drag the bottom-right handle to resize (snaps; clamped to the grid bounds).
let resize = null;
function startResize(e, w) {
  const s = steps(e);
  resize = { w, x: e.clientX, y: e.clientY, startW: w.w ?? 1, startH: w.h ?? 1, ...s };
  window.addEventListener("mousemove", onResize);
  window.addEventListener("mouseup", endResize);
}
function onResize(e) {
  if (!resize) return;
  const dw = Math.round((e.clientX - resize.x) / resize.col);
  const dh = Math.round((e.clientY - resize.y) / resize.row);
  resize.w.w = Math.min(COLS - (resize.w.x ?? 1) + 1, Math.max(1, resize.startW + dw));
  resize.w.h = Math.min(ROWS - (resize.w.y ?? 1) + 1, Math.max(1, resize.startH + dh));
}
function endResize() {
  window.removeEventListener("mousemove", onResize);
  window.removeEventListener("mouseup", endResize);
  resize = null;
  board.persist();
}

// View-mode click. A button widget fires its event on the bus; other widgets
// jump to their related section. (Edit mode swallows clicks — you're arranging.)
function clickWidget(w) {
  if (editing.value) return;
  if (isButton(w.type)) return fireButton(w);
  const src = w.config.source;
  if (src === "profiles") {
    if (profilesStore.list.length) open(`profile:${profilesStore.list[0].id}`);
  } else if (src === "widgets" || src === "webview") {
    open("webview");
  } else {
    open("automations");
  }
}

// Emit `button.clicked` (context = this button's id + label) and pulse. Bind an
// automation to it from a Profile, or via the widget's Edit-mode "Bind" tool.
function fireButton(w) {
  ipc.emitTrigger("button.clicked", { id: w.id, label: w.config?.label || "" });
  firedId.value = w.id;
  setTimeout(() => {
    if (firedId.value === w.id) firedId.value = null;
  }, 450);
}

// The dedicated profile holding a button's automation (one per button widget).
// Its single `button.clicked` trigger is filtered to this widget's id.
const buttonProfileId = (w) => `button-${w.id}`;
async function bindAction(w) {
  const id = buttonProfileId(w);
  if (!profilesStore.byId(id)) {
    await profilesStore.save({
      id,
      name: t("core.buttons.actionProfile", { label: w.config?.label || w.id }),
      triggers: [
        {
          trigger: "button.clicked",
          filter: `id == "${w.id}"`,
          sequence: { kind: "block", body: [] },
        },
      ],
    });
  }
  open(`profile:${id}`);
}
const isBound = (w) => !!profilesStore.byId(buttonProfileId(w));

// Remove a widget, dropping a button's bound automation too (no orphans).
async function removeWidget(w) {
  if (isButton(w.type) && profilesStore.byId(buttonProfileId(w))) {
    await profilesStore.remove(buttonProfileId(w));
  }
  board.remove(w.id);
}
</script>

<template>
  <div class="dash">
    <header class="dash__head">
      <h1>{{ t("core.dashboard.title") }}</h1>
      <div class="spacer" />
      <Button v-if="editing" size="small" text @click="openPicker">
        <i class="pi pi-plus" /> {{ t("core.dashboard.addWidget") }}
      </Button>
      <Popover ref="pickerPop">
        <div class="wpick">
          <div class="wpick__search">
            <i class="pi pi-search" />
            <input v-model="query" placeholder="Search widgets…" @keydown.stop />
          </div>
          <button v-for="w in filteredCatalog" :key="w.id" class="wpick__item" @click="pick(w)">
            <i :class="w.icon || 'pi pi-box'" :style="{ color: w.color || '#7ec27e' }" />
            <span class="wpick__text">
              <span class="wpick__name">{{ widgetLabel(w.id) }}</span>
              <span v-if="widgetDesc(w.id)" class="wpick__desc">{{ widgetDesc(w.id) }}</span>
            </span>
          </button>
          <div v-if="!filteredCatalog.length" class="wpick__empty">No match</div>
        </div>
      </Popover>
      <Button size="small" :severity="editing ? 'success' : 'secondary'" @click="editing = !editing">
        <i :class="editing ? 'pi pi-check' : 'pi pi-pencil'" />
        {{ editing ? t("core.dashboard.done") : t("core.dashboard.edit") }}
      </Button>
    </header>

    <div class="grid" :class="{ editing }">
      <div v-for="w in board.widgets" :key="w.id" class="cell" :style="cellStyle(w)">
        <div class="card" :class="{ editing, clickable: !editing }">
          <div v-if="editing" class="wtools">
            <i
              class="drag pi pi-arrows-alt"
              :title="t('core.dashboard.widget')"
              @mousedown.stop.prevent="startMove($event, w)"
            />
            <span class="wname">{{ widgetLabel(w.type) }}</span>
            <Button
              v-if="isButton(w.type)"
              size="small"
              text
              :severity="isBound(w) ? 'success' : 'primary'"
              :title="isBound(w) ? t('core.buttons.edit') : t('core.buttons.bind')"
              @click="bindAction(w)"
            >
              <i :class="isBound(w) ? 'pi pi-pencil' : 'pi pi-bolt'" />
            </Button>
            <Button
              v-if="hasSettings(w.type)"
              size="small"
              text
              :title="t('core.dashboard.settings')"
              @click="openSettings(w)"
            >
              <i class="pi pi-cog" />
            </Button>
            <Button size="small" severity="danger" text @click="removeWidget(w)">
              <i class="pi pi-trash" />
            </Button>
          </div>
          <div class="card__body">
            <WidgetFrame
              v-if="isRuntime(w.type)"
              :widget-id="w.type"
              :html="def(w.type).html"
              :config="w.config"
            />
            <component v-else :is="rendererFor(w.type)" v-bind="widgetProps(w)" @click="clickWidget(w)" />
          </div>
          <!-- drag-to-resize handle (snaps to whole cells) -->
          <div
            v-if="editing"
            class="resize"
            :title="`${w.w ?? 1}×${w.h ?? 1}`"
            @mousedown.stop.prevent="startResize($event, w)"
          />
        </div>
      </div>
    </div>

    <p v-if="!board.widgets.length" class="empty">{{ t("core.dashboard.empty") }}</p>

    <!-- Per-widget settings, opened by the widget's cog button. -->
    <Dialog
      :visible="!!settingsW"
      modal
      dismissableMask
      :header="settingsW ? widgetLabel(settingsW.type) : ''"
      :style="{ width: '22rem' }"
      @update:visible="(v) => (settingsW = v ? settingsW : null)"
    >
      <WidgetConfigForm
        v-if="settingsW"
        :properties="widgetProperties(settingsW.type)"
        :modelValue="settingsW.config"
        :label="(k) => propLabel(settingsW.type, k)"
        :options="(k, s) => propOptions(settingsW.type, k, s)"
        @update:modelValue="updateSettings"
      />
    </Dialog>
  </div>
</template>

<style scoped>
.dash {
  box-sizing: border-box; /* keep padding inside the 100% height */
  padding: 1.5rem 2rem;
  height: 100%;
  display: flex;
  flex-direction: column;
}
.dash__head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 1.25rem;
  flex: 0 0 auto;
}
.dash__head h1 {
  margin: 0;
  font-size: 1.5rem;
}
.spacer {
  flex: 1;
}
.add-label {
  font-size: 0.8rem;
  opacity: 0.55;
}
.wpick {
  width: 16rem;
}
.wpick__search {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.5rem;
  border-bottom: 1px solid #333;
  color: #888;
}
.wpick__search input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #eee;
  font-size: 0.85rem;
}
.wpick__item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.6rem;
  background: transparent;
  border: none;
  color: #eee;
  cursor: pointer;
  padding: 0.4rem 0.5rem;
  border-radius: 5px;
  text-align: left;
}
.wpick__item:hover {
  background: #2c2c2c;
}
.wpick__item i {
  font-size: 1.05rem;
}
.wpick__text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.wpick__name {
  font-size: 0.85rem;
}
.wpick__desc {
  font-size: 0.72rem;
  opacity: 0.6;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.wpick__empty {
  padding: 0.6rem;
  opacity: 0.5;
  font-size: 0.8rem;
  text-align: center;
}
.grid {
  flex: 1;
  min-height: 0; /* fill the remaining height below the header */
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  grid-template-rows: repeat(5, minmax(0, 1fr)); /* rows share the full height */
  gap: 1rem; /* keep in sync with GAP */
}
/* In edit mode, outline the cells so free placement has visible guides. */
.grid.editing {
  background-image: linear-gradient(#2c2c2c 1px, transparent 1px),
    linear-gradient(90deg, #2c2c2c 1px, transparent 1px);
  background-size: calc((100% + 1rem) / 5) calc((100% + 1rem) / 5);
  border-radius: 10px;
  outline: 1px dashed #2c2c2c;
  outline-offset: 6px;
}
.cell {
  min-width: 0;
  min-height: 0;
}
.card {
  position: relative;
  height: 100%;
  background: #1e1e1e;
  border: 1px solid #2c2c2c;
  border-radius: 10px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  transition: border-color 0.15s, transform 0.1s;
}
.card__body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.card.clickable {
  cursor: pointer;
}
.card.clickable:hover {
  border-color: #7ec27e;
  transform: translateY(-2px);
}
.card.editing {
  border-style: dashed;
  border-color: #3a3a3a;
}
.wtools {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.35rem 0.4rem;
  background: #191919;
  border-bottom: 1px solid #2a2a2a;
}
.drag {
  cursor: move;
  opacity: 0.6;
  padding: 0 0.2rem;
}
.drag:hover {
  opacity: 1;
}
.wname {
  flex: 1;
  min-width: 0;
  font-size: 0.82rem;
  opacity: 0.85;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* drag-to-resize handle at the card's bottom-right corner */
.resize {
  position: absolute;
  right: 0;
  bottom: 0;
  width: 18px;
  height: 18px;
  cursor: nwse-resize;
  z-index: 3;
  background: linear-gradient(135deg, transparent 50%, #7ec27e 50%);
  border-bottom-right-radius: 9px;
  opacity: 0.75;
}
.resize:hover {
  opacity: 1;
}
.empty {
  opacity: 0.55;
  padding: 2rem 0;
  text-align: center;
}
</style>
