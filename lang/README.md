# Core language bundles

The **core / app-shell** translations. One JSON file per supported locale
(`en`, `fr`, `de`, `es`, `pt` to start). Loaded directly by the frontend and
merged into `vue-i18n` under the `core.*` namespace.

This is one of three tiers of the **decentralised i18n** model — see
[`../ARCHITECTURE.md`](../ARCHITECTURE.md) §10. Every extension owns its own
`lang/` folder the same way:

- core    → `lang/` (here)            → namespace `core.*`
- modules → `modules/<id>/lang/`      → namespace `module.<id>.*`
- plugins → `plugins/<id>/lang/`      → namespace `plugin.<id>.*`

`en.json` is the reference locale; missing keys in other locales fall back to it.
