<!-- Info widget: a box showing a count + label for a chosen source
     (plugins / modules / profiles / automations / …). -->
<script setup>
defineProps({
  icon: { type: String, default: "pi pi-box" },
  color: { type: String, default: "#7ec27e" },
  count: { type: [Number, String], default: 0 },
  label: { type: String, default: "" },
});
</script>

<template>
  <div class="info" :style="{ '--c': color }">
    <i :class="icon" class="info__icon" />
    <span class="info__n">{{ count }}</span>
    <span class="info__label">{{ label }}</span>
  </div>
</template>

<style scoped>
.info {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  align-items: flex-start;
  justify-content: center;
  padding: 1.1rem 1.25rem;
}
.info__icon {
  font-size: 1.3rem;
  color: var(--c);
}
.info__n {
  font-size: 2rem;
  font-weight: 700;
  line-height: 1;
}
.info__label {
  opacity: 0.7;
  font-size: 0.85rem;
}
</style>
