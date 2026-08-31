// User settings that aren't per-resource: the Pear player endpoint and the OBS
// connection (host/port/password). Persisted to localStorage and pushed to the
// respective host-side driver modules via their services — applied on boot and
// on edit so the modules target the right endpoints.
import { defineStore } from "pinia";
import { ipc } from "../ipc.js";

const KEY = "qt.settings";
const DEFAULTS = {
  pear: { host: "127.0.0.1", port: 26538 },
  obs: { host: "127.0.0.1", port: 4455, password: "" },
};

function load() {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY));
    if (raw && typeof raw === "object") {
      return {
        pear: { ...DEFAULTS.pear, ...(raw.pear ?? {}) },
        obs: { ...DEFAULTS.obs, ...(raw.obs ?? {}) },
      };
    }
  } catch {
    /* fall through to defaults */
  }
  return JSON.parse(JSON.stringify(DEFAULTS));
}

export const useSettings = defineStore("settings", {
  state: () => ({ data: load() }),
  getters: {
    pearHost: (s) => s.data.pear.host,
    pearPort: (s) => s.data.pear.port,
    pearBaseUrl: (s) => `http://${s.data.pear.host}:${s.data.pear.port}`,
    obsHost: (s) => s.data.obs.host,
    obsPort: (s) => s.data.obs.port,
    obsPassword: (s) => s.data.obs.password,
    obsUrl: (s) => `ws://${s.data.obs.host}:${s.data.obs.port}`,
  },
  actions: {
    persist() {
      localStorage.setItem(KEY, JSON.stringify(this.data));
    },
    // Set the Pear endpoint and push it to the module.
    async setPear(host, port) {
      this.data.pear = { host: (host || "").trim() || "127.0.0.1", port: Number(port) || 26538 };
      this.persist();
      await this.applyPear();
    },
    async applyPear() {
      try {
        await ipc.callModule("pear", "set-base-url", { url: this.pearBaseUrl });
      } catch {
        /* pear module may not be loaded — ignore */
      }
    },
    // Set the OBS endpoint + password and push them to the obs module.
    async setObs(host, port, password) {
      this.data.obs = {
        host: (host || "").trim() || "127.0.0.1",
        port: Number(port) || 4455,
        password: password ?? "",
      };
      this.persist();
      await this.applyObs();
    },
    // Tell the obs module its URL + auth password (before the first poll connects).
    async applyObs() {
      try {
        await ipc.callModule("obs", "set-url", { url: this.obsUrl });
        await ipc.callModule("obs", "set-password", { password: this.obsPassword });
      } catch {
        /* obs module may not be loaded — ignore */
      }
    },
  },
});
