<!-- Schema-driven inspector form (CastMate-style: floating labels, sliders,
     template `{}` toggles). Generates fields from a `quicktys-schema` Object. -->
<script setup>
import { computed, inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import IftaLabel from "primevue/iftalabel";
import InputText from "primevue/inputtext";
import InputNumber from "primevue/inputnumber";
import Select from "primevue/select";
import Slider from "primevue/slider";
import ToggleSwitch from "primevue/toggleswitch";
import TemplateInput from "./TemplateInput.vue";
import { buildVariables } from "../variables.js";

const props = defineProps({
  schema: { type: Object, required: true },
  modelValue: { type: Object, default: () => ({}) },
});
const emit = defineEmits(["update:modelValue"]);
const { t, te } = useI18n();

const plugins = inject("plugins", ref([]));
const variables = computed(() => buildVariables(plugins.value, t, te));

// A string field can declare `suggest: "<source>"`; the app provides the list of
// suggestions per source (e.g. "alert" → existing alert widget names). Rendered
// as a free-text input with a native <datalist> of those names.
const fieldSuggestions = inject("field-suggestions", () => []);

const fields = computed(() =>
  Object.entries(props.schema?.properties ?? {}).map(([key, schema]) => ({ key, schema })),
);

const get = (key) => props.modelValue?.[key];
const set = (key, value) => emit("update:modelValue", { ...props.modelValue, [key]: value });

function fieldLabel(field) {
  const key = field.schema?.name;
  if (key && te(key)) return t(key);
  const base = key ? key.split(".").pop() : field.key;
  return base.charAt(0).toUpperCase() + base.slice(1);
}
const ranged = (s) =>
  ["Number", "Duration"].includes(s.type) && s.min != null && s.max != null;
const isEnum = (s) => s.type === "String" && Array.isArray(s.enum);
const isToggle = (s) => ["Boolean", "Toggle"].includes(s.type);
</script>

<template>
  <div class="form">
    <div v-for="field in fields" :key="field.key" class="field">
      <!-- Boolean / Toggle -->
      <div v-if="isToggle(field.schema)" class="toggle">
        <span class="toggle__label">{{ fieldLabel(field) }}</span>
        <ToggleSwitch :modelValue="!!get(field.key)" @update:modelValue="set(field.key, $event)" />
      </div>

      <!-- Ranged number → slider with value -->
      <div v-else-if="ranged(field.schema)" class="ifta">
        <label>{{ fieldLabel(field) }}</label>
        <div class="slider">
          <span class="slider__value">{{ get(field.key) ?? field.schema.min }}</span>
          <Slider
            :modelValue="get(field.key) ?? field.schema.min"
            :min="field.schema.min"
            :max="field.schema.max"
            @update:modelValue="set(field.key, $event)"
          />
        </div>
      </div>

      <!-- Enum → Select -->
      <IftaLabel v-else-if="isEnum(field.schema)">
        <Select
          :id="`f-${field.key}`"
          :modelValue="get(field.key)"
          :options="field.schema.enum"
          fluid
          @update:modelValue="set(field.key, $event)"
        />
        <label :for="`f-${field.key}`">{{ fieldLabel(field) }}</label>
      </IftaLabel>

      <!-- String with a suggest source → free text + native autocomplete list -->
      <IftaLabel v-else-if="field.schema.suggest">
        <input
          :id="`f-${field.key}`"
          class="p-inputtext p-component sg"
          :list="`sg-${field.key}`"
          :value="get(field.key) ?? ''"
          @input="set(field.key, $event.target.value)"
        />
        <datalist :id="`sg-${field.key}`">
          <option v-for="n in fieldSuggestions(field.schema.suggest)" :key="n" :value="n" />
        </datalist>
        <label :for="`f-${field.key}`">{{ fieldLabel(field) }}</label>
      </IftaLabel>

      <!-- Number / Duration → number input -->
      <IftaLabel v-else-if="['Number', 'Duration'].includes(field.schema.type)">
        <InputNumber
          :id="`f-${field.key}`"
          :modelValue="get(field.key)"
          fluid
          @update:modelValue="set(field.key, $event)"
        />
        <label :for="`f-${field.key}`">{{ fieldLabel(field) }}</label>
      </IftaLabel>

      <!-- Template string → highlighted @-variable input with autocomplete -->
      <div v-else-if="field.schema.template" class="tpl-field">
        <label class="tpl-label">{{ fieldLabel(field) }}</label>
        <TemplateInput
          :modelValue="get(field.key) ?? ''"
          :variables="variables"
          :placeholder="`Text, @variable or {{ }}`"
          @update:modelValue="set(field.key, $event)"
        />
      </div>

      <!-- Plain String / Color / FilePath → text -->
      <IftaLabel v-else>
        <InputText
          :id="`f-${field.key}`"
          :modelValue="get(field.key) ?? ''"
          fluid
          @update:modelValue="set(field.key, $event)"
        />
        <label :for="`f-${field.key}`">{{ fieldLabel(field) }}</label>
      </IftaLabel>
    </div>
  </div>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.field :deep(.p-inputtext),
.field :deep(.p-select),
.field :deep(.p-inputnumber-input) {
  width: 100%;
}
.tpl-field {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
.tpl-label {
  font-size: 0.72rem;
  opacity: 0.7;
  padding-left: 0.1rem;
}
.ifta {
  position: relative;
  border: 1px solid var(--p-inputtext-border-color, #3a3a3a);
  border-radius: var(--p-border-radius, 6px);
  padding: 1.25rem 0.75rem 0.5rem;
}
.ifta > label {
  position: absolute;
  top: 0.3rem;
  left: 0.75rem;
  font-size: 0.7rem;
  opacity: 0.7;
}
.slider {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.slider__value {
  font-size: 1rem;
}
.toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid var(--p-inputtext-border-color, #3a3a3a);
  border-radius: var(--p-border-radius, 6px);
  padding: 0.6rem 0.75rem;
}
.toggle__label {
  font-size: 0.85rem;
  opacity: 0.85;
}
</style>
