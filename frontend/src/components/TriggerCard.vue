<!-- The sequence start card (the trigger), with a Test button. -->
<script setup>
defineProps({
  name: { type: String, default: "When triggered" },
  icon: { type: String, default: "pi pi-bolt" },
  running: Boolean,
});
defineEmits(["test"]);
</script>

<template>
  <div class="trigger">
    <div class="trigger__header">
      <i :class="icon" />
      <span class="trigger__title">{{ name }}</span>
    </div>
    <div class="trigger__spacer" />
    <div class="trigger__test" @click.stop="$emit('test')">
      <i :class="running ? 'pi pi-stop' : 'pi pi-play'" />
      {{ running ? "Stop" : "Test" }}
    </div>
  </div>
</template>

<style scoped>
.trigger {
  --c: #9a5fd0;
  --darker: color-mix(in srgb, var(--c) 58%, #000);
  --lighter: color-mix(in srgb, var(--c) 72%, #fff);
  height: 100px;
  min-width: 122px;
  flex: 0 0 auto;
  border-radius: 6px;
  border: 2px solid var(--lighter);
  background: var(--c);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.trigger__header {
  height: 1.5rem;
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0 0.3rem;
  background: var(--darker);
  color: #f4f4f4;
  font-weight: 600;
  font-size: 0.8rem;
}
.trigger__title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.trigger__spacer {
  flex: 1;
}
.trigger__test {
  height: 1.6rem;
  background: var(--darker);
  color: #f4f4f4;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.3rem;
  font-size: 0.8rem;
  cursor: pointer;
}
.trigger__test:hover {
  background: color-mix(in srgb, var(--c) 40%, #000);
}
</style>
