// Decentralised i18n (ARCHITECTURE §10): the host discovers every `lang/`
// folder, namespaces the bundles by id, and hands us the merged per-locale
// object. `vue-i18n` then resolves keys like `plugin.time.actions.delay.name`.

import { createI18n } from "vue-i18n";
import { ipc } from "./ipc.js";

export const i18n = createI18n({
  legacy: false,
  locale: "en",
  fallbackLocale: "en",
  messages: {},
});

/** Fetch a locale's merged bundle from the host and install it. */
export async function loadLocale(locale) {
  const bundle = await ipc.localeBundle(locale);
  i18n.global.setLocaleMessage(locale, bundle ?? {});
}

/** Switch the active locale (loading its bundle and telling the host). */
export async function switchLocale(locale) {
  await loadLocale(locale);
  i18n.global.locale.value = locale;
  await ipc.setLocale(locale);
}
