// Factories, metadata and variant helpers for the automation node model
// (mirrors quicktys-schema's `Node`, kind-tagged). Expressions are stored as
// their source string. Each node/track/case carries a transient `_id` for
// stable list keys (drag-and-drop); `stripIds` removes them before persisting.

export const NODE_META = {
  action: { label: "Action", icon: "pi pi-bolt", color: "#8dc1a9", category: "Core" },
  block: { label: "Block", icon: "pi pi-bars", color: "#9aa0a6", category: "Structure" },
  parallel: { label: "Parallel", icon: "pi pi-share-alt", color: "#8da9c1", category: "Structure", keywords: "concurrent simultaneous branches at once" },
  timeline: { label: "Timeline", icon: "pi pi-stopwatch", color: "#c1bd8d", category: "Structure", keywords: "timeline tracks offset schedule time" },
  if: { label: "If", icon: "pi pi-code", color: "#c1a98d", category: "Flow", keywords: "condition branch if else" },
  switch: { label: "Switch", icon: "pi pi-sitemap", color: "#c1a98d", category: "Flow", keywords: "case match switch" },
  loop: { label: "Loop", icon: "pi pi-replay", color: "#a98dc1", category: "Flow", keywords: "repeat while foreach iterate loop" },
  "wait-for": {
    label: "Wait",
    icon: "pi pi-clock",
    color: "#8db7c1",
    category: "Flow",
    keywords: "delay wait sleep pause timer duration",
  },
  try: { label: "Try", icon: "pi pi-shield", color: "#c18d8d", category: "Flow" },
  let: { label: "Let", icon: "pi pi-plus-circle", color: "#b0b07a", category: "Vars" },
  set: { label: "Set", icon: "pi pi-pencil", color: "#b0b07a", category: "Vars" },
  return: { label: "Return", icon: "pi pi-arrow-circle-left", color: "#9aa0a6", category: "Jump" },
  break: { label: "Break", icon: "pi pi-ban", color: "#9aa0a6", category: "Jump" },
  continue: { label: "Continue", icon: "pi pi-forward", color: "#9aa0a6", category: "Jump" },
};

let _uid = 0;
const nid = () => `n${++_uid}`;
const withId = (obj) => {
  obj._id = nid();
  return obj;
};

/** A fresh transient id (for editor-only items like floating sequences). */
export const newId = () => nid();

const emptyBlock = () => withId({ kind: "block", body: [] });

function build(kind) {
  switch (kind) {
    case "action":
      return { kind: "action", plugin: "", action: "", config: {} };
    case "parallel":
      return { kind: "parallel", join: "all", branches: [] };
    case "timeline":
      return { kind: "timeline", tracks: [] };
    case "if":
      return { kind: "if", cond: "true", then: emptyBlock() };
    case "switch":
      return { kind: "switch", value: "x", cases: [] };
    case "loop":
      return { kind: "loop", loop: "repeat", count: "3", body: emptyBlock(), max: 100 };
    case "wait-for":
      return { kind: "wait-for", wait: "delay", ms: "1000" };
    case "try":
      return { kind: "try", body: emptyBlock() };
    case "let":
      return { kind: "let", name: "x", value: "0" };
    case "set":
      return { kind: "set", target: "x", value: "0" };
    case "return":
      return { kind: "return" };
    case "break":
      return { kind: "break" };
    case "continue":
      return { kind: "continue" };
    case "block":
    default:
      return { kind: "block", body: [] };
  }
}

export const newNode = (kind) => withId(build(kind));

/** Build a node from a palette pick: a flow kind, or a plugin action. */
export function nodeFromPick(pick) {
  if (pick.plugin) {
    const n = newNode("action");
    n.plugin = pick.plugin;
    n.action = pick.action;
    return n;
  }
  return newNode(pick.kind);
}
export const newTrack = () => withId({ offset_ms: 0, body: emptyBlock() });
export const newCase = () => withId({ equals: "", body: emptyBlock() });

export const PALETTE = Object.entries(NODE_META).map(([kind, m]) => ({ kind, ...m }));
export const CATEGORIES = ["Core", "Structure", "Flow", "Vars", "Jump"];

