<!-- Startup loading screen: shown while the host compiles + loads extension WASM
     one after another (BootProgress events). Hides on BootReady. -->
<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps({
  // { index, total, current } from boot_status / BootProgress events.
  info: { type: Object, default: () => ({ index: 0, total: 0, current: "" }) },
});
const { t, te } = useI18n();

const label = (k, fallback) => (te(k) ? t(k) : fallback);
const pct = computed(() => {
  const total = props.info.total || 0;
  return total ? Math.min(100, Math.round((props.info.index / total) * 100)) : 0;
});
</script>

<template>
  <div class="boot">
    <div class="boot__box">
      <div class="boot__logo">Quicktys</div>
      <div class="boot__spinner" />
      <div class="boot__msg">
        {{ label("core.boot.loading", "Loading extensions…") }}
      </div>
      <div class="boot__bar">
        <div class="boot__fill" :style="{ width: pct + '%' }" />
      </div>
      <div class="boot__detail">
        <span v-if="info.total" class="boot__count">{{ Math.min(info.index + 1, info.total) }}/{{ info.total }}</span>
        <span v-if="info.current" class="boot__name">{{ info.current }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.boot {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #101014;
  color: #e6e6e6;
}
.boot__box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  width: min(90vw, 320px);
}
.boot__logo {
  font-size: 1.6rem;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: #7ec27e;
}
.boot__spinner {
  width: 34px;
  height: 34px;
  border: 3px solid #2a2a2a;
  border-top-color: #7ec27e;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
.boot__msg {
  opacity: 0.75;
  font-size: 0.95rem;
}
.boot__bar {
  width: 100%;
  height: 6px;
  background: #232323;
  border-radius: 4px;
  overflow: hidden;
}
.boot__fill {
  height: 100%;
  background: #7ec27e;
  transition: width 0.25s ease;
}
.boot__detail {
  display: flex;
  gap: 0.5rem;
  align-items: baseline;
  font-size: 0.82rem;
  height: 1.1rem;
}
.boot__count {
  color: #7ec27e;
  font-variant-numeric: tabular-nums;
}
.boot__name {
  opacity: 0.7;
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 14rem;
}
</style>
