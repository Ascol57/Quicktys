//! Quicktys IPC bridge — the boundary between the Rust host and the Vue WebView.
//!
//! Replaces CastMate's Electron `ipcMain.handle`/`ipcRenderer.invoke` with
//! Tauri's `#[tauri::command]` (request/response) + `Window::emit` (broadcast).
//!
//! - **commands** (UI → host): [`AppBackend`] methods — list plugins/resources,
//!   CRUD resources, run an automation, set the locale, fetch i18n bundles, ...
//! - **events** (host → UI): [`AppEvent`], delivered through an [`EventSink`].
//!
//! Deliberately free of `tauri` types, so the whole command surface is
//! unit-testable without a WebView; `quicktys-app` wires thin Tauri shims to
//! these methods and an [`EventSink`] backed by `Window::emit`.

mod backend;
mod event;

pub use backend::{AppBackend, HostLoader, ModuleInfo, PluginInfo};
pub use event::{AppEvent, EventSink, NullSink, ResourceKind};

#[cfg(test)]
mod tests {
    use super::*;
    use quicktys_core::invoker::BoxFut;
    use quicktys_core::{
        ActionInvoker, ActionResult, ExtensionRegistry, LocaleRegistry, Namespace, ResourceStore,
        Runner, StateStore, TriggerBus,
    };
    use quicktys_expr::ExprEngine;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    /// Echoes the action config back as the result.
    struct Echo;
    impl ActionInvoker for Echo {
        fn invoke<'a>(&'a self, _p: &'a str, _a: &'a str, config: Value) -> BoxFut<'a, ActionResult> {
            Box::pin(async move { Ok(config) })
        }
    }

    /// Records the ids of invoked actions.
    struct Recorder(Mutex<Vec<String>>);
    impl ActionInvoker for Recorder {
        fn invoke<'a>(&'a self, _p: &'a str, a: &'a str, _c: Value) -> BoxFut<'a, ActionResult> {
            self.0.lock().unwrap().push(a.to_string());
            Box::pin(async move { Ok(Value::Null) })
        }
    }

    #[derive(Default)]
    struct CollectSink(Mutex<Vec<AppEvent>>);
    impl EventSink for CollectSink {
        fn emit(&self, event: AppEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    impl CollectSink {
        fn events(&self) -> Vec<AppEvent> {
            self.0.lock().unwrap().clone()
        }
    }

    fn temp_dir() -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("quicktys-ipc-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    struct Harness {
        backend: Arc<AppBackend>,
        sink: Arc<CollectSink>,
        state: Arc<StateStore>,
        registry: Arc<Mutex<ExtensionRegistry>>,
    }

    fn harness_with(locales: LocaleRegistry) -> Harness {
        let state = Arc::new(StateStore::new());
        let bus = Arc::new(TriggerBus::new());
        let runner = Runner::new(Arc::new(ExprEngine::new()), Arc::new(Echo));
        let registry = Arc::new(Mutex::new(ExtensionRegistry::new()));
        let sink = Arc::new(CollectSink::default());
        let dir = temp_dir();
        let backend = Arc::new(AppBackend::new(
            state.clone(),
            bus,
            runner,
            registry.clone(),
            locales,
            ResourceStore::new(dir.join("profiles")),
            ResourceStore::new(dir.join("automations")),
            ResourceStore::new(dir.join("queues")),
            Vec::new(),
            sink.clone(),
        ));
        Harness {
            backend,
            sink,
            state,
            registry,
        }
    }

    fn harness() -> Harness {
        harness_with(LocaleRegistry::new())
    }

    #[tokio::test]
    async fn resource_crud_persists_and_emits_events() {
        let h = harness();
        h.backend
            .upsert_resource(ResourceKind::Profile, json!({ "id": "main", "name": "Main" }))
            .unwrap();
        assert_eq!(h.backend.list_resources(ResourceKind::Profile).len(), 1);
        assert!(h.backend.get_resource(ResourceKind::Profile, "main").is_some());
        assert!(matches!(
            h.sink.events().last(),
            Some(AppEvent::ResourceAdded { id, .. }) if id == "main"
        ));

        // Update emits ResourceUpdated.
        h.backend
            .upsert_resource(ResourceKind::Profile, json!({ "id": "main", "name": "Renamed" }))
            .unwrap();
        assert!(matches!(
            h.sink.events().last(),
            Some(AppEvent::ResourceUpdated { .. })
        ));

        // Delete emits ResourceRemoved.
        h.backend.delete_resource(ResourceKind::Profile, "main").unwrap();
        assert!(h.backend.list_resources(ResourceKind::Profile).is_empty());
        assert!(matches!(
            h.sink.events().last(),
            Some(AppEvent::ResourceRemoved { id, .. }) if id == "main"
        ));
    }

    #[tokio::test]
    async fn saved_profile_dispatches_on_matching_trigger() {
        use quicktys_core::{Dispatcher, TriggerEvent};
        let h = harness();
        // A profile saved through the backend, with a trigger whose inline
        // sequence runs one action.
        h.backend
            .upsert_resource(
                ResourceKind::Profile,
                json!({
                    "id": "p1",
                    "name": "P1",
                    "triggers": [{
                        "trigger": "demo.fire",
                        "sequence": { "kind": "action", "plugin": "x", "action": "act", "config": {} }
                    }]
                }),
            )
            .unwrap();

        // A dispatcher built from the SAME shared store as the backend.
        let rec = Arc::new(Recorder(Mutex::new(Vec::new())));
        let runner = Runner::new(Arc::new(ExprEngine::new()), rec.clone());
        let dispatcher = Dispatcher::new(
            runner,
            Arc::new(ExprEngine::new()),
            h.state.clone(),
            h.backend.profiles_store(),
        );

        // A non-matching trigger runs nothing.
        dispatcher
            .dispatch(TriggerEvent::new("other.event", Default::default()))
            .await;
        assert!(rec.0.lock().unwrap().is_empty());

        // The matching trigger runs the profile's inline sequence.
        dispatcher
            .dispatch(TriggerEvent::new("demo.fire", Default::default()))
            .await;
        assert_eq!(rec.0.lock().unwrap().clone(), vec!["act".to_string()]);
    }

    #[tokio::test]
    async fn runs_a_stored_automation() {
        let h = harness();
        h.backend
            .upsert_resource(
                ResourceKind::Automation,
                json!({
                    "id": "greet",
                    "root": { "kind": "action", "plugin": "p", "action": "say", "config": { "m": "hi" } }
                }),
            )
            .unwrap();
        // The action runs (echo invoker); no Return node → no value.
        let out = h.backend.run_automation("greet", json!({})).await.unwrap();
        assert_eq!(out, None);
        assert!(h.backend.run_automation("missing", json!({})).await.is_err());
    }

    #[test]
    fn lists_registered_plugins() {
        let h = harness();
        h.registry.lock().unwrap().register_plugin(
            serde_json::from_value(json!({
                "id": "time", "name": "Time", "version": "1.0.0",
                "actions": [{ "id": "delay" }]
            }))
            .unwrap(),
        );
        let plugins = h.backend.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "time");
        assert_eq!(plugins[0].actions, vec!["delay"]);
    }

    #[test]
    fn exposes_action_config_schema_for_the_editor() {
        let h = harness();
        h.registry.lock().unwrap().register_plugin(
            serde_json::from_value(json!({
                "id": "time", "name": "Time", "version": "1.0.0",
                "actions": [{
                    "id": "delay",
                    "config": { "type": "Object", "properties": { "duration": { "type": "Duration" } } }
                }]
            }))
            .unwrap(),
        );
        let schema = h.backend.action_schema("time", "delay").unwrap();
        assert_eq!(schema["type"], "Object");
        assert_eq!(schema["properties"]["duration"]["type"], "Duration");
        assert!(h.backend.action_schema("time", "missing").is_none());
    }

    #[test]
    fn locale_bundle_and_switching() {
        let mut locales = LocaleRegistry::new();
        locales.register(
            &Namespace::Plugin("time".into()),
            "en",
            json!({ "actions": { "delay": { "name": "Delay" } } })
                .as_object()
                .unwrap()
                .clone(),
        );
        let h = harness_with(locales);

        let bundle = h.backend.locale_bundle("en");
        assert_eq!(bundle["plugin"]["time"]["actions"]["delay"]["name"], json!("Delay"));
        assert_eq!(h.backend.available_locales(), vec!["en"]);

        assert_eq!(h.backend.current_locale(), "en");
        h.backend.set_locale("fr".into());
        assert_eq!(h.backend.current_locale(), "fr");
        assert!(matches!(
            h.sink.events().last(),
            Some(AppEvent::LocaleChanged { locale }) if locale == "fr"
        ));
    }

    #[tokio::test]
    async fn state_changes_are_forwarded_to_the_ui() {
        let h = harness();
        h.backend.clone().spawn_state_forwarder();
        // Let the forwarder subscribe.
        tokio::task::yield_now().await;
        h.state.set("time.last_delay", json!(5));
        // Give the forwarder a chance to deliver.
        for _ in 0..10 {
            if !h.sink.events().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(matches!(
            h.sink.events().last(),
            Some(AppEvent::StateChanged { key, value }) if key == "time.last_delay" && *value == json!(5)
        ));
    }

    // --- marketplace integration (M6.6) ----------------------------------

    /// Records which bundles the "host" was asked to hot-load, in order.
    struct RecordingLoader(Arc<Mutex<Vec<String>>>);
    impl HostLoader for RecordingLoader {
        fn load_bundle(
            &self,
            kind: &str,
            dir: &std::path::Path,
            _wasm: &str,
            _certified: bool,
        ) -> Result<(), String> {
            let id = dir.file_name().unwrap().to_string_lossy();
            self.0.lock().unwrap().push(format!("{kind}/{id}"));
            Ok(())
        }
    }

    /// An in-memory registry server (url → bytes).
    #[derive(Default)]
    struct MockFetcher(Mutex<std::collections::HashMap<String, Vec<u8>>>);
    impl quicktys_registry::Fetcher for MockFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.0
                .lock()
                .unwrap()
                .get(url)
                .cloned()
                .ok_or_else(|| format!("404 {url}"))
        }
    }

    fn pack(manifest: &str, wasm_name: &str) -> Vec<u8> {
        let dir = temp_dir();
        std::fs::write(dir.join("manifest.json"), manifest).unwrap();
        std::fs::write(dir.join(wasm_name), b"\0asm").unwrap();
        quicktys_pkg::pack_dir(&dir).unwrap()
    }

    #[test]
    fn registry_install_resolves_deps_and_hot_loads_in_order() {
        let h = harness();
        let data = temp_dir();
        let loaded = Arc::new(Mutex::new(Vec::<String>::new()));
        h.backend.set_installer(
            data.clone(),
            Arc::new(RecordingLoader(loaded.clone())),
            Arc::new(quicktys_cert::TrustStore::default()),
        );

        // A plugin that requires a module → the resolver must pull the module too.
        let module = pack(
            r#"{"id":"http","kind":"module","version":"1.2.0","wasm":"http.wasm"}"#,
            "http.wasm",
        );
        let plugin = pack(
            r#"{"id":"discord","kind":"plugin","version":"1.0.0","wasm":"discord.wasm","requires":[{"module":"http","version":"^1"}]}"#,
            "discord.wasm",
        );
        let base = "https://reg.test";
        let index = format!(
            r#"{{"schema":1,"packages":[
                {{"id":"http","kind":"module","version":"1.2.0","content_hash":"{}","url":"blobs/http.qtpkg","requires":[]}},
                {{"id":"discord","kind":"plugin","version":"1.0.0","content_hash":"{}","url":"blobs/discord.qtpkg","requires":[{{"module":"http","version":"^1"}}]}}
            ]}}"#,
            quicktys_pkg::content_hash(&module),
            quicktys_pkg::content_hash(&plugin),
        );
        let fetcher = Arc::new(MockFetcher::default());
        {
            let mut f = fetcher.0.lock().unwrap();
            f.insert(format!("{base}/index.json"), index.into_bytes());
            f.insert(format!("{base}/blobs/http.qtpkg"), module);
            f.insert(format!("{base}/blobs/discord.qtpkg"), plugin);
        }
        h.backend
            .set_registry(fetcher, vec![base.to_string()]);

        // Refresh, then install the plugin by id.
        assert_eq!(h.backend.marketplace_refresh().unwrap(), 2);
        let recs = h.backend.install_from_registry("plugin", "discord").unwrap();
        assert_eq!(recs.len(), 2);

        // The module was hot-loaded before the plugin that needs it.
        assert_eq!(
            *loaded.lock().unwrap(),
            ["module/http", "plugin/discord"]
        );

        // Lockfile: the plugin is explicit, the pulled module is a dependency.
        let installed = quicktys_install::list(&data).unwrap();
        let discord = installed.iter().find(|r| r.slug() == "plugin/discord").unwrap();
        let http = installed.iter().find(|r| r.slug() == "module/http").unwrap();
        assert!(discord.explicit);
        assert!(!http.explicit);
        assert_eq!(http.required_by, ["plugin/discord"]);
    }
}
