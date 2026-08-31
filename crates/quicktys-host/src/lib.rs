//! Quicktys WASM host — loads and runs both extension tiers as Component Model
//! components, and wires them together.
//!
//! - **Plugins** (anyone) import `logging`/`storage`/`reactivity`/`modules`.
//! - **Modules** (certified) import `logging`/`storage`/`events` (+ system caps,
//!   added in a later milestone). A module may request system capabilities only
//!   if it is certified.
//!
//! The **linker** routes a plugin's `modules.call` to the target module instance
//! (validating the declared dependency and the method), and fans a module's
//! `events.emit-event` out to subscribed plugins' `on-module-event`. Event
//! delivery is deferred until the triggering call returns, so a module emitting
//! mid-call never re-enters the plugin that is still on the stack.
//!
//! A [`WasmHost::invoker`] adapts loaded plugins to the core runner's
//! [`ActionInvoker`].

use quicktys_core::extensions::{Capability, ModuleManifest, PluginManifest};
use quicktys_core::invoker::BoxFut;
use quicktys_core::{ActionInvoker, ActionResult, ExtensionRegistry, StateStore, TriggerBus, TriggerEvent};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

mod audio;
mod hid;
mod http;
mod input;
pub mod oauth;
mod process;
mod ws;

mod plugin_bindings {
    wasmtime::component::bindgen!({ path: "../../wit", world: "plugin" });
}
mod module_bindings {
    wasmtime::component::bindgen!({ path: "../../wit", world: "module" });
}

/// Shorthand for the module-world generated interfaces.
use module_bindings::quicktys::api as mapi;

/// Shared world: registries of loaded instances + subscriptions + the pending
/// event queue. Both plugin and module instances reference it.
struct World {
    state: Arc<StateStore>,
    bus: Arc<TriggerBus>,
    registry: Arc<Mutex<ExtensionRegistry>>,
    plugins: Mutex<HashMap<String, Arc<Mutex<PluginInstance>>>>,
    modules: Mutex<HashMap<String, Arc<Mutex<ModuleInstance>>>>,
    /// (module id, event id) → subscribed plugin ids.
    subscriptions: Mutex<HashMap<(String, String), Vec<String>>>,
    /// Module events emitted during a call, delivered after it returns.
    pending: Mutex<VecDeque<PendingEvent>>,
    /// Open WebSocket write handles by `ws-connect` handle, for `net.send`.
    ws_senders: Mutex<HashMap<u64, ws::WsSender>>,
    /// OAuth token-sets (core-facilitated, module-facing).
    tokens: oauth::TokenStore,
    /// UI notification sink (message, level), wired by the app to toasts.
    notify: Mutex<Option<NotifySink>>,
    /// User consent check `(module_id, cap_name) -> allowed` (M6.7), wired by the
    /// app over its consent store. `None` → consent not enforced (may_grant only).
    consent: Mutex<Option<ConsentSink>>,
    /// Directory backing the `storage` capability (`<dir>/<id>.json` per
    /// extension). `None` → storage stays in-memory (tests).
    storage_dir: Mutex<Option<std::path::PathBuf>>,
}

/// A UI notification sink: `(message, level)`.
type NotifySink = Arc<dyn Fn(String, String) + Send + Sync>;

/// A consent check: `(module_id, cap_name) -> allowed`.
type ConsentSink = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

struct PendingEvent {
    module_id: String,
    event_id: String,
    payload_json: String,
}

/// Neutral routing error, mapped to the WIT `call-error` by the handler.
enum RouteErr {
    NotInstalled,
    NoSuchMethod,
    Denied,
    Failed(String),
}

impl World {
    /// Route a plugin's `modules.call` to a module service method.
    fn module_call(
        &self,
        plugin_id: &str,
        module_id: &str,
        method: &str,
        args_json: &str,
    ) -> Result<String, RouteErr> {
        {
            let registry = self.registry.lock().unwrap();
            // The plugin must have declared the dependency…
            if !registry.declares_dependency(plugin_id, module_id) {
                return Err(RouteErr::Denied);
            }
            // …and the loaded module must be present at a compatible version.
            if !registry.dependency_satisfied(plugin_id, module_id) {
                return Err(RouteErr::NotInstalled);
            }
            match registry.get_module(module_id) {
                None => return Err(RouteErr::NotInstalled),
                Some(m) => {
                    if !m.services.iter().any(|s| s.id == method) {
                        return Err(RouteErr::NoSuchMethod);
                    }
                }
            }
        }
        let instance = self.modules.lock().unwrap().get(module_id).cloned();
        let Some(instance) = instance else {
            return Err(RouteErr::NotInstalled);
        };
        let mut guard = instance.lock().unwrap();
        guard.call(method, args_json).map_err(RouteErr::Failed)
    }

    fn subscribe(&self, plugin_id: &str, module_id: &str, event_id: &str) -> Result<(), RouteErr> {
        {
            let registry = self.registry.lock().unwrap();
            match registry.get_module(module_id) {
                None => return Err(RouteErr::NotInstalled),
                Some(m) => {
                    if !m.events.iter().any(|e| e.id == event_id) {
                        return Err(RouteErr::NoSuchMethod);
                    }
                }
            }
        }
        let mut subs = self.subscriptions.lock().unwrap();
        let list = subs
            .entry((module_id.to_string(), event_id.to_string()))
            .or_default();
        if !list.iter().any(|p| p == plugin_id) {
            list.push(plugin_id.to_string());
        }
        Ok(())
    }

