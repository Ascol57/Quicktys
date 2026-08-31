//! Quicktys desktop application — the Tauri shell hosting the Rust runtime.
//!
//! Boot: build the core services + the WASM host, assemble the IPC
//! [`AppBackend`], expose it to thin `#[tauri::command]` shims, forward state
//! changes to the WebView, and run the window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod webserver;

use std::path::PathBuf;
use std::sync::Arc;

use quicktys_core::{
    Automation, DataDir, Dispatcher, LocaleRegistry, Namespace, Profile, QueueConfig,
    ResourceStore, Runner, StateStore, TriggerBus,
};
use quicktys_expr::ExprEngine;
use quicktys_host::WasmHost;
use quicktys_ipc::{AppBackend, AppEvent, EventSink, ModuleInfo, PluginInfo, ResourceKind};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

/// An [`EventSink`] that forwards host events to the WebView over Tauri.
struct TauriSink {
    app: tauri::AppHandle,
}

impl EventSink for TauriSink {
    fn emit(&self, event: AppEvent) {
        let _ = self.app.emit("app://event", event);
    }
}

// --- Commands (UI → host): thin shims over the backend ---------------------

#[tauri::command]
fn list_plugins(backend: State<Arc<AppBackend>>) -> Vec<PluginInfo> {
    backend.list_plugins()
}

#[tauri::command]
fn list_modules(backend: State<Arc<AppBackend>>) -> Vec<ModuleInfo> {
    backend.list_modules()
}

#[tauri::command]
fn list_widgets(backend: State<Arc<AppBackend>>) -> Vec<Value> {
    backend.list_widgets()
}

#[tauri::command]
fn sideload_package(backend: State<Arc<AppBackend>>, path: String) -> Result<Value, String> {
    backend.sideload_package(&path)
}

#[tauri::command]
fn uninstall_package(backend: State<Arc<AppBackend>>, slug: String) -> Result<Vec<String>, String> {
    backend.uninstall_package(&slug)
}

#[tauri::command]
fn list_installed(backend: State<Arc<AppBackend>>) -> Vec<Value> {
    backend.list_installed()
}

#[tauri::command]
fn purge_orphans(backend: State<Arc<AppBackend>>) -> Result<Vec<String>, String> {
    backend.purge_orphans()
}

#[tauri::command]
fn marketplace_refresh(backend: State<Arc<AppBackend>>) -> Result<usize, String> {
    backend.marketplace_refresh()
}

#[tauri::command]
fn marketplace_search(
    backend: State<Arc<AppBackend>>,
    query: String,
    kind: Option<String>,
) -> Vec<Value> {
    backend.marketplace_search(&query, kind)
}

#[tauri::command]
fn install_from_registry(
    backend: State<Arc<AppBackend>>,
    kind: String,
    id: String,
) -> Result<Vec<Value>, String> {
    backend.install_from_registry(&kind, &id)
}

#[tauri::command]
fn list_updates(backend: State<Arc<AppBackend>>) -> Vec<Value> {
    backend.list_updates()
}

#[tauri::command]
fn consent_status(backend: State<Arc<AppBackend>>) -> Vec<Value> {
    backend.consent_status()
}

#[tauri::command]
fn enter_license_key(backend: State<Arc<AppBackend>>, key: String) -> Result<Value, String> {
    backend.enter_license_key(&key)
}

#[tauri::command]
fn list_entitlements(backend: State<Arc<AppBackend>>) -> Vec<Value> {
    backend.list_entitlements()
}

/// Fire a trigger on the bus (e.g. a dashboard **button** widget click emits
/// `button.clicked`). Profiles bound to that trigger run via the Dispatcher.
#[tauri::command]
async fn emit_trigger(
    backend: State<'_, Arc<AppBackend>>,
    trigger: String,
    context: Value,
) -> Result<(), String> {
    backend.emit_trigger(trigger, context).await
}

/// Boot-time extension loading progress, for the startup loading screen. Polled
/// on mount so a UI that missed the `BootProgress`/`BootReady` events still syncs.
#[tauri::command]
fn boot_status(boot: State<Arc<BootState>>) -> Value {
    use std::sync::atomic::Ordering::Relaxed;
    serde_json::json!({
        "done": boot.done.load(Relaxed),
        "index": boot.index.load(Relaxed),
        "total": boot.total.load(Relaxed),
        "current": *boot.current.lock().unwrap(),
    })
}

