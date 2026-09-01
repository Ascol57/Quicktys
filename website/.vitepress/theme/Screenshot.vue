<script setup>
import { ref } from "vue";
import { withBase } from "vitepress";

const props = defineProps({
  /** Path under `public/`, e.g. `/screens/dashboard.png`. */
  src: { type: String, required: true },
  /** Caption shown under the frame — and inside the placeholder. */
  caption: { type: String, default: "" },
  /** Alt text; falls back to the caption. */
  alt: { type: String, default: "" },
  /** Rough aspect ratio used to size the placeholder. */
  ratio: { type: String, default: "16 / 9" },
});

// The shot list is aspirational: a page may reference a capture that has not
// been taken yet. Rather than a broken image, show a labelled frame so the page
// still reads correctly.
const missing = ref(false);
</script>

<template>
  <figure class="qt-shot">
    <div v-if="missing" class="qt-shot-placeholder" :style="{ aspectRatio: ratio }">
      <span class="qt-shot-icon">▣</span>
      <span class="qt-shot-pending">{{ caption || "Screenshot" }}</span>
      <code>{{ src }}</code>
    </div>
    <img
      v-else
      :src="withBase(src)"
      :alt="alt || caption"
      loading="lazy"
      @error="missing = true"
    />
    <figcaption v-if="caption && !missing">{{ caption }}</figcaption>
  </figure>
</template>

<style scoped>
.qt-shot {
  margin: 24px 0;
}
/* Cap the height so a tall capture (a full window, a sidebar) cannot push the
   surrounding prose off the screen. Width follows, so the aspect is kept. */
.qt-shot img {
  display: block;
  width: auto;
  max-width: 100%;
  max-height: 560px;
  margin: 0 auto;
  border-radius: 12px;
  border: 1px solid var(--vp-c-divider);
  box-shadow: 0 12px 32px -18px rgb(0 0 0 / 0.55);
}
@media (max-width: 639px) {
  .qt-shot img {
    max-height: 380px;
  }
}
.qt-shot figcaption {
  margin-top: 8px;
  font-size: 13px;
  color: var(--vp-c-text-2);
  text-align: center;
}
.qt-shot-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  max-height: 560px;
  border-radius: 12px;
  border: 1px dashed var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
  color: var(--vp-c-text-3);
  padding: 24px;
  text-align: center;
}
.qt-shot-icon {
  font-size: 28px;
  line-height: 1;
  color: var(--vp-c-brand-1);
  opacity: 0.5;
}
.qt-shot-pending {
  font-size: 14px;
  color: var(--vp-c-text-2);
}
.qt-shot-placeholder code {
  font-size: 12px;
  background: transparent;
  color: var(--vp-c-text-3);
}
</style>
