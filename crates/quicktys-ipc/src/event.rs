//! The host → UI event catalog and the sink that delivers it.
//!
//! Events are plain serializable values; `quicktys-app` provides an
//! [`EventSink`] backed by Tauri's `Window::emit`, while tests use a collecting
//! sink. This keeps the backend testable without a WebView.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The kinds of editable resource exposed over IPC (mirrors CastMate's
/// `resources_*`/`profiles_*`/`automations_*`/`queues_*` channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Profile,
    Automation,
    Queue,
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ResourceKind::Profile => "profile",
            ResourceKind::Automation => "automation",
            ResourceKind::Queue => "queue",
        }
    }
}

/// A host → UI event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppEvent {
    /// A reactive state value changed.
    StateChanged { key: String, value: Value },
    /// A resource was created.
    ResourceAdded {
        kind: ResourceKind,
        id: String,
        value: Value,
    },
    /// A resource was modified.
    ResourceUpdated {
        kind: ResourceKind,
        id: String,
        value: Value,
    },
    /// A resource was deleted.
    ResourceRemoved { kind: ResourceKind, id: String },
    /// A plugin was registered (loaded).
    PluginRegistered { id: String },
    /// The set of installed extensions changed (install/uninstall at runtime) —
    /// the UI should re-fetch `list_plugins`/`list_modules`/`list_widgets`.
    ExtensionsChanged,
    /// Boot-time extension loading progress: `id` (a `module`/`plugin`) is the
    /// `index`-th of `total` to load. Drives the startup loading screen.
    BootProgress {
        index: usize,
        total: usize,
        id: String,
        kind: String,
    },
    /// All boot-time extensions finished loading — hide the loading screen and
    /// fetch the initial lists.
    BootReady,
    /// A module's capability consent changed (grant/revoke) — the UI should
    /// re-fetch `consent_status`.
    ConsentChanged,
    /// The active UI locale changed.
    LocaleChanged { locale: String },
    /// A transient notification to show in the UI (from a plugin's `notify`).
    Toast { message: String, level: String },
    /// Live per-node execution telemetry during an automation run: a node in
    /// `automation`, addressed by index `path` from the root sequence, started
    /// (`running = true`) or finished (`running = false`). Drives the editor's
    /// real-time "now running" highlight.
    NodeProgress {
        automation: String,
        path: Vec<usize>,
        running: bool,
    },
}

/// Delivers [`AppEvent`]s to the UI.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: AppEvent);
}

/// Drops every event (useful for headless/boot phases).
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: AppEvent) {}
}