    fn unsubscribe(&self, plugin_id: &str, module_id: &str, event_id: &str) {
        if let Some(list) = self
            .subscriptions
            .lock()
            .unwrap()
            .get_mut(&(module_id.to_string(), event_id.to_string()))
        {
            list.retain(|p| p != plugin_id);
        }
    }

    /// Deliver every queued module event to its subscribers. Called after a
    /// top-level invocation returns, so no instance lock is held.
    fn deliver_pending(&self) {
        let mut budget = 10_000u32;
        loop {
            let event = self.pending.lock().unwrap().pop_front();
            let Some(event) = event else { break };
            budget -= 1;
            if budget == 0 {
                tracing::warn!("module event delivery budget exhausted");
                break;
            }
            let subscribers = self
                .subscriptions
                .lock()
                .unwrap()
                .get(&(event.module_id.clone(), event.event_id.clone()))
                .cloned()
                .unwrap_or_default();
            for plugin_id in subscribers {
                let instance = self.plugins.lock().unwrap().get(&plugin_id).cloned();
                if let Some(instance) = instance {
                    if let Ok(mut guard) = instance.lock() {
                        guard.on_module_event(&event.module_id, &event.event_id, &event.payload_json);
                    }
                }
            }
        }
    }
}

/// Per-instance store data: identity + a handle back to the shared world.
struct HostState {
    id: String,
    world: Weak<World>,
    kv: HashMap<String, String>,
    table: ResourceTable,
    wasi: WasiCtx,
}

impl HostState {
    fn new(world: &Arc<World>) -> Self {
        HostState {
            id: String::new(),
            world: Arc::downgrade(world),
            kv: HashMap::new(),
            table: ResourceTable::new(),
            wasi: WasiCtxBuilder::new().build(),
        }
    }

    /// The persistence file for this extension's storage, if a dir is configured.
    fn kv_path(&self) -> Option<std::path::PathBuf> {
        let world = self.world.upgrade()?;
        let dir = world.storage_dir.lock().unwrap().clone()?;
        (!self.id.is_empty()).then(|| dir.join(format!("{}.json", self.id)))
    }

    /// Whether the calling extension may exercise system capability `cap`
    /// (M6.5): certified **and** it declared the cap (`ExtensionRegistry::
    /// may_grant`). Enforced at every real cap dispatch below — defense in depth
    /// over the load-time gate, and the seam per-cap consent (M6.7) will refine.
    fn cap_granted(&self, cap: Capability) -> bool {
        let Some(world) = self.world.upgrade() else {
            return false;
        };
        // Certified + declared (trust boundary)…
        if !world.registry.lock().unwrap().may_grant(&self.id, cap) {
            return false;
        }
        // …and user consent (M6.7). No consent sink wired → not enforced.
        let check = world.consent.lock().unwrap().clone();
        match check {
            Some(check) => check(&self.id, cap.as_str()),
            None => true,
        }
    }

    /// Load persisted storage into `kv` (called once the id is known, at load).
    fn load_kv(&mut self) {
        if let Some(path) = self.kv_path() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&text) {
                    self.kv = map;
                }
            }
        }
    }

    /// Set a key and persist the whole map (durable storage).
    fn set_kv(&mut self, key: String, value: String) {
        self.kv.insert(key, value);
        if let Some(path) = self.kv_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(text) = serde_json::to_string(&self.kv) {
                let _ = std::fs::write(&path, text);
            }
        }
    }
}

impl WasiView for HostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

// --- Plugin-world capability imports ---------------------------------------

impl plugin_bindings::quicktys::api::logging::Host for HostState {
    fn log(&mut self, lvl: plugin_bindings::quicktys::api::logging::Level, message: String) {
        use plugin_bindings::quicktys::api::logging::Level;
        let id = &self.id;
        match lvl {
            Level::Trace => tracing::trace!(ext = %id, "{message}"),
            Level::Debug => tracing::debug!(ext = %id, "{message}"),
            Level::Info => tracing::info!(ext = %id, "{message}"),
            Level::Warn => tracing::warn!(ext = %id, "{message}"),
            Level::Error => tracing::error!(ext = %id, "{message}"),
        }
    }
}

impl plugin_bindings::quicktys::api::storage::Host for HostState {
    fn kv_get(&mut self, key: String) -> Option<String> {
        self.kv.get(&key).cloned()
    }
    fn kv_set(&mut self, key: String, value: String) {
        self.set_kv(key, value);
    }
    fn get_secret(&mut self, _key: String) -> Option<String> {
        None
    }
}

impl plugin_bindings::quicktys::api::reactivity::Host for HostState {
    fn set_state(&mut self, key: String, value_json: String) {
        if let Some(world) = self.world.upgrade() {
            let value: Value = serde_json::from_str(&value_json).unwrap_or(Value::Null);
            world.state.set(format!("{}.{}", self.id, key), value);
        }
    }
    fn emit_trigger(&mut self, trigger_id: String, context_json: String) {
        if let Some(world) = self.world.upgrade() {
            let context = serde_json::from_str::<Value>(&context_json)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            world
                .bus
                .emit(TriggerEvent::new(format!("{}.{}", self.id, trigger_id), context));
        }
    }
    fn notify(&mut self, message: String, level: String) {
        if let Some(world) = self.world.upgrade() {
            let sink = world.notify.lock().unwrap().clone();
            if let Some(sink) = sink {
                sink(message, level);
            }
        }
    }
}

