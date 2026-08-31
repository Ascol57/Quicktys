//! The application backend facade.
//!
//! Holds the core services and exposes the command surface (UI → host) as plain
//! async methods. `quicktys-app` wraps each in a thin `#[tauri::command]`; this
//! module knows nothing about Tauri, so it is fully unit-testable.

use crate::event::{AppEvent, EventSink, ResourceKind};
use quicktys_core::{
    Automation, ExtensionRegistry, Namespace, ProgressSink, Profile, QueueConfig, Resource,
    ResourceStore, Runner, StateStore, TriggerBus, TriggerEvent, UiDecl,
};
use quicktys_install::{ConsentStore, Kind, Source};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

const DEFAULT_LOCALE: &str = "en";

/// Calls a module service (`module`, `service`, JSON args) → JSON result. The app
/// wires this to the WASM host; `quicktys-ipc` stays host-agnostic (testable).
pub type ModuleCaller = Arc<dyn Fn(&str, &str, Value) -> Result<Value, String> + Send + Sync>;

/// Delivers persisted settings to a loaded extension `(id, settings_json)`. The
/// app wires it over the WASM host's `apply_settings`.
pub type SettingsApplier = Arc<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>;

/// Current time as Unix seconds (for entitlement/signature expiry checks).
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Loads an extracted plugin/module bundle into the **running** WASM host, so an
/// install takes effect without a restart. The app implements it over `WasmHost`;
/// `quicktys-ipc` stays host-agnostic (testable).
pub trait HostLoader: Send + Sync {
    /// `kind` is `"plugin"` or `"module"`; `dir` is the installed bundle dir;
    /// `wasm` is the component file name inside it; `certified` gates a module's
    /// system capabilities (hash-only installs are uncertified — signature M6.5).
    fn load_bundle(&self, kind: &str, dir: &Path, wasm: &str, certified: bool)
        -> Result<(), String>;
}

/// The install context, injected once at boot: where to install, how to hot-load,
/// and the pinned trust anchors that decide `certified` (M6.5).
#[derive(Clone)]
struct InstallCtx {
    root: PathBuf,
    loader: Arc<dyn HostLoader>,
    trust: Arc<quicktys_cert::TrustStore>,
}

impl InstallCtx {
    /// The trust store for an install: the pinned CA keys plus the CRL cached by
    /// the last registry refresh (so revocations take effect, M6.6).
    fn effective_trust(&self) -> quicktys_cert::TrustStore {
        let crl = std::fs::read(self.root.join("registry-cache/crl.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        quicktys_cert::TrustStore {
            ca_keys: self.trust.ca_keys.clone(),
            registry_keys: Vec::new(),
            store_keys: self.trust.store_keys.clone(),
            crl,
        }
    }
}

/// The registry context (marketplace, M6.6): how to fetch, and which registries.
#[derive(Clone)]
struct RegistryCtx {
    fetcher: Arc<dyn quicktys_registry::Fetcher>,
    /// Configured registry base URLs (first is used; multi-registry is a follow-up).
    registries: Vec<String>,
}

/// The capability-consent store (M6.7), shared with the host's consent check.
#[derive(Clone)]
struct ConsentState {
    store: Arc<RwLock<ConsentStore>>,
    root: PathBuf,
}

/// Summary of a loaded plugin, for the UI plugin list.
#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub actions: Vec<String>,
    pub triggers: Vec<String>,
    /// Reactive variable ids the plugin exposes (for `@plugin.<id>` templates).
    pub state: Vec<String>,
    /// UI contributions this plugin injects into named interface slots.
    pub ui: Vec<UiDecl>,
    /// The plugin's settings schema (if any), for the config form.
    pub settings: Option<Value>,
}

/// Summary of a loaded module (certified driver), for the UI.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub services: Vec<String>,
    /// The module's settings schema (if any), for the config form.
    pub settings: Option<Value>,
    /// A view/tab the module contributes to the app shell (icon/label/entry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<Value>,
}