#[tauri::command]
fn get_settings(backend: State<Arc<AppBackend>>, id: String) -> Value {
    backend.get_settings(&id)
}

#[tauri::command]
fn set_settings(backend: State<Arc<AppBackend>>, id: String, values: Value) -> Result<(), String> {
    backend.set_settings(&id, values)
}

#[tauri::command]
fn grant_capabilities(
    backend: State<Arc<AppBackend>>,
    id: String,
    caps: Vec<String>,
) -> Result<(), String> {
    backend.grant_capabilities(&id, caps)
}

#[tauri::command]
fn revoke_capability(
    backend: State<Arc<AppBackend>>,
    id: String,
    cap: String,
) -> Result<(), String> {
    backend.revoke_capability(&id, &cap)
}

/// Hot-loads an installed plugin/module bundle into the running WASM host, so a
/// marketplace install takes effect without a restart (M6.2). Decrypts an
/// encrypted (paid) payload first, using the user's license (M6.9).
struct HostBundleLoader {
    host: Arc<WasmHost>,
    data_root: PathBuf,
    store_keys: Arc<Vec<quicktys_cert::VerifyingKey>>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Load a plugin/module component. If the package is **encrypted** (paid, M6.9) —
/// a `<wasm>.enc` payload with `manifest.protection` — decrypt it first with the
/// content key from a covering license, then load from the plaintext bytes.
/// Returns the loaded extension id.
fn load_component(
    host: &WasmHost,
    kind: &str,
    dir: &std::path::Path,
    wasm: &str,
    certified: bool,
    data_root: &std::path::Path,
    store_keys: &[quicktys_cert::VerifyingKey],
) -> anyhow::Result<String> {
    let enc_path = dir.join(format!("{wasm}.enc"));
    if enc_path.exists() {
        let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json"))?)?;
        let id = manifest["id"].as_str().unwrap_or_default().to_string();
        let ckid = manifest["protection"]["content_key_id"].as_str().unwrap_or_default().to_string();
        let key = quicktys_install::EntitlementStore::load(data_root)
            .unwrap_or_default()
            .key_material_for(kind, &id, store_keys, now_secs())
            .ok_or_else(|| anyhow::anyhow!("no license content key for {kind}/{id}"))?;
        let bytes = quicktys_install::decrypt_payload(&std::fs::read(&enc_path)?, &key, &ckid)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(match kind {
            "module" => host.load_module_bytes(&bytes, certified)?.id,
            "plugin" => host.load_plugin_bytes(&bytes)?.id,
            other => anyhow::bail!("unknown kind {other}"),
        })
    } else {
        let path = dir.join(wasm);
        Ok(match kind {
            "module" => host.load_module(&path, certified)?.id,
            "plugin" => host.load_plugin(&path)?.id,
            other => anyhow::bail!("unknown kind {other}"),
        })
    }
}

/// Drives the host's OAuth **Device Code** flow for the Settings page.
///
/// The interactive half lives here rather than behind the WIT `oauth.authorize`
/// because that call is synchronous: it has nowhere to hand back a user code for
/// the UI to display while we wait for the approval. The module still owns the
/// provider config (ARCHITECTURE §4.6) and reads the resulting token through
/// `oauth.access-token` as usual.
struct HostOAuth {
    host: Arc<WasmHost>,
}

impl quicktys_ipc::OAuthBridge for HostOAuth {
    fn start(
        &self,
        provider: &str,
        device_url: &str,
        token_url: &str,
        client_id: &str,
        scopes: &str,
    ) -> Result<Value, String> {
        let flow = self
            .host
            .oauth_start_device(provider, device_url, token_url, client_id, scopes)?;
        serde_json::to_value(flow).map_err(|e| e.to_string())
    }

    fn poll(&self, provider: &str) -> Result<String, String> {
        use quicktys_host::oauth::DeviceStatus;
        Ok(match self.host.oauth_poll_device(provider)? {
            DeviceStatus::Pending => "pending".to_string(),
            DeviceStatus::Authorized => "authorized".to_string(),
            DeviceStatus::Failed(why) => format!("failed: {why}"),
        })
    }

