//! Quicktys core runtime — the host-side domain engine.
//!
//! Rust port of `castmate-core`, with all extension-specific code moved OUT into
//! sandboxed WASM components (see the two tiers in ARCHITECTURE.md). This crate
//! owns the cross-cutting machinery that is NOT extension-specific:
//!
//! - `extensions` — registry of installed PLUGINS and MODULES (manifests decoded
//!   from the WASM ABI), dependency resolution (plugin → required modules),
//!   trust/certification metadata, on-demand install/remove bookkeeping.
//! - `resources` — Resource / FileResource registry (Profiles, Automations,
//!   ActionQueues, Accounts, ...), persisted as YAML.
//! - `queues` — action queues (kept from CastMate): sequential execution,
//!   configurable gap, pause/resume, skip, replay, history.
//! - `profile` — profiles, activation conditions, trigger binding.
//! - `sequence` — the automation runner: a tree-walking interpreter over the
//!   `Node` model, with structured concurrency, abort propagation and bounded
//!   loops. Uses `quicktys-expr` for all conditions/values.
//! - `reactivity` — async reactive refs + state propagation to the UI.
//! - `template` — `{{ ... }}` resolution over trigger context + plugin state.
//! - `i18n` — registry of locale bundles discovered from each `lang/` folder
//!   (core + every module/plugin), namespaced by id and pushed to the UI.
//! - `bus` — the trigger event bus (plugins emit, profiles match) and
//!   module-event fan-out to subscribed plugins.
//! - `persist` — YAML load/save helpers and the user-data directory layout.
//!
//! Extension *execution* is delegated to `quicktys-host`; this crate only knows
//! the abstract contracts: a plugin (actions/triggers, requires modules) and a
//! module (services/events, requires system capabilities).

pub mod bus;
pub mod cancel;
pub mod extensions;
pub mod i18n;
pub mod invoker;
pub mod persist;
pub mod profile;
pub mod queues;
pub mod reactivity;
pub mod resources;
pub mod sequence;
mod template;

#[cfg(test)]
mod test_support;

pub use bus::{TriggerBus, TriggerEvent};
pub use cancel::Cancel;
pub use extensions::{Capability, ExtensionRegistry, ModuleManifest, PluginManifest, UiDecl};
pub use i18n::{LocaleRegistry, Namespace};
pub use invoker::{ActionInvoker, ActionResult};
pub use persist::{DataDir, PersistError};
pub use profile::{Automation, Dispatcher, Profile, TriggerBinding};
pub use queues::{ActionQueue, HistoryEntry, JobStatus, QueueConfig, QueuedJob};
pub use reactivity::StateStore;
pub use resources::{Resource, ResourceError, ResourceStore};
pub use sequence::{ProgressSink, RunError, Runner};