/// The host-side application backend: the one object the IPC commands call into.
pub struct AppBackend {
    state: Arc<StateStore>,
    #[allow(dead_code)]
    bus: Arc<TriggerBus>,
    runner: Runner,
    registry: Arc<Mutex<ExtensionRegistry>>,
    locales: Mutex<quicktys_core::LocaleRegistry>,
    // Shared with the trigger Dispatcher so profile edits dispatch live.
    profiles: Arc<RwLock<ResourceStore<Profile>>>,
    automations: Mutex<ResourceStore<Automation>>,
    queues: Mutex<ResourceStore<QueueConfig>>,
    /// Widget manifests discovered from widget bundles (frontend-only extensions).
    /// Behind a lock so runtime installs can extend the catalogue.
    widgets: Mutex<Vec<Value>>,
    events: Arc<dyn EventSink>,
    locale: Mutex<String>,
    /// Bridge to the WASM host for direct module-service calls (settings, etc.);
    /// set by the app after boot. `None` in unit tests → calls error cleanly.
    module_caller: Mutex<Option<ModuleCaller>>,
    /// Install context (data root + hot-loader), injected at boot. `None` in unit
    /// tests → install/uninstall error cleanly.
    install: Mutex<Option<InstallCtx>>,
    /// Registry context (marketplace fetch), injected at boot.
    registry_ctx: Mutex<Option<RegistryCtx>>,
    /// Capability-consent store (M6.7), injected at boot.
    consent: Mutex<Option<ConsentState>>,
    /// Bridge that delivers settings to a running extension (`on_settings_changed`).
    settings_applier: Mutex<Option<SettingsApplier>>,
}