    fn is_authorized(&self, provider: &str) -> bool {
        self.host.oauth_is_authorized(provider)
    }
    fn is_pending(&self, provider: &str) -> bool {
        self.host.oauth_device_pending(provider)
    }
    fn disconnect(&self, provider: &str) {
        self.host.oauth_disconnect(provider);
    }
}

/// Backs the registry client's HTTP transport with the host's client (M6.6).
struct HostFetcher {
    host: Arc<WasmHost>,
}

impl quicktys_registry::Fetcher for HostFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        self.host.http_get(url)
    }
}

impl quicktys_ipc::HostLoader for HostBundleLoader {
    fn load_bundle(
        &self,
        kind: &str,
        dir: &std::path::Path,
        wasm: &str,
        certified: bool,
    ) -> Result<(), String> {
        load_component(&self.host, kind, dir, wasm, certified, &self.data_root, &self.store_keys)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn action_schema(backend: State<Arc<AppBackend>>, plugin: String, action: String) -> Option<Value> {
    backend.action_schema(&plugin, &action)
}

#[tauri::command]
fn list_resources(backend: State<Arc<AppBackend>>, kind: ResourceKind) -> Vec<Value> {
    backend.list_resources(kind)
}

#[tauri::command]
fn get_resource(backend: State<Arc<AppBackend>>, kind: ResourceKind, id: String) -> Option<Value> {
    backend.get_resource(kind, &id)
}

#[tauri::command]
fn upsert_resource(
    backend: State<Arc<AppBackend>>,
    kind: ResourceKind,
    value: Value,
) -> Result<(), String> {
    backend.upsert_resource(kind, value)
}

#[tauri::command]
fn delete_resource(
    backend: State<Arc<AppBackend>>,
    kind: ResourceKind,
    id: String,
) -> Result<(), String> {
    backend.delete_resource(kind, &id)
}

#[tauri::command]
async fn run_automation(
    backend: State<'_, Arc<AppBackend>>,
    id: String,
    context: Value,
) -> Result<Option<Value>, String> {
    backend.run_automation(&id, context).await
}

#[tauri::command]
async fn run_sequence(
    backend: State<'_, Arc<AppBackend>>,
    root: Value,
) -> Result<Option<Value>, String> {
    backend.run_sequence(root).await
}

#[tauri::command]
fn get_state(backend: State<Arc<AppBackend>>) -> Value {
    backend.get_state()
}

#[tauri::command]
fn call_module(
    backend: State<Arc<AppBackend>>,
    module: String,
    service: String,
    args: Value,
) -> Result<Value, String> {
    backend.call_module(&module, &service, args)
}

/// Start connecting an account (OAuth Device Code): returns the `user_code` to
/// show and the `verification_uri` to open.
#[tauri::command]
fn oauth_connect(backend: State<Arc<AppBackend>>, module: String) -> Result<Value, String> {
    backend.oauth_connect(&module)
}

/// Poll a pending connection: `"pending"` | `"authorized"` | `"failed: …"`.
#[tauri::command]
fn oauth_poll(backend: State<Arc<AppBackend>>, provider: String) -> Result<String, String> {
    backend.oauth_poll(&provider)
}

#[tauri::command]
fn oauth_status(backend: State<Arc<AppBackend>>, provider: String) -> Value {
    backend.oauth_status(&provider)
}

#[tauri::command]
fn oauth_disconnect(backend: State<Arc<AppBackend>>, provider: String) -> Result<(), String> {
    backend.oauth_disconnect(&provider)
}

#[tauri::command]
fn available_locales(backend: State<Arc<AppBackend>>) -> Vec<String> {
    backend.available_locales()
}

#[tauri::command]
fn locale_bundle(backend: State<Arc<AppBackend>>, locale: String) -> Value {
    backend.locale_bundle(&locale)
}

#[tauri::command]
fn set_locale(backend: State<Arc<AppBackend>>, locale: String) {
    backend.set_locale(locale);
}

// --- Boot -------------------------------------------------------------------

fn data_dir() -> PathBuf {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".local/share"))
        .unwrap_or_else(std::env::temp_dir);
    base.join("quicktys")
}

/// The repo root (compile-time) — used to find the core `lang/` folder and the
/// dev-built bundles (`dist/bundles`) during development.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `bundle.json` descriptor: the self-describing header of an extension bundle
/// (produced by `scripts/build-bundles.sh`).
#[derive(serde::Deserialize)]
struct BundleManifest {
    id: String,
    kind: String, // "plugin" | "module"
    wasm: String,
    #[serde(default)]
    certified: bool,
}