export function label(node) {
  if (node.kind === "loop") return `Loop · ${node.loop ?? "repeat"}`;
  if (node.kind === "wait-for") return `Wait · ${node.wait ?? "delay"}`;
  return NODE_META[node.kind]?.label ?? node.kind;
}

export const meta = (node) => NODE_META[node.kind] ?? { icon: "pi pi-circle", color: "#888" };

export function setLoopMode(node, mode) {
  const base = { ...node, kind: "loop", loop: mode, body: node.body ?? emptyBlock(), max: node.max ?? 100 };
  if (mode === "repeat") return { ...base, count: node.count ?? "3", cond: undefined, var: undefined, list: undefined };
  if (mode === "while") return { ...base, cond: node.cond ?? "true", count: undefined, var: undefined, list: undefined };
  return { ...base, var: node.var ?? "item", list: node.list ?? "[1, 2, 3]", count: undefined, cond: undefined };
}

export function setWaitMode(node, mode) {
  const base = { _id: node._id, kind: "wait-for", wait: mode };
  if (mode === "delay") return { ...base, ms: node.ms ?? "1000" };
  if (mode === "until") return { ...base, cond: node.cond ?? "ready", timeout_ms: node.timeout_ms };
  return { ...base, trigger: node.trigger ?? "", timeout_ms: node.timeout_ms };
}

/** Recursively assign transient `_id`s to a loaded tree (in place). */
export function assignIds(value) {
  if (Array.isArray(value)) {
    value.forEach(assignIds);
  } else if (value && typeof value === "object") {
    const isItem = "kind" in value || "offset_ms" in value || "equals" in value;
    if (isItem && !value._id) value._id = nid();
    Object.values(value).forEach(assignIds);
  }
}

/** Find a node anywhere in the tree by its transient `_id`. */
export function findNodeById(value, id) {
  if (!value || typeof value !== "object") return null;
  if (value._id === id && "kind" in value) return value;
  for (const v of Object.values(value)) {
    if (Array.isArray(v)) {
      for (const item of v) {
        const found = findNodeById(item, id);
        if (found) return found;
      }
    } else if (v && typeof v === "object") {
      const found = findNodeById(v, id);
      if (found) return found;
    }
  }
  return null;
}

/** Replace `target`'s contents with `source` in place, preserving `_id` (and
 *  thus the reference the parent list holds). */
export function applyNode(target, source) {
  const id = target._id;
  for (const k of Object.keys(target)) {
    if (k !== "_id" && !(k in source)) delete target[k];
  }
  for (const [k, v] of Object.entries(source)) {
    if (k !== "_id") target[k] = v;
  }
  target._id = id;
}

/** Collect (stripped clones of) nodes whose `_id` is in `ids`, in tree order.
 *  A matched node is taken whole (children included), not descended into. */
export function collectNodes(value, ids, out = []) {
  if (Array.isArray(value)) {
    for (const item of value) {
      if (item && typeof item === "object" && "kind" in item && ids.has(item._id)) {
        out.push(stripIds(item));
      } else {
        collectNodes(item, ids, out);
      }
    }
  } else if (value && typeof value === "object") {
    for (const v of Object.values(value)) collectNodes(v, ids, out);
  }
  return out;
}

/** Remove (in place) every array-item node whose `_id` is in `ids`. */
export function removeIds(value, ids) {
  if (Array.isArray(value)) {
    for (let i = value.length - 1; i >= 0; i--) {
      const item = value[i];
      if (item && typeof item === "object" && "kind" in item && ids.has(item._id)) {
        value.splice(i, 1);
      } else {
        removeIds(item, ids);
      }
    }
  } else if (value && typeof value === "object") {
    for (const v of Object.values(value)) removeIds(v, ids);
  }
}

/** Deep-clone stripping every `_id` (for persistence). */
export function stripIds(value) {
  if (Array.isArray(value)) return value.map(stripIds);
  if (value && typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) {
      if (k === "_id" || v === undefined) continue;
      out[k] = stripIds(v);
    }
    return out;
  }
  return value;
}
