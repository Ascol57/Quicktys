<!-- Button widget: a big clickable button. Clicking it (in view mode) fires the
     `button.clicked` event on the host bus — bind an automation to it from a
     Profile (or via the widget's "Bind action" tool in Edit mode). The click →
     emit is wired by the dashboard; this component is purely presentational and
     flashes a short "fired" pulse when told to. -->
<script setup>
// `fired` is driven by the dashboard right after it emits the event, for a short
// visual pulse.
defineProps({
  label: { type: String, default: "Button" },
  icon: { type: String, default: "" },
  color: { type: String, default: "#7c5cff" },
  fired: { type: Boolean, default: false },
});
</script>

<template>
  <button class="btnw" :class="{ fired }" :style="{ '--c': color }">
    <i v-if="icon" :class="icon" class="btnw__icon" />
    <span class="btnw__label">{{ label }}</span>
  </button>
</template>

<style scoped>
.btnw {
  height: 100%;
  width: 100%;
  border: none;
  border-radius: 12px;
  background: var(--c);
  color: #fff;
  font-weight: 600;
  font-size: 1rem;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  transition: transform 0.06s, filter 0.15s, box-shadow 0.2s;
  box-shadow: 0 0 0 0 rgba(0, 0, 0, 0);
}
.btnw:active {
  transform: scale(0.96);
  filter: brightness(1.15);
}
.btnw.fired {
  box-shadow: 0 0 0 3px #38d39f inset;
}
.btnw__icon {
  font-size: 1.5rem;
}
</style>