/// Bundle roots, most-specific last (a user-installed bundle overrides a dev one
/// of the same id, since we skip ids already loaded): dev bundles built into the
/// repo by `scripts/build-bundles.sh`, then the user's installed bundles.
///
/// Set `QUICKTYS_NO_DEV_BUNDLES=1` to skip the dev bundles entirely and boot only
/// from `installed/` (a clean slate to test the marketplace).
/// Parse a comma-separated list of hex hybrid verifying keys from an env var.
fn pinned_keys(var: &str) -> Vec<quicktys_cert::VerifyingKey> {
    std::env::var(var)
        .ok()
        .into_iter()
        .flat_map(|s| s.split(',').map(str::trim).map(str::to_string).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .filter_map(|hex| {
            quicktys_cert::from_hex(&hex)
                .ok()
                .and_then(|b| quicktys_cert::VerifyingKey::from_bytes(&b).ok())
        })
        .collect()
}

fn bundle_roots(data: &DataDir) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if std::env::var_os("QUICKTYS_NO_DEV_BUNDLES").is_none() {
        roots.push(repo_root().join("dist/bundles"));
    } else {
        tracing::info!("QUICKTYS_NO_DEV_BUNDLES set — skipping dev bundles, loading only installed/");
    }
    roots.push(data.installed());
    roots
}

/// Load every extension bundle under `<root>/{plugins,modules}`. A bundle is a
/// self-contained directory (`<id>.wasm`, an optional `lang/`, and `bundle.json`)
/// — the host loads the component and discovers the bundle's own locales, with no
/// hardcoded dev paths. `want_module` selects which kind to load in this pass
/// (modules first, since a plugin may depend on one).
/// One extension queued for loading: everything needed to compile its WASM
/// component, resolved cheaply from `bundle.json` (no WASM work yet).
struct LoadTask {
    kind: &'static str,
    bundle: PathBuf,
    wasm: String,
    certified: bool,
    id: String,
}

/// Plan (don't load) the bundles in `dir`: read each `bundle.json`, load its
/// `lang/` into `locales` (cheap — no WASM compile), and append a [`LoadTask`].
/// Duplicate ids (same id across roots) are skipped, first wins. The slow part —
/// `wasmtime` compiling each component — is deferred to [`spawn_extension_loader`]
/// so the window can render a loading screen instead of freezing.
fn plan_bundle_dir(
    locales: &mut LocaleRegistry,
    dir: &std::path::Path,
    want_module: bool,
    seen: &mut std::collections::HashSet<String>,
    plan: &mut Vec<LoadTask>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let bundle = entry.path();
        if !bundle.is_dir() {
            continue;
        }
        let text = match std::fs::read_to_string(bundle.join("bundle.json")) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let manifest: BundleManifest = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("invalid bundle {bundle:?}: {e}");
                continue;
            }
        };
        if (manifest.kind == "module") != want_module {
            continue;
        }
        if !seen.insert(manifest.id.clone()) {
            continue; // already planned from an earlier root
        }
        let kind = if want_module { "module" } else { "plugin" };
        let ns = if want_module {
            Namespace::Module(manifest.id.clone())
        } else {
            Namespace::Plugin(manifest.id.clone())
        };
        let _ = locales.load_lang_dir(&ns, &bundle.join("lang"));
        plan.push(LoadTask {
            kind,
            bundle,
            wasm: manifest.wasm,
            certified: manifest.certified,
            id: manifest.id,
        });
    }
}