impl plugin_bindings::quicktys::api::modules::Host for HostState {
    fn call(
        &mut self,
        module_id: String,
        method: String,
        args_json: String,
    ) -> Result<String, plugin_bindings::quicktys::api::modules::CallError> {
        use plugin_bindings::quicktys::api::modules::CallError;
        let Some(world) = self.world.upgrade() else {
            return Err(CallError::Failed("host gone".into()));
        };
        world
            .module_call(&self.id, &module_id, &method, &args_json)
            .map_err(|e| match e {
                RouteErr::NotInstalled => CallError::NotInstalled,
                RouteErr::NoSuchMethod => CallError::NoSuchMethod,
                RouteErr::Denied => CallError::Denied,
                RouteErr::Failed(m) => CallError::Failed(m),
            })
    }

    fn subscribe(
        &mut self,
        module_id: String,
        event_id: String,
    ) -> Result<(), plugin_bindings::quicktys::api::modules::CallError> {
        use plugin_bindings::quicktys::api::modules::CallError;
        let Some(world) = self.world.upgrade() else {
            return Err(CallError::Failed("host gone".into()));
        };
        world
            .subscribe(&self.id, &module_id, &event_id)
            .map_err(|e| match e {
                RouteErr::NotInstalled => CallError::NotInstalled,
                RouteErr::NoSuchMethod => CallError::NoSuchMethod,
                RouteErr::Denied => CallError::Denied,
                RouteErr::Failed(m) => CallError::Failed(m),
            })
    }

    fn unsubscribe(&mut self, module_id: String, event_id: String) {
        if let Some(world) = self.world.upgrade() {
            world.unsubscribe(&self.id, &module_id, &event_id);
        }
    }
}

// --- Module-world capability imports (subset for M4) ------------------------

impl module_bindings::quicktys::api::logging::Host for HostState {
    fn log(&mut self, lvl: module_bindings::quicktys::api::logging::Level, message: String) {
        use module_bindings::quicktys::api::logging::Level;
        let id = &self.id;
        match lvl {
            Level::Trace => tracing::trace!(ext = %id, "{message}"),
            Level::Debug => tracing::debug!(ext = %id, "{message}"),
            Level::Info => tracing::info!(ext = %id, "{message}"),
            Level::Warn => tracing::warn!(ext = %id, "{message}"),
            Level::Error => tracing::error!(ext = %id, "{message}"),
        }
    }
}

impl module_bindings::quicktys::api::storage::Host for HostState {
    fn kv_get(&mut self, key: String) -> Option<String> {
        self.kv.get(&key).cloned()
    }
    fn kv_set(&mut self, key: String, value: String) {
        self.set_kv(key, value);
    }
    fn get_secret(&mut self, _key: String) -> Option<String> {
        None
    }
}

impl mapi::events::Host for HostState {
    fn emit_event(&mut self, event_id: String, payload_json: String) {
        if let Some(world) = self.world.upgrade() {
            world.pending.lock().unwrap().push_back(PendingEvent {
                module_id: self.id.clone(),
                event_id,
                payload_json,
            });
        }
    }
}

// --- Module-world SYSTEM capabilities --------------------------------------
//
// `net.fetch` is real (host-mediated plain HTTP — see `http`). The streaming
// socket calls (`tcp/ws-connect`/`send`) and the device caps
// (input/audio/hid/files/process/timers) are host-side STUBS for now: they need
// real OS I/O / inbound delivery via `on-host-event`, wired with the first
// driver that requires them (e.g. a real OBS module over `ws-connect`).

