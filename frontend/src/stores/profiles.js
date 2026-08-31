// Shared profiles state: the sidebar folder lists them, profile tabs edit them.
import { defineStore } from "pinia";
import { ipc } from "../ipc.js";

export const useProfiles = defineStore("profiles", {
  state: () => ({ list: [] }),
  getters: {
    byId: (s) => (id) => s.list.find((p) => p.id === id) ?? null,
    name: (s) => (id) => s.list.find((p) => p.id === id)?.name ?? id,
  },
  actions: {
    async load() {
      this.list = await ipc.listResources("profile");
    },
    async save(profile) {
      await ipc.upsertResource("profile", profile);
      await this.load();
    },
    async remove(id) {
      await ipc.deleteResource("profile", id);
      await this.load();
    },
    // Create a fresh profile, persist it, and return its id.
    async create() {
      const n = this.list.length + 1;
      let id = `profile-${n}`;
      while (this.byId(id)) id = `profile-${Math.random().toString(36).slice(2, 7)}`;
      await this.save({ id, name: `Profile ${n}`, triggers: [] });
      return id;
    },
  },
});
