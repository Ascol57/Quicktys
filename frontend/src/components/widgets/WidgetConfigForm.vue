<!-- Renders a widget's config schema as editable fields (used by the dashboard
     settings modal and the overlay editor). Field type is inferred from the
     schema: enum → Select, Boolean → switch, Color → colour picker, Number →
     slider/number, else text. Emits the merged config via v-model. -->
<script setup>
import { computed } from "vue";
import Select from "primevue/select";
import InputNumber from "primevue/inputnumber";
import InputText from "primevue/inputtext";
import Slider from "primevue/slider";
import ToggleSwitch from "primevue/toggleswitch";

const props = defineProps({
  // Schema `properties` object: { key: { type, enum?, default?, min?, max? } }.
  properties: { type: Object, default: () => ({}) },
  modelValue: { type: Object, default: () => ({}) },
  // (key) => display label. Defaults to the raw key.
  label: { type: Function, default: (k) => k },
  // Optional (key, schema) => [{ value, label }] for enum options.
  options: { type: Function, default: null },
});
const emit = defineEmits(["update:modelValue"]);

const fields = computed(() => Object.entries(props.properties).map(([key, schema]) => ({ key, schema })));
const t = (s) => String(s?.type || "").toLowerCase();
function kind(s) {
  if (Array.isArray(s.enum)) return "enum";
  const ty = t(s);
  if (ty === "boolean" || ty === "toggle") return "boolean";
  if (ty === "color") return "color";
  if ((ty === "number" || ty === "integer") && s.min != null && s.max != null) return "range";
  if (ty === "number" || ty === "integer") return "number";
  return "text";
}
const get = (key) => props.modelValue?.[key];
const set = (key, value) => emit("update:modelValue", { ...props.modelValue, [key]: value });
function opts(key, s) {
  if (props.options) return props.options(key, s);
  return (s.enum ?? []).map((v) => ({ value: v, label: String(v) }));
}
</script>

<template>
  <div class="wcf">
    <div v-for="f in fields" :key="f.key" class="row" :class="{ inline: kind(f.schema) === 'boolean' }">
      <label class="lbl">{{ label(f.key) }}</label>

      <!-- Boolean → switch -->
      <ToggleSwitch
        v-if="kind(f.schema) === 'boolean'"
        :modelValue="get(f.key) !== false"
        @update:modelValue="set(f.key, $event)"
      />

      <!-- Enum → select -->
      <Select
        v-else-if="kind(f.schema) === 'enum'"
        :modelValue="get(f.key)"
        :options="opts(f.key, f.schema)"
        optionValue="value"
        optionLabel="label"
        fluid
        @update:modelValue="set(f.key, $event)"
      />

      <!-- Color → swatch + hex -->
      <div v-else-if="kind(f.schema) === 'color'" class="color">
        <input
          type="color"
          :value="get(f.key) ?? f.schema.default ?? '#000000'"
          @input="set(f.key, $event.target.value)"
        />
        <InputText
          :modelValue="get(f.key) ?? f.schema.default ?? ''"
          class="hex"
          @update:modelValue="set(f.key, $event)"
        />
      </div>

      <!-- Ranged number → slider + value -->
      <div v-else-if="kind(f.schema) === 'range'" class="range">
        <Slider
          :modelValue="get(f.key) ?? f.schema.default ?? f.schema.min"
          :min="f.schema.min"
          :max="f.schema.max"
          @update:modelValue="set(f.key, $event)"
        />
        <span class="range__val">{{ get(f.key) ?? f.schema.default ?? f.schema.min }}</span>
      </div>

      <!-- Number → input -->
      <InputNumber
        v-else-if="kind(f.schema) === 'number'"
        :modelValue="get(f.key)"
        :min="f.schema.min"
        :max="f.schema.max"
        fluid
        @update:modelValue="set(f.key, $event)"
      />

      <!-- Fallback → text -->
      <InputText
        v-else
        :modelValue="get(f.key) ?? ''"
        fluid
        @update:modelValue="set(f.key, $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.wcf {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}
.row {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.row.inline {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}
.lbl {
  font-size: 0.82rem;
  opacity: 0.75;
}
.color {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.color input[type="color"] {
  width: 2.2rem;
  height: 2.2rem;
  padding: 0;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  background: none;
  cursor: pointer;
}
.hex {
  flex: 1;
  min-width: 0;
}
.range {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}
.range :deep(.p-slider) {
  flex: 1;
}
.range__val {
  font-variant-numeric: tabular-nums;
  min-width: 2ch;
  text-align: right;
  opacity: 0.8;
}
.row :deep(.p-inputtext),
.row :deep(.p-select),
.row :deep(.p-inputnumber-input) {
  width: 100%;
}
</style>