/// Discover widget bundles (`<root>/widgets/<id>/bundle.json`). Widgets are
/// frontend-only (no WASM): the host just reads their manifest and locales, and
/// hands the manifests to the UI via `list_widgets`.
fn load_widget_bundles(locales: &mut LocaleRegistry, data: &DataDir) -> Vec<Value> {
    let mut widgets = Vec::new();
    for root in bundle_roots(data) {
        let Ok(entries) = std::fs::read_dir(root.join("widgets")) else {
            continue;
        };
        for entry in entries.flatten() {
            let bundle = entry.path();
            let Ok(text) = std::fs::read_to_string(bundle.join("bundle.json")) else {
                continue;
            };
            let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
                tracing::warn!("invalid widget bundle {bundle:?}");
                continue;
            };
            if let Some(id) = manifest.get("id").and_then(|v| v.as_str()) {
                // Skip a widget id already seen (dev bundle shadowed by installed).
                if widgets.iter().any(|w: &Value| w.get("id") == manifest.get("id")) {
                    continue;
                }
                let id = id.to_string();
                let _ = locales.load_lang_dir(&Namespace::Widget(id.clone()), &bundle.join("lang"));
                // Runtime widgets ship an `index.html`; embed it for the sandboxed
                // iframe the frontend renders it in.
                let mut manifest = manifest;
                if let Ok(html) = std::fs::read_to_string(bundle.join("index.html")) {
                    if let Some(obj) = manifest.as_object_mut() {
                        obj.insert("html".into(), Value::String(html));
                    }
                }
                // Optional editor-only preview page (a self-demo), embedded for
                // the browser mock; native loads it via the `widget://` scheme.
                if let Some(file) = manifest.get("preview").and_then(|v| v.as_str()) {
                    let file = file.to_string();
                    if let Ok(html) = std::fs::read_to_string(bundle.join(&file)) {
                        if let Some(obj) = manifest.as_object_mut() {
                            obj.insert("previewHtml".into(), Value::String(html));
                        }
                    }
                }
                tracing::info!(widget = %id, "loaded widget bundle");
                widgets.push(manifest);
            }
        }
    }
    widgets
}

/// Plan all extension bundles: **modules before plugins** (drivers first), each
/// bundle's `lang/` discovered into `locales`. Returns the ordered load plan —
/// the actual WASM compilation is deferred (see [`spawn_extension_loader`]).
fn plan_extensions(locales: &mut LocaleRegistry, data: &DataDir) -> Vec<LoadTask> {
    // Core locales ship beside the app, not in a bundle.
    let _ = locales.load_lang_dir(&Namespace::Core, &repo_root().join("lang"));
    let roots = bundle_roots(data);
    let mut plan = Vec::new();
    let mut seen_modules = std::collections::HashSet::new();
    let mut seen_plugins = std::collections::HashSet::new();
    for root in &roots {
        plan_bundle_dir(locales, &root.join("modules"), true, &mut seen_modules, &mut plan);
    }
    for root in &roots {
        plan_bundle_dir(locales, &root.join("plugins"), false, &mut seen_plugins, &mut plan);
    }
    plan
}

/// Progress of boot-time extension loading, polled by the `boot_status` command
/// (so a UI that mounts mid-load, or misses the events, still knows where it is).
#[derive(Default)]
struct BootState {
    done: std::sync::atomic::AtomicBool,
    index: std::sync::atomic::AtomicUsize,
    total: std::sync::atomic::AtomicUsize,
    current: std::sync::Mutex<String>,
}

/// Load the planned extensions on a dedicated thread (wasmtime compilation is
/// blocking/CPU — keep it off the async runtime and off the UI). Emits a
/// `BootProgress` per extension, then — once modules exist — delivers persisted
/// settings, starts the pollers, and emits `BootReady` + `ExtensionsChanged`.
#[allow(clippy::too_many_arguments)]
fn spawn_extension_loader(
    host: Arc<WasmHost>,
    backend: Arc<AppBackend>,
    sink: Arc<TauriSink>,
    boot: Arc<BootState>,
    plan: Vec<LoadTask>,
    data_root: PathBuf,
    store_keys: Arc<Vec<quicktys_cert::VerifyingKey>>,
) {
    use std::sync::atomic::Ordering::Relaxed;
    std::thread::spawn(move || {
        let total = plan.len();
        boot.total.store(total, Relaxed);
        for (index, task) in plan.into_iter().enumerate() {
            boot.index.store(index, Relaxed);
            *boot.current.lock().unwrap() = task.id.clone();
            sink.emit(AppEvent::BootProgress {
                index,
                total,
                id: task.id.clone(),
                kind: task.kind.to_string(),
            });
            match load_component(
                &host,
                task.kind,
                &task.bundle,
                &task.wasm,
                task.certified,
                &data_root,
                &store_keys,
            ) {
                Ok(id) => tracing::info!(kind = task.kind, %id, "loaded bundle"),
                Err(e) => tracing::warn!("failed to load {} bundle {:?}: {e}", task.kind, task.bundle),
            }
        }
        boot.index.store(total, Relaxed);
        boot.done.store(true, Relaxed);
        // Modules now exist: deliver their persisted settings + start live polling.
        backend.apply_persisted_settings();
        spawn_module_pollers(host);
        sink.emit(AppEvent::BootReady);
        sink.emit(AppEvent::ExtensionsChanged);
    });
}