impl mapi::net::Host for HostState {
    fn fetch(&mut self, req: mapi::net::HttpRequest) -> Result<mapi::net::HttpResponse, mapi::net::NetError> {
        if !self.cap_granted(Capability::Net) {
            return Err(mapi::net::NetError::Other("capability `net` not granted".to_string()));
        }
        let resp = http::request(&req.method, &req.url, &req.headers, req.body.as_deref())
            .map_err(mapi::net::NetError::Other)?;
        Ok(mapi::net::HttpResponse {
            status: resp.status,
            headers: resp.headers,
            body: resp.body,
        })
    }
    fn tcp_connect(&mut self, _host: String, _port: u16, _event_id: String) -> Result<u64, mapi::net::NetError> {
        if !self.cap_granted(Capability::Net) {
            return Err(mapi::net::NetError::Other("capability `net` not granted".to_string()));
        }
        Err(mapi::net::NetError::Other("tcp-connect not yet implemented".into()))
    }
    fn ws_connect(&mut self, url: String, event_id: String) -> Result<u64, mapi::net::NetError> {
        if !self.cap_granted(Capability::Net) {
            return Err(mapi::net::NetError::Other("capability `net` not granted".to_string()));
        }
        use std::sync::atomic::{AtomicU64, Ordering};
        static HANDLE: AtomicU64 = AtomicU64::new(1);
        let Some(world) = self.world.upgrade() else {
            return Err(mapi::net::NetError::Other("host gone".into()));
        };
        let module_id = self.id.clone();
        let handle = HANDLE.fetch_add(1, Ordering::Relaxed);
        // Reader thread: connect, then push each message to the module via
        // `on-host-event`, draining the events it emits to subscribers.
        std::thread::spawn(move || {
            let mut conn = match ws::connect(&url) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(module = %module_id, "ws-connect {url}: {e}");
                    return;
                }
            };
            // Register the write half so the module can `net.send` on this handle
            // (e.g. the obs-websocket Identify sent in reply to Hello).
            match conn.sender() {
                Ok(sender) => {
                    world.ws_senders.lock().unwrap().insert(handle, sender);
                }
                Err(e) => tracing::warn!(module = %module_id, "ws sender: {e}"),
            }
            tracing::info!(module = %module_id, "ws-connect {url} open");
            loop {
                let text = match conn.read_message() {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::debug!(module = %module_id, "ws closed: {e}");
                        break;
                    }
                };
                let instance = world.modules.lock().unwrap().get(&module_id).cloned();
                let Some(instance) = instance else { break };
                if let Ok(mut guard) = instance.lock() {
                    guard.on_host_event(&event_id, &text);
                }
                world.deliver_pending();
            }
            world.ws_senders.lock().unwrap().remove(&handle);
        });
        Ok(handle)
    }
    fn send(&mut self, handle: u64, data: Vec<u8>) -> Result<(), mapi::net::NetError> {
        if !self.cap_granted(Capability::Net) {
            return Err(mapi::net::NetError::Other("capability `net` not granted".to_string()));
        }
        let Some(world) = self.world.upgrade() else {
            return Err(mapi::net::NetError::Other("host gone".into()));
        };
        let text = String::from_utf8(data)
            .map_err(|_| mapi::net::NetError::Other("send: non-UTF8 payload".into()))?;
        let mut senders = world.ws_senders.lock().unwrap();
        match senders.get_mut(&handle) {
            Some(sender) => sender.send_text(&text).map_err(mapi::net::NetError::Other),
            None => Err(mapi::net::NetError::Other("send: no such connection".into())),
        }
    }
    fn close(&mut self, handle: u64) {
        if let Some(world) = self.world.upgrade() {
            world.ws_senders.lock().unwrap().remove(&handle);
        }
    }
}

impl mapi::oauth::Host for HostState {
    fn authorize(&mut self, provider: String, config: mapi::oauth::ProviderConfig) -> Result<(), String> {
        // Record the provider env so refreshes work; the interactive browser
        // flow (open URL, capture redirect, exchange the auth code) is the
        // remaining piece and reuses the same token store.
        if let Some(world) = self.world.upgrade() {
            world.tokens.set_provider(
                &provider,
                &config.token_url,
                &config.client_id,
                config.client_secret.as_deref(),
            );
        }
        Err("interactive authorization is not available in this build".into())
    }
    fn access_token(&mut self, provider: String) -> Result<String, String> {
        let world = self.world.upgrade().ok_or("host gone")?;
        world.tokens.access_token(&provider)
    }
    fn is_authorized(&mut self, provider: String) -> bool {
        self.world
            .upgrade()
            .map(|w| w.tokens.is_authorized(&provider))
            .unwrap_or(false)
    }
    fn revoke(&mut self, provider: String) {
        if let Some(world) = self.world.upgrade() {
            world.tokens.revoke(&provider);
        }
    }
}

impl mapi::input::Host for HostState {
    fn send_key(&mut self, _code: u32, _down: bool) {}
    fn send_mouse_move(&mut self, x: i32, y: i32) {
        if !self.cap_granted(Capability::Input) {
            return;
        }
        let _ = input::mouse_move(x, y);
    }
    fn send_mouse_button(&mut self, _button: u8, _down: bool) {}
    fn type_text(&mut self, text: String) -> Result<(), String> {
        if !self.cap_granted(Capability::Input) {
            return Err("capability `input` not granted".to_string());
        }
        input::type_text(&text)
    }
    fn key_combo(&mut self, keys: Vec<String>) -> Result<(), String> {
        if !self.cap_granted(Capability::Input) {
            return Err("capability `input` not granted".to_string());
        }
        input::key_combo(&keys)
    }
    fn mouse_click(&mut self, button: u8) -> Result<(), String> {
        if !self.cap_granted(Capability::Input) {
            return Err("capability `input` not granted".to_string());
        }
        input::mouse_click(button)
    }
}

impl mapi::audio::Host for HostState {
    fn list_output_devices(&mut self) -> Vec<String> {
        Vec::new()
    }
    fn play_buffer(
        &mut self,
        _device: Option<String>,
        _pcm: Vec<u8>,
        _sample_rate: u32,
        _channels: u8,
    ) -> Result<(), String> {
        Err("play-buffer not yet implemented (use play-file)".into())
    }
    fn speak(&mut self, text: String, voice: String) -> Result<(), String> {
        if !self.cap_granted(Capability::Audio) {
            return Err("capability `audio` not granted".to_string());
        }
        audio::speak(&text, &voice)
    }
    fn list_voices(&mut self) -> Vec<String> {
        audio::list_voices()
    }
    fn play_file(&mut self, path: String, volume: f32) -> Result<(), String> {
        if !self.cap_granted(Capability::Audio) {
            return Err("capability `audio` not granted".to_string());
        }
        audio::play_file(&path, volume)
    }
}

