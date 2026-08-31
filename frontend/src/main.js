import { createApp } from "vue";
import { createPinia } from "pinia";
import PrimeVue from "primevue/config";
import ToastService from "primevue/toastservice";
import Aura from "@primevue/themes/aura";
import "primeicons/primeicons.css";

import App from "./App.vue";
import { i18n, loadLocale } from "./i18n.js";

async function boot() {
  // Pull the reference locale before mounting so first paint is translated.
  await loadLocale("en").catch(() => {});

  const app = createApp(App);
  app.use(createPinia());
  app.use(i18n);
  app.use(PrimeVue, {
    theme: { preset: Aura, options: { darkModeSelector: ".dark" } },
  });
  app.use(ToastService);
  app.mount("#app");
}

boot();