/// The interval at which pollable modules are asked to check for changes.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Spawn a background thread that periodically calls the `poll` service of every
/// loaded module that declares one. `poll` is where a driver detects change and
/// emits an event (which a plugin turns into a trigger) — this is what turns a
/// `song-changed`-style trigger into a live one. Errors (player not running) are
/// ignored. A dedicated thread keeps the (blocking) network poll off the async
/// runtime.
fn spawn_module_pollers(host: Arc<WasmHost>) {
    let pollable: Vec<String> = {
        let registry = host.registry();
        let registry = registry.lock().unwrap();
        registry
            .modules()
            .filter(|m| m.services.iter().any(|s| s.id == "poll"))
            .map(|m| m.id.clone())
            .collect()
    };
    if pollable.is_empty() {
        return;
    }
    tracing::info!(?pollable, "polling modules for live triggers");
    std::thread::spawn(move || loop {
        std::thread::sleep(POLL_INTERVAL);
        for id in &pollable {
            if let Err(e) = host.call_module(id, "poll", serde_json::json!({})) {
                tracing::debug!(module = %id, "poll failed: {e}");
            }
        }
    });
}

/// Serve a file from a bundle dir (`<root>/<kind>/<id>/<file>`) for a custom
/// scheme. Path is `/<id>/<file>` (default `index.html`); `..` is refused.
fn bundle_asset(kind: &str, uri_path: &str, default_file: &str) -> Option<(Vec<u8>, &'static str)> {
    let rel = uri_path.trim_start_matches('/');
    let mut parts = rel.splitn(2, '/');
    let id = parts.next().unwrap_or("");
    let file = match parts.next() {
        Some(f) if !f.is_empty() => f,
        _ => default_file,
    };
    if id.is_empty() || id.contains("..") || file.contains("..") {
        return None;
    }
    let data = DataDir::new(data_dir());
    for root in [repo_root().join("dist/bundles"), data.installed()] {
        let path = root.join(kind).join(id).join(file);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some((bytes, content_type(file)));
        }
    }
    None
}

/// Serve a widget bundle file (`widget://` scheme).
pub(crate) fn widget_asset(uri_path: &str) -> Option<(Vec<u8>, &'static str)> {
    bundle_asset("widgets", uri_path, "index.html")
}

/// Serve a plugin's custom UI file (`pluginui://` scheme) from `plugins/<id>/`.
pub(crate) fn plugin_asset(uri_path: &str) -> Option<(Vec<u8>, &'static str)> {
    bundle_asset("plugins", uri_path, "index.html")
}

/// Serve a module's contributed-view file (`moduleui://` scheme) from
/// `modules/<id>/` (the module bundle dir; `ui/` is copied in by the bundler).
pub(crate) fn module_asset(uri_path: &str) -> Option<(Vec<u8>, &'static str)> {
    bundle_asset("modules", uri_path, "index.html")
}

