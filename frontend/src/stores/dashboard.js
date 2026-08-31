// The dashboard layout: widgets freely placed on a fixed grid. Each widget has
// an explicit position (`x`, `y`, 1-based grid cell) and size (`w` cols × `h`
// rows), plus its type and config. Persisted to localStorage (per-user UI
// state); could move to a host resource later.
import { defineStore } from "pinia";

const KEY = "qt.dashboard";

// The board is a fixed cols × rows grid; positions are clamped to it.
export const GRID = { cols: 5, rows: 5 };

function newId() {
  return `w-${Math.random().toString(36).slice(2, 8)}`;
}

// Shipped by default so a fresh dashboard isn't empty (positions fit 5×5).
const DEFAULT = [
  { id: newId(), type: "pear-now-playing", x: 1, y: 1, w: 2, h: 1, config: {} },
  { id: newId(), type: "info", x: 3, y: 1, w: 1, h: 1, config: { source: "plugins" } },
  { id: newId(), type: "info", x: 4, y: 1, w: 1, h: 1, config: { source: "profiles" } },
  { id: newId(), type: "pear-queue", x: 1, y: 2, w: 2, h: 3, config: {} },
];

// First free top-left cell that fits a w×h widget, given the already-placed
// ones. Used to auto-place new widgets and migrate positionless layouts.
function firstFree(widgets, w, h) {
  const occ = new Set();
  for (const g of widgets) {
    if (g.x == null || g.y == null) continue;
    for (let dx = 0; dx < (g.w ?? 1); dx++)
      for (let dy = 0; dy < (g.h ?? 1); dy++) occ.add(`${g.x + dx},${g.y + dy}`);
  }
  for (let y = 1; y <= GRID.rows; y++)
    for (let x = 1; x <= GRID.cols - w + 1; x++) {
      let fits = true;
      for (let dx = 0; dx < w && fits; dx++)
        for (let dy = 0; dy < h && fits; dy++) if (occ.has(`${x + dx},${y + dy}`)) fits = false;
      if (fits) return { x, y };
    }
  return { x: 1, y: 1 }; // board full → allow overlap
}

function load() {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY));
    if (Array.isArray(raw?.widgets)) {
      const placed = [];
      for (const w of raw.widgets) {
        // Normalize older layouts (`span` → `w`) and auto-place any widget that
        // predates free positioning (no `x`/`y`).
        const g = { w: w.w ?? w.span ?? 1, h: w.h ?? 1, ...w };
        // Migrate the renamed info source (overlays → webview).
        if (g.config?.source === "overlays") g.config = { ...g.config, source: "webview" };
        if (g.x == null || g.y == null) Object.assign(g, firstFree(placed, g.w, g.h));
        placed.push(g);
      }
      return placed;
    }
  } catch {
    /* fall through to default */
  }
  return JSON.parse(JSON.stringify(DEFAULT));
}

export const useDashboard = defineStore("dashboard", {
  state: () => ({ widgets: load() }),
  actions: {
    persist() {
      localStorage.setItem(KEY, JSON.stringify({ widgets: this.widgets }));
    },
    add(type = "info", config = { source: "plugins" }, w = 1, h = 1) {
      this.widgets.push({ id: newId(), type, ...firstFree(this.widgets, w, h), w, h, config });
      this.persist();
    },
    remove(id) {
      this.widgets = this.widgets.filter((w) => w.id !== id);
      this.persist();
    },
    update(id, patch) {
      const w = this.widgets.find((x) => x.id === id);
      if (w) Object.assign(w, patch);
      this.persist();
    },
    reset() {
      this.widgets = JSON.parse(JSON.stringify(DEFAULT));
      this.persist();
    },
  },
});