impl mapi::hid::Host for HostState {
    fn enumerate(&mut self, vendor_id: Option<u16>, product_id: Option<u16>) -> Vec<String> {
        if !self.cap_granted(Capability::Hid) {
            return Vec::new();
        }
        hid::enumerate(vendor_id, product_id)
    }

    fn open(&mut self, path: String, event_id: String) -> Result<u64, String> {
        if !self.cap_granted(Capability::Hid) {
            return Err("capability `hid` not granted".to_string());
        }
        use std::sync::atomic::{AtomicU64, Ordering};
        static HANDLE: AtomicU64 = AtomicU64::new(1);
        let Some(world) = self.world.upgrade() else {
            return Err("host gone".to_string());
        };
        let device = hid::open_device(&path)?;
        let handle = HANDLE.fetch_add(1, Ordering::Relaxed);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        hid::register(handle, path, stop.clone());
        let module_id = self.id.clone();
        // Reader thread: push each input report (as hex) to the module via
        // `on-host-event`, mirroring the `ws-connect` pattern.
        std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            while !stop.load(Ordering::Relaxed) {
                match device.read_timeout(&mut buf, 200) {
                    Ok(0) => continue,
                    Ok(n) => {
                        // Deliver the input report as a JSON array of bytes.
                        let payload = serde_json::to_string(&buf[..n]).unwrap_or_else(|_| "[]".into());
                        let instance = world.modules.lock().unwrap().get(&module_id).cloned();
                        let Some(instance) = instance else { break };
                        if let Ok(mut guard) = instance.lock() {
                            guard.on_host_event(&event_id, &payload);
                        }
                        world.deliver_pending();
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(handle)
    }

    fn write(&mut self, handle: u64, data: Vec<u8>) -> Result<(), String> {
        if !self.cap_granted(Capability::Hid) {
            return Err("capability `hid` not granted".to_string());
        }
        hid::write(handle, &data)
    }

    fn close(&mut self, handle: u64) {
        hid::close(handle);
    }
}

impl mapi::files::Host for HostState {
    fn read(&mut self, _path: String) -> Result<Vec<u8>, String> {
        Err("files capability not yet implemented".into())
    }
    fn write(&mut self, _path: String, _data: Vec<u8>) -> Result<(), String> {
        Err("files capability not yet implemented".into())
    }
    fn list_dir(&mut self, _path: String) -> Result<Vec<String>, String> {
        Err("files capability not yet implemented".into())
    }
}

impl mapi::process::Host for HostState {
    fn spawn(&mut self, program: String, args: Vec<String>) -> Result<u64, String> {
        if !self.cap_granted(Capability::Process) {
            return Err("capability `process` not granted".to_string());
        }
        process::spawn(&program, &args)
    }
    fn kill(&mut self, handle: u64) {
        if !self.cap_granted(Capability::Process) {
            return;
        }
        process::kill(handle);
    }
    fn run_shell(&mut self, command: String) -> Result<u64, String> {
        if !self.cap_granted(Capability::Process) {
            return Err("capability `process` not granted".to_string());
        }
        process::run_shell(&command)
    }
    fn open_path(&mut self, target: String) -> Result<(), String> {
        if !self.cap_granted(Capability::Process) {
            return Err("capability `process` not granted".to_string());
        }
        process::open_path(&target)
    }
}

impl mapi::timers::Host for HostState {
    fn set_timer(&mut self, _ms: u64, _repeat: bool, _event_id: String) -> u64 {
        0
    }
    fn clear_timer(&mut self, _handle: u64) {}
}

// --- Loaded instances -------------------------------------------------------

struct PluginInstance {
    store: Store<HostState>,
    bindings: plugin_bindings::Plugin,
}

impl PluginInstance {
    fn invoke(&mut self, action: &str, config: &Value, context: &Value) -> ActionResult {
        let config_json = serde_json::to_string(config).unwrap_or_else(|_| "null".into());
        let context_json = serde_json::to_string(context).unwrap_or_else(|_| "null".into());
        let guest = self.bindings.quicktys_api_plugin_guest();
        match guest.call_invoke_action(&mut self.store, action, &config_json, &context_json) {
            Ok(Ok(result_json)) => Ok(serde_json::from_str(&result_json).unwrap_or(Value::Null)),
            Ok(Err(plugin_bindings::exports::quicktys::api::plugin_guest::InvokeError::Aborted)) => {
                Err("aborted".to_string())
            }
            Ok(Err(plugin_bindings::exports::quicktys::api::plugin_guest::InvokeError::Failed(m))) => {
                Err(m)
            }
            Err(trap) => Err(format!("wasm trap: {trap}")),
        }
    }

    fn on_load(&mut self) -> Result<(), String> {
        let guest = self.bindings.quicktys_api_plugin_guest();
        match guest.call_on_load(&mut self.store) {
            Ok(result) => result,
            Err(trap) => Err(format!("on-load trap: {trap}")),
        }
    }

    fn on_module_event(&mut self, module_id: &str, event_id: &str, payload_json: &str) {
        let guest = self.bindings.quicktys_api_plugin_guest();
        if let Err(trap) =
            guest.call_on_module_event(&mut self.store, module_id, event_id, payload_json)
        {
            tracing::error!("on-module-event trap: {trap}");
        }
    }

    fn on_settings_changed(&mut self, settings_json: &str) -> Result<(), String> {
        let guest = self.bindings.quicktys_api_plugin_guest();
        guest
            .call_on_settings_changed(&mut self.store, settings_json)
            .map_err(|trap| format!("on-settings-changed trap: {trap}"))
    }
}

struct ModuleInstance {
    store: Store<HostState>,
    bindings: module_bindings::Module,
}

impl ModuleInstance {
    fn call(&mut self, method: &str, args_json: &str) -> Result<String, String> {
        let guest = self.bindings.quicktys_api_module_guest();
        match guest.call_call(&mut self.store, method, args_json) {
            Ok(result) => result,
            Err(trap) => Err(format!("module trap: {trap}")),
        }
    }

    fn on_load(&mut self) -> Result<(), String> {
        let guest = self.bindings.quicktys_api_module_guest();
        match guest.call_on_load(&mut self.store) {
            Ok(r) => r,
            Err(trap) => Err(format!("module trap: {trap}")),
        }
    }

    /// Deliver a host-capability event (socket data, timer tick) to the module.
    fn on_host_event(&mut self, event_id: &str, payload_json: &str) {
        let guest = self.bindings.quicktys_api_module_guest();
        if let Err(trap) = guest.call_on_host_event(&mut self.store, event_id, payload_json) {
            tracing::warn!("module on_host_event trap: {trap}");
        }
    }

    fn on_settings_changed(&mut self, settings_json: &str) -> Result<(), String> {
        let guest = self.bindings.quicktys_api_module_guest();
        guest
            .call_on_settings_changed(&mut self.store, settings_json)
            .map_err(|trap| format!("on-settings-changed trap: {trap}"))
    }
}

// --- Public host ------------------------------------------------------------

/// Loads and runs Quicktys plugins and modules.
pub struct WasmHost {
    engine: Engine,
    world: Arc<World>,
}

impl WasmHost {
    pub fn new(state: Arc<StateStore>, bus: Arc<TriggerBus>) -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;
        let world = Arc::new(World {
            state,
            bus,
            registry: Arc::new(Mutex::new(ExtensionRegistry::new())),
            plugins: Mutex::new(HashMap::new()),
            modules: Mutex::new(HashMap::new()),
            subscriptions: Mutex::new(HashMap::new()),
            pending: Mutex::new(VecDeque::new()),
            ws_senders: Mutex::new(HashMap::new()),
            tokens: oauth::TokenStore::new(),
            notify: Mutex::new(None),
            consent: Mutex::new(None),
            storage_dir: Mutex::new(None),
        });
        Ok(WasmHost { engine, world })
    }

    /// Back the `storage` capability with a directory (`<dir>/<id>.json` per
    /// extension). Set before loading extensions so their storage loads.
    pub fn set_storage_dir(&self, dir: impl Into<std::path::PathBuf>) {
        *self.world.storage_dir.lock().unwrap() = Some(dir.into());
    }

    /// Wire the UI notification sink: plugin `reactivity.notify(message, level)`
    /// calls are forwarded here (the app turns them into toasts).
    pub fn set_notify(&self, sink: impl Fn(String, String) + Send + Sync + 'static) {
        *self.world.notify.lock().unwrap() = Some(Arc::new(sink));
    }

    /// Wire the user-consent check (M6.7): `(module_id, cap_name) -> allowed`,
    /// consulted at every system-cap dispatch (in addition to certification). The
    /// app backs it with its persisted consent store.
    pub fn set_consent(&self, check: impl Fn(&str, &str) -> bool + Send + Sync + 'static) {
        *self.world.consent.lock().unwrap() = Some(Arc::new(check));
    }

    /// Instantiate a plugin component, decode its manifest, run `on-load`, and
    /// register it by id.
    pub fn load_plugin(&self, wasm_path: &Path) -> anyhow::Result<PluginManifest> {
        let component = Component::from_file(&self.engine, wasm_path)?;
        self.load_plugin_component(component)
    }

    /// Load a plugin from in-memory component bytes (e.g. a decrypted payload, M6.9).
    pub fn load_plugin_bytes(&self, bytes: &[u8]) -> anyhow::Result<PluginManifest> {
        let component = Component::from_binary(&self.engine, bytes)?;
        self.load_plugin_component(component)
    }

    fn load_plugin_component(&self, component: Component) -> anyhow::Result<PluginManifest> {
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;
        plugin_bindings::Plugin::add_to_linker(&mut linker, |s: &mut HostState| s)?;

        let mut store = Store::new(&self.engine, HostState::new(&self.world));
        let plugin = plugin_bindings::Plugin::instantiate(&mut store, &component, &linker)?;

        let guest = plugin.quicktys_api_plugin_guest();
        let manifest: PluginManifest = serde_json::from_str(&guest.call_init(&mut store)?)?;
        store.data_mut().id = manifest.id.clone();
        store.data_mut().load_kv();

        // Register before on-load, so the plugin can subscribe to modules during
        // on-load and the dependency lookups already see it.
        self.world
            .registry
            .lock()
            .unwrap()
            .register_plugin(manifest.clone());
        // Surface unsatisfied module dependencies (modules load before plugins,
        // so at this point a missing/incompatible module is a real gap). The
        // plugin still loads; `modules.call` denies the specific call at runtime.
        {
            let missing = self
                .world
                .registry
                .lock()
                .unwrap()
                .missing_dependencies(&manifest.id);
            if !missing.is_empty() {
                let list: Vec<String> = missing
                    .iter()
                    .map(|r| format!("{}@{}", r.module, r.version))
                    .collect();
                tracing::warn!(
                    plugin = %manifest.id,
                    "unsatisfied module dependencies: {}",
                    list.join(", ")
                );
            }
        }
        self.world.plugins.lock().unwrap().insert(
            manifest.id.clone(),
            Arc::new(Mutex::new(PluginInstance {
                store,
                bindings: plugin,
            })),
        );

        // Run on-load on the now-registered instance.
        {
            let instance = self.world.plugins.lock().unwrap().get(&manifest.id).cloned();
            if let Some(instance) = instance {
                instance
                    .lock()
                    .unwrap()
                    .on_load()
                    .map_err(|e| anyhow::anyhow!("plugin {} on-load failed: {e}", manifest.id))?;
            }
        }
        self.world.deliver_pending();
        Ok(manifest)
    }

    /// Instantiate a module component. A module that requests system
    /// capabilities is rejected unless `certified` (the trust boundary that
    /// keeps system access out of un-vetted code).
    pub fn load_module(&self, wasm_path: &Path, certified: bool) -> anyhow::Result<ModuleManifest> {
        let component = Component::from_file(&self.engine, wasm_path)?;
        self.load_module_component(component, certified)
    }

    /// Load a module from in-memory component bytes (e.g. a decrypted payload, M6.9).
    pub fn load_module_bytes(&self, bytes: &[u8], certified: bool) -> anyhow::Result<ModuleManifest> {
        let component = Component::from_binary(&self.engine, bytes)?;
        self.load_module_component(component, certified)
    }

    fn load_module_component(
        &self,
        component: Component,
        certified: bool,
    ) -> anyhow::Result<ModuleManifest> {
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;
        // Adds every module-world import; a module only resolves the ones it
        // actually imports (and only certified modules reach the system caps,
        // gated below).
        module_bindings::Module::add_to_linker(&mut linker, |s: &mut HostState| s)?;

        let mut store = Store::new(&self.engine, HostState::new(&self.world));
        let module = module_bindings::Module::instantiate(&mut store, &component, &linker)?;

        let guest = module.quicktys_api_module_guest();
        let manifest: ModuleManifest = serde_json::from_str(&guest.call_init(&mut store)?)?;

        if !certified && !manifest.capabilities.is_empty() {
            anyhow::bail!(
                "module {} requests system capabilities but is not certified",
                manifest.id
            );
        }

        store.data_mut().id = manifest.id.clone();
        store.data_mut().load_kv();
        // Register the instance BEFORE on-load: a module may open a `ws-connect`
        // in on-load, and the reader thread must be able to find the instance to
        // deliver `on-host-event` (it blocks on the same lock we hold here).
        self.world
            .registry
            .lock()
            .unwrap()
            .register_module(manifest.clone(), certified);
        let instance = Arc::new(Mutex::new(ModuleInstance {
            store,
            bindings: module,
        }));
        self.world
            .modules
            .lock()
            .unwrap()
            .insert(manifest.id.clone(), instance.clone());
        instance
            .lock()
            .unwrap()
            .on_load()
            .map_err(|e| anyhow::anyhow!("module {} on-load failed: {e}", manifest.id))?;
        Ok(manifest)
    }

    /// Instantiate a component just far enough to read its `init()` manifest,
    /// **without** registering it, running `on-load`, or applying the
    /// certification gate. Used by the packaging tool to derive a pkg
    /// `manifest.json` (id/kind/version/requires/capabilities) from a built
    /// `.wasm`. Tries the module world first, then the plugin world.
    pub fn inspect(&self, wasm_path: &Path) -> anyhow::Result<(String, Value)> {
        let component = Component::from_file(&self.engine, wasm_path)?;

        // Module world.
        let module_json = (|| -> anyhow::Result<String> {
            let mut linker: Linker<HostState> = Linker::new(&self.engine);
            wasmtime_wasi::add_to_linker_sync(&mut linker)?;
            module_bindings::Module::add_to_linker(&mut linker, |s: &mut HostState| s)?;
            let mut store = Store::new(&self.engine, HostState::new(&self.world));
            let module = module_bindings::Module::instantiate(&mut store, &component, &linker)?;
            module.quicktys_api_module_guest().call_init(&mut store)
        })();
        if let Ok(json) = module_json {
            return Ok(("module".to_string(), serde_json::from_str(&json)?));
        }

        // Plugin world.
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;
        plugin_bindings::Plugin::add_to_linker(&mut linker, |s: &mut HostState| s)?;
        let mut store = Store::new(&self.engine, HostState::new(&self.world));
        let plugin = plugin_bindings::Plugin::instantiate(&mut store, &component, &linker)?;
        let json = plugin.quicktys_api_plugin_guest().call_init(&mut store)?;
        Ok(("plugin".to_string(), serde_json::from_str(&json)?))
    }

    /// Deliver settings to a loaded extension via its `on-settings-changed` hook
    /// (plugin or module). No-op if `id` isn't loaded.
    pub fn apply_settings(&self, id: &str, settings_json: &str) -> Result<(), String> {
        if let Some(inst) = self.world.plugins.lock().unwrap().get(id).cloned() {
            return inst.lock().unwrap().on_settings_changed(settings_json);
        }
        if let Some(inst) = self.world.modules.lock().unwrap().get(id).cloned() {
            return inst.lock().unwrap().on_settings_changed(settings_json);
        }
        Ok(())
    }

    /// Plain HTTP GET, for the marketplace registry client (M6.6). This is a
    /// host facility (not a WASM cap): the app fetches the signed index / blobs
    /// through it. Non-2xx is an error.
    pub fn http_get(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = http::request("GET", url, &[], None)?;
        if !(200..300).contains(&resp.status) {
            return Err(format!("HTTP {} for {url}", resp.status));
        }
        Ok(resp.body)
    }

    pub fn has_plugin(&self, id: &str) -> bool {
        self.world.plugins.lock().unwrap().contains_key(id)
    }

    pub fn has_module(&self, id: &str) -> bool {
        self.world.modules.lock().unwrap().contains_key(id)
    }

    /// The shared extension registry (manifests of loaded plugins/modules).
    /// Hand this to the IPC backend so the UI can list plugins.
    pub fn registry(&self) -> Arc<Mutex<ExtensionRegistry>> {
        self.world.registry.clone()
    }

    /// Begin an OAuth **Device Code** authorization for `provider`, returning the
    /// code to show the user and where to enter it.
    ///
    /// This is the interactive half the WIT `oauth.authorize` cannot express: it
    /// is a synchronous call, so it has nowhere to hand back a user code to
    /// display while we wait. The core therefore drives the flow from the UI
    /// side (Settings → IPC → here), while the module keeps supplying the
    /// provider specifics (ARCHITECTURE §4.6) through its own service. Once the
    /// tokens land, the module reads them through `oauth.access-token` as usual.
    pub fn oauth_start_device(
        &self,
        provider: &str,
        device_url: &str,
        token_url: &str,
        client_id: &str,
        scopes: &str,
    ) -> Result<oauth::DeviceFlow, String> {
        self.world
            .tokens
            .start_device(provider, device_url, token_url, client_id, scopes)
    }

    /// Poll a pending device authorization once.
    pub fn oauth_poll_device(&self, provider: &str) -> Result<oauth::DeviceStatus, String> {
        self.world.tokens.poll_device(provider)
    }

    /// Whether `provider` currently holds a usable (or refreshable) token-set.
    pub fn oauth_is_authorized(&self, provider: &str) -> bool {
        self.world.tokens.is_authorized(provider)
    }

    /// Whether a device authorization is awaiting the user's approval.
    pub fn oauth_device_pending(&self, provider: &str) -> bool {
        self.world.tokens.device_pending(provider)
    }

    /// Drop the tokens (and any pending authorization) for `provider`.
    pub fn oauth_disconnect(&self, provider: &str) {
        self.world.tokens.cancel_device(provider);
        self.world.tokens.revoke(provider);
    }

    /// Call a module service method directly (host-side: diagnostics, or driving
    /// a module without a plugin). Plugins normally reach modules via the runner
    /// → `modules.call` path.
    pub fn call_module(&self, module_id: &str, method: &str, args: Value) -> Result<Value, String> {
        let instance = self.world.modules.lock().unwrap().get(module_id).cloned();
        let Some(instance) = instance else {
            return Err(format!("no such module: {module_id}"));
        };
        let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        let result = instance.lock().unwrap().call(method, &args_json)?;
        self.world.deliver_pending();
        Ok(serde_json::from_str(&result).unwrap_or(Value::Null))
    }

    /// An [`ActionInvoker`] over the loaded plugins, for the core runner.
    pub fn invoker(&self) -> Arc<dyn ActionInvoker> {
        Arc::new(PluginInvoker {
            world: self.world.clone(),
        })
    }
}

/// Routes runner action invocations to the owning plugin, then delivers any
/// module events the action produced.
struct PluginInvoker {
    world: Arc<World>,
}

impl ActionInvoker for PluginInvoker {
    fn invoke<'a>(
        &'a self,
        plugin: &'a str,
        action: &'a str,
        config: Value,
    ) -> BoxFut<'a, ActionResult> {
        let world = self.world.clone();
        let plugin = plugin.to_string();
        let action = action.to_string();
        Box::pin(async move {
            let instance = world.plugins.lock().unwrap().get(&plugin).cloned();
            let Some(instance) = instance else {
                return Err(format!("no such plugin: {plugin}"));
            };
            let result = {
                let mut guard = instance.lock().unwrap();
                guard.invoke(&action, &config, &Value::Null)
            };
            // Deliver module events emitted during the action (lock released).
            world.deliver_pending();
            result
        })
    }
}