/// Build the HTTP response for a scheme handler from an optional asset.
fn serve_bundle_asset(asset: Option<(Vec<u8>, &'static str)>) -> tauri::http::Response<Vec<u8>> {
    match asset {
        Some((body, ct)) => tauri::http::Response::builder()
            .status(200)
            .header(tauri::http::header::CONTENT_TYPE, ct)
            .body(body)
            .unwrap(),
        None => tauri::http::Response::builder().status(404).body(Vec::new()).unwrap(),
    }
}

pub(crate) fn content_type(file: &str) -> &'static str {
    match file.rsplit('.').next().unwrap_or("") {
        "html" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn build_backend(app: &tauri::App) -> anyhow::Result<Arc<AppBackend>> {
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = Arc::new(WasmHost::new(state.clone(), bus.clone())?);

    // Persisted resources.
    let data = DataDir::new(data_dir());
    let _ = data.ensure_layout();

    // Back the `storage` capability with disk (per-extension), so modules that
    // own data (e.g. `webview` overlays) persist across restarts. Set before
    // loading so each extension's storage loads.
    host.set_storage_dir(data.root().join("storage"));

    // Connected accounts, so "connect once" is actually once: a device-code
    // authorization would otherwise have to be re-approved on Twitch at every
    // launch. Refresh tokens are long-lived; the file is 0600 (see oauth.rs).
    host.set_oauth_store(data.root().join("oauth.json"));

    // Pinned store keys (M6.8), needed early so encrypted installed packages can
    // be decrypted at boot from a covering license. `QUICKTYS_STORE` (comma-sep
    // hex) — the dev store key printed by `quicktys-pack --issue-license`.
    let store_keys = Arc::new(pinned_keys("QUICKTYS_STORE"));
    if !store_keys.is_empty() {
        tracing::info!(count = store_keys.len(), "pinned store key(s) from QUICKTYS_STORE");
    }

    // Discover each bundle's `lang/` + build the ordered load plan now (cheap:
    // just reads `bundle.json` + `lang/`). The slow part — wasmtime compiling
    // each component — is deferred to a background thread (`spawn_extension_loader`
    // below) so the window renders a loading screen instead of freezing.
    let mut locales = LocaleRegistry::new();
    let plan = plan_extensions(&mut locales, &data);
    let widgets = load_widget_bundles(&mut locales, &data);

    // The runner drives automations; its action invoker is backed by the WASM
    // host, with reactive state + the trigger bus wired in.
    let engine = Arc::new(ExprEngine::new());
    let runner =
        Runner::with_state(engine.clone(), host.invoker(), state.clone()).with_bus(bus.clone());

    let sink = Arc::new(TauriSink {
        app: app.handle().clone(),
    });

    // Forward plugin `reactivity.notify` calls to the UI as toasts.
    {
        let sink = sink.clone();
        host.set_notify(move |message, level| {
            sink.emit(AppEvent::Toast { message, level });
        });
    }

    let mut profiles = ResourceStore::<Profile>::new(data.profiles());
    let mut automations = ResourceStore::<Automation>::new(data.automations());
    let mut queues = ResourceStore::<QueueConfig>::new(data.queues());
    let _ = profiles.load();
    let _ = automations.load();
    let _ = queues.load();

    let backend = Arc::new(AppBackend::new(
        state.clone(),
        bus.clone(),
        runner.clone(),
        host.registry(),
        locales,
        profiles,
        automations,
        queues,
        widgets,
        sink.clone(),
    ));

    let state_for_ws = state.clone();

    // Wire the trigger dispatcher: when a trigger fires on the bus, active
    // profiles whose bindings match run their inline sequences. Shares the
    // backend's profiles store, so profiles saved in the UI dispatch live.
    let dispatcher = Arc::new(Dispatcher::new(
        runner,
        engine,
        state,
        backend.profiles_store(),
    ));
    tauri::async_runtime::spawn(dispatcher.run_forever(bus));

    // Let the UI call module services directly (e.g. `pear.set-base-url` from the
    // Settings page) via a closure over the host.
    {
        let host = host.clone();
        backend.set_module_caller(std::sync::Arc::new(move |module, service, args| {
            host.call_module(module, service, args)
        }));
    }

    // Interactive account connection (OAuth device flow) for the Settings page.
    backend.set_oauth(std::sync::Arc::new(HostOAuth { host: host.clone() }));

    // Deliver extension settings to their `on_settings_changed` hook.
    {
        let host = host.clone();
        backend.set_settings_applier(std::sync::Arc::new(move |id, json| {
            host.apply_settings(id, json)
        }));
    }

    // Let the UI install/uninstall `.qtpkg` packages at runtime, hot-loading them
    // into this same host (M6.2). The trust store pins the CA keys that make a
    // package `certified` (M6.5); empty for now (no CA operations yet), so
    // installed modules load uncertified and their system caps stay gated —
    // first-party dev bundles under `dist/bundles` are unaffected.
    // Pinned CA keys (M6.5): a package is `certified` only if its signature chains
    // to one of these. `QUICKTYS_CA` (comma-separated hex verifying keys) pins them
    // — e.g. the dev CA printed by `quicktys-pack --sign`. Empty → nothing certifies.
    let ca_keys: Vec<quicktys_cert::VerifyingKey> = std::env::var("QUICKTYS_CA")
        .ok()
        .into_iter()
        .flat_map(|s| s.split(',').map(str::trim).map(str::to_string).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .filter_map(|hex| {
            quicktys_cert::from_hex(&hex)
                .ok()
                .and_then(|b| quicktys_cert::VerifyingKey::from_bytes(&b).ok())
        })
        .collect();
    if !ca_keys.is_empty() {
        tracing::info!(count = ca_keys.len(), "pinned CA key(s) from QUICKTYS_CA");
    }
    backend.set_installer(
        data.root().to_path_buf(),
        Arc::new(HostBundleLoader {
            host: host.clone(),
            data_root: data.root().to_path_buf(),
            store_keys: store_keys.clone(),
        }),
        Arc::new(quicktys_cert::TrustStore {
            ca_keys,
            store_keys: (*store_keys).clone(),
            ..Default::default()
        }),
    );

    // Wire the marketplace registry client over the host's HTTP client (M6.6).
    // Registry URLs come from `QUICKTYS_REGISTRY` (comma-separated) for now — no
    // official registry is configured by default, so marketplace commands stay
    // inert until one is set.
    let registries: Vec<String> = std::env::var("QUICKTYS_REGISTRY")
        .ok()
        .map(|s| s.split(',').map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect())
        .unwrap_or_default();
    backend.set_registry(Arc::new(HostFetcher { host: host.clone() }), registries);

    // Capability consent (M6.7): a shared, persisted store gates system-cap
    // dispatch on top of certification. The host reads it at each dispatch; the
    // backend mutates it via grant/revoke.
    let consent = Arc::new(std::sync::RwLock::new(
        quicktys_install::ConsentStore::load(data.root()).unwrap_or_default(),
    ));
    {
        let consent = consent.clone();
        host.set_consent(move |id, cap| consent.read().unwrap().allows(id, cap));
    }
    backend.set_consent_store(consent, data.root().to_path_buf());

    // Serve overlays as browser sources (OBS) + stream live state over SSE.
    let overlay_server = Arc::new(webserver::OverlayServer {
        state: state_for_ws,
        backend: backend.clone(),
        port: webserver::DEFAULT_PORT,
    });
    tauri::async_runtime::spawn(webserver::serve(overlay_server));

    // Everything is wired but no extension WASM is loaded yet: kick off the
    // background loader (emits per-extension `BootProgress`, then `BootReady`).
    // The `boot_status` command lets a late-mounting UI catch up. Settings apply
    // + module pollers run inside it, after the modules exist.
    let boot = Arc::new(BootState::default());
    app.manage(boot.clone());
    spawn_extension_loader(
        host.clone(),
        backend.clone(),
        sink.clone(),
        boot,
        plan,
        data.root().to_path_buf(),
        store_keys.clone(),
    );

    // Keep the WASM host alive for the app's lifetime.
    app.manage(host);
    Ok(backend)
}

fn main() {
    // Default to `info`, but silence `enigo`'s per-input WARN spam (it logs a
    // Wayland-no-protocol warning on every action even when it falls back and
    // works). `RUST_LOG` still overrides.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,enigo=error"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tauri::Builder::default()
        // Serve runtime widgets from their own origin (`widget://localhost/<id>/`)
        // so each widget gets its own document/CSP — inline scripts run without
        // the app CSP's nonce blocking them.
        .register_uri_scheme_protocol("widget", |_app, request| {
            serve_bundle_asset(widget_asset(request.uri().path()))
        })
        // Custom plugin UI (slot contributions) from `pluginui://localhost/<id>/<file>`.
        .register_uri_scheme_protocol("pluginui", |_app, request| {
            serve_bundle_asset(plugin_asset(request.uri().path()))
        })
        // Module-contributed views/pages from `moduleui://localhost/<id>/<file>`.
        .register_uri_scheme_protocol("moduleui", |_app, request| {
            serve_bundle_asset(module_asset(request.uri().path()))
        })
        .setup(|app| {
            let backend = build_backend(app)?;
            tauri::async_runtime::spawn(backend.clone().run_state_forwarder());
            app.manage(backend);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_plugins,
            list_modules,
            list_widgets,
            action_schema,
            list_resources,
            get_resource,
            upsert_resource,
            delete_resource,
            run_automation,
            run_sequence,
            get_state,
            call_module,
            oauth_connect,
            oauth_poll,
            oauth_status,
            oauth_disconnect,
            available_locales,
            locale_bundle,
            set_locale,
            sideload_package,
            uninstall_package,
            list_installed,
            purge_orphans,
            marketplace_refresh,
            marketplace_search,
            install_from_registry,
            list_updates,
            consent_status,
            grant_capabilities,
            revoke_capability,
            get_settings,
            set_settings,
            enter_license_key,
            list_entitlements,
            emit_trigger,
            boot_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Quicktys");
}
