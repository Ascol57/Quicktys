<!-- Settings. Language switching is fully functional: it loads the locale bundle
     from the host, flips vue-i18n, and tells the host to persist it. -->
<script setup>
import { inject, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Select from "primevue/select";
import InputText from "primevue/inputtext";
import InputNumber from "primevue/inputnumber";
import Password from "primevue/password";
import Button from "primevue/button";
import { useToast } from "primevue/usetoast";
import { useSettings } from "../../stores/settings.js";
import { ipc } from "../../ipc.js";

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

// --- Twitch account (OAuth Device Code) ------------------------------------
// The core runs the flow; the `twitch` module supplies the endpoints, client id
// and scopes. We show the code, open the page, and poll at the interval Twitch
// asks for until it says authorized.
const twitch = ref({ connected: false, clientId: "", hasToken: false });
const twitchCode = ref(null); // { user_code, verification_uri, expires_in }
const twitchBusy = ref(false);
const twitchError = ref("");
let pollTimer = null;

async function refreshTwitch() {
  const status = (await ipc.oauthStatus("twitch")) ?? {};
  const mod = (await ipc.callModuleSafe("twitch", "auth-status", {})) ?? {};
  twitch.value = {
    connected: !!status.connected || !!mod.hasToken,
    pending: !!status.pending,
    clientId: mod.clientId ?? "",
  };
  twitchClientId.value = twitch.value.clientId;
}
const twitchClientId = ref("");

async function saveTwitchClientId() {
  await ipc.callModule("twitch", "set-client-id", { client_id: twitchClientId.value.trim() });
  await refreshTwitch();
  toast.add({ severity: "success", detail: t("core.settings.saved"), life: 2000 });
}

function stopPolling() {
  clearInterval(pollTimer);
  pollTimer = null;
}

async function connectTwitch() {
  twitchError.value = "";
  twitchBusy.value = true;
  try {
    const flow = await ipc.oauthConnect("twitch");
    twitchCode.value = flow;
    // Twitch's verification_uri already carries the code, so the page is
    // pre-filled; opening it is a convenience, not a requirement — the user can
    // approve from a phone with the code alone.
    window.open(flow.verification_uri, "_blank");
    stopPolling();
    pollTimer = setInterval(pollTwitch, Math.max(1, Number(flow.interval) || 5) * 1000);
  } catch (e) {
    twitchError.value = String(e?.message ?? e);
  } finally {
    twitchBusy.value = false;
  }
}

async function pollTwitch() {
  try {
    const result = await ipc.oauthPoll("twitch");
    if (result === "pending") return;
    stopPolling();
    twitchCode.value = null;
    if (result === "authorized") {
      toast.add({ severity: "success", detail: t("core.settings.twitchConnected"), life: 3000 });
      await refreshTwitch();
    } else {
      twitchError.value = String(result);
    }
  } catch (e) {
    stopPolling();
    twitchCode.value = null;
    twitchError.value = String(e?.message ?? e);
  }
}

async function disconnectTwitch() {
  stopPolling();
  twitchCode.value = null;
  await ipc.oauthDisconnect("twitch");
  await refreshTwitch();
}

onMounted(refreshTwitch);
onUnmounted(stopPolling);

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
      <h3>{{ t("core.settings.twitch") }}</h3>

      <!-- Connected: nothing to do but disconnect. -->
      <template v-if="twitch.connected">
        <div class="row">
          <span class="badge badge--ok"><i class="pi pi-check-circle" /> {{ t("core.settings.twitchConnected") }}</span>
          <Button class="apply" severity="secondary" @click="disconnectTwitch">
            {{ t("core.settings.twitchDisconnect") }}
          </Button>
        </div>
      </template>

      <!-- Awaiting approval: the code is the whole UI. -->
      <template v-else-if="twitchCode">
        <p class="muted">{{ t("core.settings.twitchEnterCode") }}</p>
        <div class="row">
          <code class="usercode">{{ twitchCode.user_code }}</code>
          <a class="p-button p-button-secondary apply" :href="twitchCode.verification_uri" target="_blank">
            {{ t("core.settings.twitchOpenPage") }}
          </a>
          <Button class="apply" severity="secondary" text @click="disconnectTwitch">
            {{ t("core.settings.twitchCancel") }}
          </Button>
        </div>
        <small class="muted">{{ t("core.settings.twitchWaiting") }}</small>
      </template>

      <!-- Not connected: the client id is the only prerequisite. -->
      <template v-else>
        <div class="row">
          <label class="field host">
            <span class="field__label">{{ t("core.settings.twitchClientId") }}</span>
            <InputText v-model="twitchClientId" placeholder="abcdefg1234567" />
          </label>
          <Button class="apply" severity="secondary" @click="saveTwitchClientId">
            {{ t("core.settings.apply") }}
          </Button>
        </div>
        <div class="row">
          <Button class="apply" :disabled="!twitchClientId.trim() || twitchBusy" @click="connectTwitch">
            <i class="pi pi-twitch" />&nbsp;{{ t("core.settings.twitchConnect") }}
          </Button>
        </div>
      </template>

      <small v-if="twitchError" class="err">{{ twitchError }}</small>
      <small class="muted">{{ t("core.settings.twitchHint") }}</small>
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
.err {
  color: #e5827a;
  font-size: 0.85rem;
}
.badge {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.9rem;
}
.badge--ok {
  color: #7ec27e;
}
/* The device code is the one thing the user must read and retype. */
.usercode {
  font-family: ui-monospace, Menlo, Consolas, monospace;
  font-size: 1.6rem;
  font-weight: 700;
  letter-spacing: 0.18em;
  padding: 0.35rem 0.75rem;
  border: 1px solid #333;
  border-radius: 8px;
  background: #1c1c1c;
  user-select: all;
}
</style>
