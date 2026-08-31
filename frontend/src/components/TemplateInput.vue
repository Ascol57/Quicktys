<!-- A template text field: `@plugin.var` variables are highlighted, and typing
     `@` opens an autocomplete of available variables. The stored value is the
     raw template string (`@pear.title`, `{{ }}` Handlebars) resolved by the host. -->
<script setup>
import { computed, nextTick, ref } from "vue";
import { HELPERS } from "../helpers.js";

const props = defineProps({
  modelValue: { type: String, default: "" },
  variables: { type: Array, default: () => [] }, // [{ id, name, plugin, icon, color }]
  placeholder: { type: String, default: "" },
});
const emit = defineEmits(["update:modelValue"]);

const taRef = ref(null);
const highlightRef = ref(null);

// Highlight `@path` variables (start-of-string or after whitespace, like the
// host resolver) and `{{ … }}` Handlebars regions.
// An escape `\x` (consumed first, so `\@`/`\{{` are not tokens); a `@path`
// (start or after whitespace); or a non-escaped `{{ … }}` region.
const TOKEN = /\\[\s\S]|(?<=^|\s)@[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*|(?<!\\)\{\{[^}]*\}\}?/g;
const escapeHtml = (s) =>
  s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);
const highlighted = computed(() => {
  const text = props.modelValue ?? "";
  let html = "";
  let last = 0;
  for (const m of text.matchAll(TOKEN)) {
    const tok = m[0];
    html += escapeHtml(text.slice(last, m.index));
    if (tok[0] === "\\") {
      // Active escape: dim the backslash, keep the escaped char literal.
      html += `<span class="esc">\\</span>${escapeHtml(tok.slice(1))}`;
    } else {
      const cls = tok[0] === "@" ? "var" : "hb";
      html += `<span class="${cls}">${escapeHtml(tok)}</span>`;
    }
    last = m.index + tok.length;
  }
  html += escapeHtml(text.slice(last));
  return html + "​"; // keep trailing newline height
});

function syncScroll() {
  if (highlightRef.value && taRef.value) {
    highlightRef.value.scrollTop = taRef.value.scrollTop;
    highlightRef.value.scrollLeft = taRef.value.scrollLeft;
  }
}

// --- autocomplete: `@` variables, and helpers/variables inside `{{ }}` -------
const menu = ref({ open: false, mode: "var", query: "", from: 0 });
const active = ref(0);

const suggestions = computed(() => {
  if (!menu.value.open) return [];
  const q = menu.value.query.toLowerCase();
  const prefix = menu.value.mode === "var" ? "@" : "";
  const vars = props.variables
    .filter((v) => v.id.toLowerCase().includes(q) || v.name.toLowerCase().includes(q))
    .map((v) => ({
      kind: "var",
      insert: v.id,
      label: prefix + v.id,
      hint: v.plugin,
      icon: v.icon,
      color: v.color,
    }));
  if (menu.value.mode === "var") return vars.slice(0, 8);
  // Inside `{{ }}`: helpers (prefix match) first, then variables.
  const helpers = HELPERS.filter((h) => h.name.toLowerCase().startsWith(q)).map((h) => ({
    kind: "helper",
    insert: h.block ? `#${h.name}` : h.name,
    label: h.sig,
    hint: h.desc,
    icon: "pi pi-bolt",
    color: "#e0a458",
  }));
  return [...helpers, ...vars].slice(0, 10);
});