impl AppBackend {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: Arc<StateStore>,
        bus: Arc<TriggerBus>,
        runner: Runner,
        registry: Arc<Mutex<ExtensionRegistry>>,
        locales: quicktys_core::LocaleRegistry,
        profiles: ResourceStore<Profile>,
        automations: ResourceStore<Automation>,
        queues: ResourceStore<QueueConfig>,
        widgets: Vec<Value>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        AppBackend {
            state,
            bus,
            runner,
            registry,
            locales: Mutex::new(locales),
            profiles: Arc::new(RwLock::new(profiles)),
            automations: Mutex::new(automations),
            queues: Mutex::new(queues),
            widgets: Mutex::new(widgets),
            events,
            locale: Mutex::new(DEFAULT_LOCALE.to_string()),
            module_caller: Mutex::new(None),
            install: Mutex::new(None),
            registry_ctx: Mutex::new(None),
            consent: Mutex::new(None),
            settings_applier: Mutex::new(None),
        }
    }

    /// Install the bridge used by [`call_module`](Self::call_module). The app
    /// calls this once at boot with a closure over the WASM host.
    pub fn set_module_caller(&self, caller: ModuleCaller) {
        *self.module_caller.lock().unwrap() = Some(caller);
    }

    /// Call a module service directly (UI → host), e.g. the `pear` module's
    /// `set-base-url` from the Settings page.
    pub fn call_module(&self, module: &str, service: &str, args: Value) -> Result<Value, String> {
        let caller = self.module_caller.lock().unwrap().clone();
        match caller {
            Some(f) => f(module, service, args),
            None => Err("module calls are unavailable".to_string()),
        }
    }

    /// The shared profiles store, so the trigger [`Dispatcher`] and the UI act on
    /// the same data (a profile saved in the UI dispatches immediately).
    pub fn profiles_store(&self) -> Arc<RwLock<ResourceStore<Profile>>> {
        self.profiles.clone()
    }

    // --- plugins ----------------------------------------------------------

    /// List loaded plugins (for the UI).
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.registry
            .lock()
            .unwrap()
            .plugins()
            .map(|m| PluginInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                icon: m.icon.clone(),
                color: m.color.clone(),
                actions: m.actions.iter().map(|a| a.id.clone()).collect(),
                triggers: m.triggers.iter().map(|t| t.id.clone()).collect(),
                state: m.state.iter().map(|s| s.id.clone()).collect(),
                ui: m.ui.clone(),
                settings: m.settings.as_ref().and_then(|s| serde_json::to_value(s).ok()),
            })
            .collect()
    }

    /// List loaded modules (for the UI).
    pub fn list_modules(&self) -> Vec<ModuleInfo> {
        self.registry
            .lock()
            .unwrap()
            .modules()
            .map(|m| ModuleInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                services: m.services.iter().map(|s| s.id.clone()).collect(),
                settings: m.settings.as_ref().and_then(|s| serde_json::to_value(s).ok()),
                view: m.view.as_ref().and_then(|v| serde_json::to_value(v).ok()),
            })
            .collect()
    }

    /// List available widget manifests (from widget bundles). Widgets are
    /// frontend-only, so the host only catalogs them for the UI.
    pub fn list_widgets(&self) -> Vec<Value> {
        self.widgets.lock().unwrap().clone()
    }

    // --- install / marketplace (M6.2) -------------------------------------

    /// Install the bridge used by [`sideload_package`](Self::sideload_package) &
    /// friends: the data root, the hot-loader over the host, and the pinned trust
    /// store (empty = nothing verifies as certified).
    pub fn set_installer(
        &self,
        root: PathBuf,
        loader: Arc<dyn HostLoader>,
        trust: Arc<quicktys_cert::TrustStore>,
    ) {
        *self.install.lock().unwrap() = Some(InstallCtx {
            root,
            loader,
            trust,
        });
    }

    /// Install a local `.qtpkg` (M6.2, hash-only trust). Extracts it into
    /// `installed/`, updates the lockfile, hot-loads it, and emits
    /// `ExtensionsChanged`. Returns the lockfile record as JSON.
    pub fn sideload_package(&self, path: &str) -> Result<Value, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
        self.install_archive(&bytes, Source::Sideload, None, true)
    }

    /// Wire the marketplace registry (M6.6): how to fetch, and the registry URLs.
    pub fn set_registry(
        &self,
        fetcher: Arc<dyn quicktys_registry::Fetcher>,
        registries: Vec<String>,
    ) {
        *self.registry_ctx.lock().unwrap() = Some(RegistryCtx {
            fetcher,
            registries,
        });
    }

    /// Download + verify + cache the registry's signed index (and CRL). Returns
    /// the number of catalogued packages.
    pub fn marketplace_refresh(&self) -> Result<usize, String> {
        let (install, registry) = self.market_ctx()?;
        let base = registry.registries.first().ok_or("no registry configured")?;
        let cache = install.root.join("registry-cache");
        let index = quicktys_registry::refresh(
            base,
            registry.fetcher.as_ref(),
            &cache,
            &install.trust.registry_keys,
        )
        .map_err(|e| e.to_string())?;
        Ok(index.packages.len())
    }

    /// Search the cached registry index (highest version per id).
    pub fn marketplace_search(&self, query: &str, kind: Option<String>) -> Vec<Value> {
        let Ok((install, _registry)) = self.market_ctx() else {
            return Vec::new();
        };
        let cache = install.root.join("registry-cache");
        let Ok(index) = quicktys_registry::load_index(&cache) else {
            return Vec::new();
        };
        quicktys_registry::search(&index, query, kind.as_deref())
            .into_iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect()
    }

    /// Install a package from the registry by `kind`/`id`: resolve dependencies,
    /// download + integrity-check each blob, then install & hot-load them (modules
    /// before plugins). Returns the installed records.
    pub fn install_from_registry(&self, kind: &str, id: &str) -> Result<Vec<Value>, String> {
        let (install, registry) = self.market_ctx()?;
        let base = registry
            .registries
            .first()
            .ok_or("no registry configured")?
            .clone();
        let cache = install.root.join("registry-cache");
        // Resolve + download against the CURRENT catalogue: a best-effort refresh
        // first, so a stale cached hash never mismatches a re-published blob. Falls
        // back to the cache if the registry is offline.
        let _ = quicktys_registry::refresh(
            &base,
            registry.fetcher.as_ref(),
            &cache,
            &install.trust.registry_keys,
        );
        let index = quicktys_registry::load_index(&cache)
            .map_err(|_| "registry not refreshed yet".to_string())?;
        let installed: Vec<(String, String)> = quicktys_install::list(&install.root)
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.slug(), r.version))
            .collect();
        let plan = quicktys_registry::plan(&index, id, kind, &installed).map_err(|e| e.to_string())?;
        let fetched =
            quicktys_registry::fetch_all(&base, &plan, registry.fetcher.as_ref()).map_err(|e| e.to_string())?;

        let mut records = Vec::new();
        for pkg in fetched {
            records.push(self.install_archive(
                &pkg.archive,
                Source::Registry,
                Some(&pkg.content_hash),
                pkg.explicit,
            )?);
        }
        Ok(records)
    }

    /// Packages the registry offers a newer version of than what's installed.
    pub fn list_updates(&self) -> Vec<Value> {
        let Ok((install, _)) = self.market_ctx() else {
            return Vec::new();
        };
        let cache = install.root.join("registry-cache");
        let Ok(index) = quicktys_registry::load_index(&cache) else {
            return Vec::new();
        };
        let installed: Vec<(String, String)> = quicktys_install::list(&install.root)
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.slug(), r.version))
            .collect();
        quicktys_registry::updates(&index, &installed)
            .into_iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect()
    }

    // --- extension settings -----------------------------------------------

    /// Wire the bridge that applies settings to a running extension.
    pub fn set_settings_applier(&self, applier: SettingsApplier) {
        *self.settings_applier.lock().unwrap() = Some(applier);
    }

    fn settings_path(&self, id: &str) -> Option<PathBuf> {
        self.install
            .lock()
            .unwrap()
            .as_ref()
            .map(|c| c.root.join("settings").join(format!("{id}.json")))
    }

    /// An extension's persisted settings values (`{}` if none / unavailable).
    pub fn get_settings(&self, id: &str) -> Value {
        self.settings_path(id)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(|| json!({}))
    }

    /// Persist an extension's settings and deliver them to the running instance.
    pub fn set_settings(&self, id: &str, values: Value) -> Result<(), String> {
        let path = self.settings_path(id).ok_or("settings unavailable")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&values).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if let Some(applier) = self.settings_applier.lock().unwrap().clone() {
            let json = serde_json::to_string(&values).unwrap_or_else(|_| "null".into());
            applier(id, &json)?;
        }
        Ok(())
    }

    /// Deliver every persisted settings file to its (now-loaded) extension —
    /// called once at boot so settings survive restarts.
    pub fn apply_persisted_settings(&self) {
        let Some(applier) = self.settings_applier.lock().unwrap().clone() else {
            return;
        };
        let Some(dir) = self.install.lock().unwrap().as_ref().map(|c| c.root.join("settings")) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Ok(bytes) = std::fs::read(&path) {
                let json = String::from_utf8_lossy(&bytes);
                let _ = applier(id, &json);
            }
        }
    }

    // --- capability consent (M6.7) ----------------------------------------

    /// Wire the consent store, shared with the host's consent check.
    pub fn set_consent_store(&self, store: Arc<RwLock<ConsentStore>>, root: PathBuf) {
        *self.consent.lock().unwrap() = Some(ConsentState { store, root });
    }

    /// A loaded module's version + declared system caps (as strings).
    fn module_caps(&self, id: &str) -> Option<(String, Vec<String>)> {
        let reg = self.registry.lock().unwrap();
        let version = reg.modules().find(|m| m.id == id)?.version.clone();
        let caps = reg
            .module_capabilities(id)
            .into_iter()
            .map(|c| c.as_str().to_string())
            .collect();
        Some((version, caps))
    }

    /// Record the user's decision: `id` is granted exactly `caps`. Persists and
    /// emits `ConsentChanged`. Enforced immediately at the next cap dispatch.
    pub fn grant_capabilities(&self, id: &str, caps: Vec<String>) -> Result<(), String> {
        let Some(state) = self.consent.lock().unwrap().clone() else {
            return Err("consent is unavailable".to_string());
        };
        let version = self.module_caps(id).map(|(v, _)| v).unwrap_or_default();
        state.store.write().unwrap().grant(id, caps, &version);
        state
            .store
            .read()
            .unwrap()
            .save(&state.root)
            .map_err(|e| e.to_string())?;
        self.events.emit(AppEvent::ConsentChanged);
        Ok(())
    }

    /// Revoke one capability from a module (denied immediately).
    pub fn revoke_capability(&self, id: &str, cap: &str) -> Result<(), String> {
        let Some(state) = self.consent.lock().unwrap().clone() else {
            return Err("consent is unavailable".to_string());
        };
        let (version, declared) = self.module_caps(id).unwrap_or_default();
        state
            .store
            .write()
            .unwrap()
            .revoke(id, cap, &declared, &version);
        state
            .store
            .read()
            .unwrap()
            .save(&state.root)
            .map_err(|e| e.to_string())?;
        self.events.emit(AppEvent::ConsentChanged);
        Ok(())
    }

    /// Consent status of every loaded module that declares system caps: what it
    /// declared, what's granted, and what still needs a decision (for the UI).
    pub fn consent_status(&self) -> Vec<Value> {
        let Some(state) = self.consent.lock().unwrap().clone() else {
            return Vec::new();
        };
        let store = state.store.read().unwrap();
        let modules: Vec<(String, String, String, Vec<String>)> = {
            let reg = self.registry.lock().unwrap();
            reg.modules()
                .filter_map(|m| {
                    let caps: Vec<String> = reg
                        .module_capabilities(&m.id)
                        .into_iter()
                        .map(|c| c.as_str().to_string())
                        .collect();
                    if caps.is_empty() {
                        return None;
                    }
                    Some((m.id.clone(), m.name.clone(), m.version.clone(), caps))
                })
                .collect()
        };
        modules
            .into_iter()
            .map(|(id, name, version, declared)| {
                let granted: Vec<String> = declared
                    .iter()
                    .filter(|c| store.allows(&id, c))
                    .cloned()
                    .collect();
                let pending = store.needs_review(&id, &declared, &version);
                json!({
                    "id": id,
                    "name": name,
                    "version": version,
                    "declared": declared,
                    "granted": granted,
                    "pending": pending,
                    "reviewed": store.is_reviewed(&id),
                })
            })
            .collect()
    }

    // --- entitlements / licensing (M6.8) ----------------------------------

    /// Enter a `QTLIC-…` license key: verify it against the pinned store keys and
    /// persist it, so paid packages it covers can be installed. Returns the
    /// granted entitlement.
    pub fn enter_license_key(&self, key: &str) -> Result<Value, String> {
        let ctx = self.install.lock().unwrap().clone().ok_or("install is unavailable")?;
        let lic = quicktys_cert::parse_license(key).map_err(|e| e.to_string())?;
        let ent = quicktys_cert::verify_license(&lic, &ctx.trust.store_keys, now_secs())?;
        let mut store = quicktys_install::EntitlementStore::load(&ctx.root).map_err(|e| e.to_string())?;
        store.add(lic);
        store.save(&ctx.root).map_err(|e| e.to_string())?;
        self.events.emit(AppEvent::ExtensionsChanged);
        serde_json::to_value(&ent).map_err(|e| e.to_string())
    }

    /// The valid entitlements the user holds (for the UI).
    pub fn list_entitlements(&self) -> Vec<Value> {
        let Some(ctx) = self.install.lock().unwrap().clone() else {
            return Vec::new();
        };
        quicktys_install::EntitlementStore::load(&ctx.root)
            .unwrap_or_default()
            .valid(&ctx.trust.store_keys, now_secs())
            .into_iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect()
    }

    /// Snapshot both contexts, or error if the marketplace isn't wired.
    fn market_ctx(&self) -> Result<(InstallCtx, RegistryCtx), String> {
        let install = self
            .install
            .lock()
            .unwrap()
            .clone()
            .ok_or("install is unavailable")?;
        let registry = self
            .registry_ctx
            .lock()
            .unwrap()
            .clone()
            .ok_or("registry is unavailable")?;
        Ok((install, registry))
    }

    /// The installed packages (lockfile records), for the "Installed" UI.
    pub fn list_installed(&self) -> Vec<Value> {
        let Some(ctx) = self.install.lock().unwrap().clone() else {
            return Vec::new();
        };
        quicktys_install::list(&ctx.root)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| serde_json::to_value(r).ok())
            .collect()
    }

    /// Uninstall a package by slug (`<kind>/<id>`): removes its files and lockfile
    /// entry, and returns any modules left orphaned. The running WASM component
    /// stays loaded until the next restart (live unload needs host support); a
    /// widget is dropped from the live catalogue immediately.
    pub fn uninstall_package(&self, slug: &str) -> Result<Vec<String>, String> {
        let Some(ctx) = self.install.lock().unwrap().clone() else {
            return Err("install is unavailable".to_string());
        };
        let orphans = quicktys_install::uninstall(slug, &ctx.root).map_err(|e| e.to_string())?;
        if let Some(id) = slug.strip_prefix("widget/") {
            self.widgets
                .lock()
                .unwrap()
                .retain(|w| w.get("id").and_then(Value::as_str) != Some(id));
        }
        self.events.emit(AppEvent::ExtensionsChanged);
        Ok(orphans)
    }

    /// Remove every orphaned dependency-module (nothing depends on it anymore),
    /// returning the slugs removed. Emits `ExtensionsChanged` if anything changed.
    pub fn purge_orphans(&self) -> Result<Vec<String>, String> {
        let Some(ctx) = self.install.lock().unwrap().clone() else {
            return Err("install is unavailable".to_string());
        };
        let removed = quicktys_install::purge_orphans(&ctx.root).map_err(|e| e.to_string())?;
        if !removed.is_empty() {
            self.events.emit(AppEvent::ExtensionsChanged);
        }
        Ok(removed)
    }

    /// Shared install path. `explicit` is `true` for a user-requested package,
    /// `false` for a dependency pulled by the resolver (M6.6). The trust store
    /// merges the pinned CA keys with the CRL from the last registry refresh.
    fn install_archive(
        &self,
        bytes: &[u8],
        source: Source,
        expected: Option<&str>,
        explicit: bool,
    ) -> Result<Value, String> {
        let Some(ctx) = self.install.lock().unwrap().clone() else {
            return Err("install is unavailable".to_string());
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let outcome = quicktys_install::install(
            bytes,
            &ctx.root,
            source,
            expected,
            explicit,
            &ctx.effective_trust(),
            now,
        )
        .map_err(|e| e.to_string())?;
        match outcome.kind {
            Kind::Plugin | Kind::Module => {
                let wasm = outcome.wasm.as_deref().unwrap_or_default();
                ctx.loader
                    .load_bundle(outcome.kind.as_str(), &outcome.dir, wasm, outcome.certified)?;
                self.load_extension_locales(outcome.kind, &outcome.record.id, &outcome.dir);
            }
            Kind::Widget => {
                if let Some(wm) = outcome.widget_manifest {
                    let mut widgets = self.widgets.lock().unwrap();
                    widgets.retain(|w| w.get("id") != wm.get("id"));
                    widgets.push(wm);
                }
            }
            Kind::Content => {}
        }
        self.events.emit(AppEvent::ExtensionsChanged);
        serde_json::to_value(&outcome.record).map_err(|e| e.to_string())
    }

    /// Discover a freshly-installed extension's `lang/` into the live registry.
    fn load_extension_locales(&self, kind: Kind, id: &str, dir: &Path) {
        let ns = match kind {
            Kind::Plugin => Namespace::Plugin(id.to_string()),
            Kind::Module => Namespace::Module(id.to_string()),
            Kind::Widget => Namespace::Widget(id.to_string()),
            Kind::Content => return,
        };
        let _ = self
            .locales
            .lock()
            .unwrap()
            .load_lang_dir(&ns, &dir.join("lang"));
    }

    /// The config schema of a plugin action (drives the action's form in the
    /// automation editor). `None` if the plugin/action is unknown or schemaless.
    pub fn action_schema(&self, plugin: &str, action: &str) -> Option<Value> {
        let registry = self.registry.lock().unwrap();
        registry
            .resolve_action(plugin, action)
            .and_then(|a| a.config.as_ref())
            .and_then(|schema| serde_json::to_value(schema).ok())
    }

    // --- resources --------------------------------------------------------

    /// List every resource of a kind, as JSON.
    pub fn list_resources(&self, kind: ResourceKind) -> Vec<Value> {
        match kind {
            ResourceKind::Profile => self
                .profiles
                .read()
                .unwrap()
                .iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
            ResourceKind::Automation => self
                .automations
                .lock()
                .unwrap()
                .iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
            ResourceKind::Queue => self
                .queues
                .lock()
                .unwrap()
                .iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
        }
    }

    /// Fetch one resource as JSON.
    pub fn get_resource(&self, kind: ResourceKind, id: &str) -> Option<Value> {
        match kind {
            ResourceKind::Profile => self
                .profiles
                .read()
                .unwrap()
                .get(id)
                .and_then(|r| serde_json::to_value(r).ok()),
            ResourceKind::Automation => self
                .automations
                .lock()
                .unwrap()
                .get(id)
                .and_then(|r| serde_json::to_value(r).ok()),
            ResourceKind::Queue => self
                .queues
                .lock()
                .unwrap()
                .get(id)
                .and_then(|r| serde_json::to_value(r).ok()),
        }
    }

    /// Create or update a resource (persisted), emitting the corresponding event.
    pub fn upsert_resource(&self, kind: ResourceKind, value: Value) -> Result<(), String> {
        match kind {
            ResourceKind::Profile => {
                let mut store = self.profiles.write().unwrap();
                self.upsert_in(kind, value, &mut store)
            }
            ResourceKind::Automation => {
                let mut store = self.automations.lock().unwrap();
                self.upsert_in(kind, value, &mut store)
            }
            ResourceKind::Queue => {
                let mut store = self.queues.lock().unwrap();
                self.upsert_in(kind, value, &mut store)
            }
        }
    }

    /// Deserialize, upsert into an already-locked store, and emit the event.
    fn upsert_in<T>(
        &self,
        kind: ResourceKind,
        value: Value,
        store: &mut ResourceStore<T>,
    ) -> Result<(), String>
    where
        T: Resource + Clone,
    {
        let item: T = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let id = item.id().to_string();
        let json = serde_json::to_value(&item).map_err(|e| e.to_string())?;
        let existed = store.get(&id).is_some();
        store.upsert(item).map_err(|e| e.to_string())?;
        let event = if existed {
            AppEvent::ResourceUpdated { kind, id, value: json }
        } else {
            AppEvent::ResourceAdded { kind, id, value: json }
        };
        self.events.emit(event);
        Ok(())
    }

    /// Delete a resource, emitting `ResourceRemoved` if it existed.
    pub fn delete_resource(&self, kind: ResourceKind, id: &str) -> Result<(), String> {
        let removed = match kind {
            ResourceKind::Profile => self.profiles.write().unwrap().remove(id).map(|o| o.is_some()),
            ResourceKind::Automation => self
                .automations
                .lock()
                .unwrap()
                .remove(id)
                .map(|o| o.is_some()),
            ResourceKind::Queue => self.queues.lock().unwrap().remove(id).map(|o| o.is_some()),
        }
        .map_err(|e| e.to_string())?;
        if removed {
            self.events.emit(AppEvent::ResourceRemoved {
                kind,
                id: id.to_string(),
            });
        }
        Ok(())
    }

    // --- automations ------------------------------------------------------

    /// Run an automation by id with the given trigger context, returning its
    /// return value (if any).
    pub async fn run_automation(
        &self,
        id: &str,
        context: Value,
    ) -> Result<Option<Value>, String> {
        let root = self
            .automations
            .lock()
            .unwrap()
            .get(id)
            .map(|a| a.root.clone());
        let Some(root) = root else {
            return Err(format!("no such automation: {id}"));
        };
        let base = context.as_object().cloned().unwrap_or_default();
        let progress = Arc::new(ProgressForwarder {
            automation: id.to_string(),
            events: self.events.clone(),
        });
        self.runner
            .run_with_progress(&root, base, progress)
            .await
            .map_err(|e| e.to_string())
    }

    /// Run an arbitrary inline sequence (a node tree), returning its result.
    /// Used by the profile editor's per-trigger Test button, where the sequence
    /// lives inline in the profile rather than as a named automation.
    pub async fn run_sequence(&self, root: Value) -> Result<Option<Value>, String> {
        let node: quicktys_schema::Node =
            serde_json::from_value(root).map_err(|e| format!("invalid sequence: {e}"))?;
        self.runner
            .run(&node, serde_json::Map::new())
            .await
            .map_err(|e| e.to_string())
    }

    /// Fire a trigger on the bus with a JSON-object context. The Dispatcher runs
    /// any profile binding subscribed to `trigger` whose filter passes. This is
    /// how a frontend surface (e.g. a dashboard **button** widget) drives an
    /// automation without a WASM plugin in the loop.
    pub async fn emit_trigger(&self, trigger: String, context: Value) -> Result<(), String> {
        let context = match context {
            Value::Object(map) => map,
            Value::Null => serde_json::Map::new(),
            _ => return Err("trigger context must be a JSON object".to_string()),
        };
        self.bus.emit(TriggerEvent::new(trigger, context));
        Ok(())
    }

    // --- state ------------------------------------------------------------

    /// Snapshot of all reactive state.
    pub fn get_state(&self) -> Value {
        Value::Object(self.state.snapshot())
    }

    /// Forward reactive state changes to the UI as `StateChanged` events until
    /// the store is dropped. Run it on whatever runtime the host provides
    /// (`tauri::async_runtime::spawn`, or [`spawn_state_forwarder`] under tokio).
    pub async fn run_state_forwarder(self: Arc<Self>) {
        let mut rx = self.state.subscribe();
        while let Ok(key) = rx.recv().await {
            if let Some(value) = self.state.get(&key) {
                self.events.emit(AppEvent::StateChanged { key, value });
            }
        }
    }

    /// Spawn [`run_state_forwarder`](Self::run_state_forwarder) on the current
    /// tokio runtime.
    pub fn spawn_state_forwarder(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.run_state_forwarder())
    }

    // --- i18n -------------------------------------------------------------

    /// The merged locale bundle to hand `vue-i18n` (falls back to an empty map).
    pub fn locale_bundle(&self, locale: &str) -> Value {
        self.locales
            .lock()
            .unwrap()
            .bundle(locale)
            .cloned()
            .map(Value::Object)
            .unwrap_or_else(|| Value::Object(Default::default()))
    }

    /// Locales that have at least one bundle.
    pub fn available_locales(&self) -> Vec<String> {
        self.locales
            .lock()
            .unwrap()
            .locales()
            .map(str::to_string)
            .collect()
    }

    pub fn current_locale(&self) -> String {
        self.locale.lock().unwrap().clone()
    }

    /// Change the active UI locale, emitting `LocaleChanged`.
    pub fn set_locale(&self, locale: String) {
        *self.locale.lock().unwrap() = locale.clone();
        self.events.emit(AppEvent::LocaleChanged { locale });
    }
}

/// Bridges the sequence runner's [`ProgressSink`] to UI [`AppEvent::NodeProgress`]
/// events, tagging each with the running automation's id so the editor can match
/// telemetry to the automation it currently displays.
struct ProgressForwarder {
    automation: String,
    events: Arc<dyn EventSink>,
}

impl ProgressSink for ProgressForwarder {
    fn node(&self, path: &[usize], running: bool) {
        self.events.emit(AppEvent::NodeProgress {
            automation: self.automation.clone(),
            path: path.to_vec(),
            running,
        });
    }
}
