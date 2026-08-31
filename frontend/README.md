# Quicktys frontend (Tauri WebView)

The UI is **written from scratch** (clean-room) on a third-party stack: Vue 3 +
PrimeVue + Pinia, with schema-driven form generation and `vue-i18n`. **No CastMate
code is reused** — see [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §0bis/§1.

Build plan (see [`../docs/ROADMAP.md`](../docs/ROADMAP.md) M3):

1. Scaffold a Vue 3 + PrimeVue app (original code).
2. Implement a thin Tauri client (`@tauri-apps/api` `invoke` / `listen`) and the
   Pinia stores the components consume.
3. Implement schema-driven form generation from the extension **manifests** the
   host sends (from each WASM `guest.init`), the automation editor (hybrid
   timeline + blocks), and the decentralised i18n wiring (per-extension `lang/`).

> Stack note: Vue, PrimeVue, Pinia, vue-i18n are independent third-party
> libraries (their own permissive licenses) — fine to use. What we must NOT do is
> copy CastMate's UI source. M0 placeholder only.