function onInput(e) {
  emit("update:modelValue", e.target.value);
  updateMenu();
}
function updateMenu() {
  const ta = taRef.value;
  const upto = ta.value.slice(0, ta.selectionStart);
  // `@variable`
  const at = upto.match(/(?:^|\s)@([A-Za-z0-9_.-]*)$/);
  if (at) {
    menu.value = { open: true, mode: "var", query: at[1], from: ta.selectionStart - at[1].length - 1 };
    active.value = 0;
    return;
  }
  // Inside an unclosed, non-escaped `{{ … }}` → helper / variable.
  const open = upto.lastIndexOf("{{");
  if (open !== -1 && open > upto.lastIndexOf("}}") && upto[open - 1] !== "\\") {
    const tail = upto.match(/(#?)([A-Za-z0-9_.]*)$/);
    const word = tail[2];
    menu.value = {
      open: true,
      mode: "hb",
      query: word,
      from: ta.selectionStart - tail[1].length - word.length,
    };
    active.value = 0;
    return;
  }
  menu.value.open = false;
}
function choose(item) {
  const ta = taRef.value;
  const before = ta.value.slice(0, menu.value.from);
  const after = ta.value.slice(ta.selectionStart);
  const insert =
    menu.value.mode === "var"
      ? `@${item.insert} `
      : item.kind === "helper"
        ? `${item.insert} `
        : item.insert;
  emit("update:modelValue", `${before}${insert}${after}`);
  menu.value.open = false;
  nextTick(() => {
    const pos = (before + insert).length;
    ta.focus();
    ta.setSelectionRange(pos, pos);
    syncScroll();
  });
}
function onKeydown(e) {
  if (!menu.value.open || !suggestions.value.length) return;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    active.value = (active.value + 1) % suggestions.value.length;
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    active.value = (active.value - 1 + suggestions.value.length) % suggestions.value.length;
  } else if (e.key === "Enter" || e.key === "Tab") {
    e.preventDefault();
    choose(suggestions.value[active.value]);
  } else if (e.key === "Escape") {
    menu.value.open = false;
  }
}
</script>

<template>
  <div class="tin">
    <div class="tin__box">
      <div ref="highlightRef" class="tin__highlight" v-html="highlighted" />
      <textarea
        ref="taRef"
        class="tin__input"
        rows="1"
        :value="modelValue"
        :placeholder="placeholder"
        spellcheck="false"
        @input="onInput"
        @scroll="syncScroll"
        @keydown="onKeydown"
        @blur="menu.open = false"
        @click="updateMenu"
      />
    </div>

    <ul v-if="menu.open && suggestions.length" class="tin__menu">
      <li
        v-for="(s, i) in suggestions"
        :key="s.kind + s.insert"
        class="tin__opt"
        :class="{ active: i === active }"
        @mousedown.prevent="choose(s)"
      >
        <i :class="s.icon" :style="{ color: s.color }" />
        <span class="tin__id">{{ s.label }}</span>
        <span class="tin__name">{{ s.hint }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.tin {
  position: relative;
}
.tin__box {
  position: relative;
}
/* The highlight layer and the textarea must share identical metrics. */
.tin__highlight,
.tin__input {
  margin: 0;
  padding: 0.5rem 0.6rem;
  font: inherit;
  font-family: ui-monospace, monospace;
  font-size: 0.85rem;
  line-height: 1.4;
  white-space: pre-wrap;
  word-break: break-word;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
}
.tin__highlight {
  position: absolute;
  inset: 0;
  overflow: hidden;
  color: #eee;
  pointer-events: none;
  background: #1c1c1c;
}
.tin__input {
  position: relative;
  width: 100%;
  min-height: 2.4rem;
  resize: vertical;
  background: transparent;
  color: transparent;
  caret-color: #eee;
  outline: none;
}
.tin__input:focus {
  border-color: #7ec27e;
}
.tin__highlight :deep(.var) {
  background: color-mix(in srgb, #7ec27e 30%, transparent);
  border-radius: 3px;
  padding: 0 1px;
  color: #bfe6bf;
  font-weight: 600;
}
.tin__highlight :deep(.hb) {
  background: color-mix(in srgb, #e0a458 26%, transparent);
  border-radius: 3px;
  padding: 0 1px;
  color: #f0c48a;
  font-weight: 600;
}
.tin__highlight :deep(.esc) {
  opacity: 0.35;
}
.tin__menu {
  position: absolute;
  z-index: 30;
  left: 0;
  right: 0;
  margin: 2px 0 0;
  padding: 0.25rem;
  list-style: none;
  background: #1e1e1e;
  border: 1px solid #383838;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  max-height: 15rem;
  overflow-y: auto;
}
.tin__opt {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.35rem 0.5rem;
  border-radius: 5px;
  cursor: pointer;
  font-size: 0.82rem;
}
.tin__opt.active,
.tin__opt:hover {
  background: #2c2c2c;
}
.tin__id {
  font-family: ui-monospace, monospace;
  color: #bfe6bf;
}
.tin__name {
  opacity: 0.6;
  margin-left: auto;
}
</style>
