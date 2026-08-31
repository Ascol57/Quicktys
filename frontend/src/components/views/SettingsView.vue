<!-- Settings. Language switching is fully functional: it loads the locale bundle
     from the host, flips vue-i18n, and tells the host to persist it. -->
<script setup>
import { inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import Select from "primevue/select";
import InputText from "primevue/inputtext";
import InputNumber from "primevue/inputnumber";
import Password from "primevue/password";
import Button from "primevue/button";
import { useToast } from "primevue/usetoast";
import { useSettings } from "../../stores/settings.js";

const { t } = useI18n();
const toast = useToast();

const locales = inject("locales", ref([]));
const locale = inject("locale", ref("en"));
const changeLocale = inject("change-locale", async () => {});

// Human-readable locale names; fall back to the raw code.
const NAMES = { en: "English", fr: "Français", de: "Deutsch", es: "Español", pt: "Português" };
const localeOptions = () => locales.value.map((code) => ({ code, label: NAMES[code] ?? code }));

const model = ref(locale.value);
async function onChange() {
  await changeLocale(model.value);
}

// --- Pear player endpoint (host/port) --------------------------------------
const settings = useSettings();
const pearHost = ref(settings.pearHost);
const pearPort = ref(settings.pearPort);
async function savePear() {
  await settings.setPear(pearHost.value, pearPort.value);
  toast.add({ severity: "success", detail: t("core.settings.saved"), life: 2000 });
}

// --- OBS connection (host/port/password) -----------------------------------
const obsHost = ref(settings.obsHost);
const obsPort = ref(settings.obsPort);
const obsPassword = ref(settings.obsPassword);
async function saveObs() {
  await settings.setObs(obsHost.value, obsPort.value, obsPassword.value);
  toast.add({ severity: "success", detail: t("core.settings.saved"), life: 2000 });
}

const version = "0.1.0 · M3";
</script>

<template>
  <div class="settings">
    <h2>{{ t("core.settings.title") }}</h2>

    <section class="group">
      <h3>{{ t("core.settings.language") }}</h3>
      <Select
        v-model="model"
        :options="localeOptions()"
        optionLabel="label"
        optionValue="code"
        class="ctl"
        @change="onChange"
      />
      <small class="muted">{{ t("core.settings.languageHint") }}</small>
    </section>

    <section class="group">
      <h3>{{ t("core.settings.pear") }}</h3>
      <div class="row">
        <label class="field host">
          <span class="field__label">{{ t("core.settings.host") }}</span>
          <InputText v-model="pearHost" placeholder="127.0.0.1" />
        </label>
        <label class="field port">
          <span class="field__label">{{ t("core.settings.port") }}</span>
          <InputNumber v-model="pearPort" :useGrouping="false" :min="1" :max="65535" />
        </label>
        <Button class="apply" @click="savePear">{{ t("core.settings.apply") }}</Button>
      </div>
      <small class="muted">{{ t("core.settings.pearHint") }}</small>
    </section>

    <section class="group">
      <h3>{{ t("core.settings.obs") }}</h3>
      <div class="row">
        <label class="field host">
          <span class="field__label">{{ t("core.settings.host") }}</span>
          <InputText v-model="obsHost" placeholder="127.0.0.1" />
        </label>
        <label class="field port">
          <span class="field__label">{{ t("core.settings.port") }}</span>
          <InputNumber v-model="obsPort" :useGrouping="false" :min="1" :max="65535" />
        </label>
      </div>
      <div class="row">
        <label class="field host">
          <span class="field__label">{{ t("core.settings.password") }}</span>
          <Password v-model="obsPassword" :feedback="false" toggleMask fluid />
        </label>
        <Button class="apply" @click="saveObs">{{ t("core.settings.apply") }}</Button>
      </div>
      <small class="muted">{{ t("core.settings.obsHint") }}</small>
    </section>

    <section class="group">
      <h3>{{ t("core.settings.appearance") }}</h3>
      <Select :options="['Dark']" modelValue="Dark" disabled class="ctl" />
      <small class="muted">{{ t("core.settings.theme") }}</small>
    </section>

    <section class="group">
      <h3>{{ t("core.settings.about") }}</h3>
      <p class="muted">{{ t("core.settings.version") }} — {{ version }}</p>
    </section>
  </div>
</template>

<style scoped>
.settings {
  padding: 1.75rem 2rem;
  display: flex;
  flex-direction: column;
  gap: 1.75rem;
  max-width: 560px;
}
.settings h2 {
  margin: 0;
}
.group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding-bottom: 1.25rem;
  border-bottom: 1px solid #242424;
}
.group h3 {
  margin: 0;
  font-size: 1rem;
}
.ctl {
  max-width: 240px;
}
.row {
  display: flex;
  align-items: flex-end;
  gap: 0.75rem;
  flex-wrap: wrap;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  min-width: 0; /* let inputs shrink instead of overflowing onto the button */
}
.field__label {
  font-size: 0.82rem;
  opacity: 0.75;
}
.field.host {
  flex: 1;
  min-width: 160px;
}
.field.port {
  flex: 0 0 auto;
  width: 110px;
}
/* Make every control fill its field so nothing overflows the flex cell. */
.field :deep(.p-inputtext),
.field :deep(.p-inputnumber),
.field :deep(.p-inputnumber input) {
  width: 100%;
}
.apply {
  flex: 0 0 auto;
}
.muted {
  opacity: 0.55;
  font-size: 0.85rem;
}
</style>
