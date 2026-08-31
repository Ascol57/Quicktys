//! Quicktys plugin SDK (guest side) — for the **anyone-can-write** tier.
//!
//! Ergonomic Rust layer over the raw `plugin` WIT world so plugin authors write
//! idiomatic code instead of hand-marshalling JSON. Target `wasm32-wasip2`; the
//! output is a Component Model component.
//!
//! A plugin is sandboxed: NO direct network/OS access. To reach the outside it
//! declares `requires: [obs@^1]` and calls the OBS **module**'s services through
//! the host `modules` interface.
//!
//! ## Authoring a plugin
//! Implement [`Plugin`] on a unit type, then hand it to the [`define_plugin!`]
//! macro, which runs `wit_bindgen::generate!`, wires the generated `plugin-guest`
//! export to your impl (JSON marshalling included), and emits the component
//! export shims. See `plugins/time` for a worked example.
//!
//! The schema types come from [`quicktys_schema`] (pure, compiles to wasm), so
//! the manifest a plugin emits validates against the very same schema model the
//! host uses.

use serde::Serialize;
pub use serde_json::{self, json, Value};

pub use quicktys_schema::Schema;

/// The contract a plugin implements. All methods are associated functions: a
/// plugin instance is a singleton from the component's point of view.
pub trait Plugin {
    /// The manifest returned by `plugin-guest.init` (id, name, declarations).
    fn manifest() -> Manifest;

    /// Called once after instantiation. Open resources here.
    fn on_load() -> Result<(), String> {
        Ok(())
    }

    /// Called before the plugin is dropped.
    fn on_unload() {}

    /// Run an action. `config`/`context` are JSON the host already validated
    /// against the declared schemas.
    fn invoke_action(action: &str, config: Value, context: Value) -> Result<Value, ActionError>;

    /// The plugin's settings changed.
    fn on_settings_changed(_settings: Value) {}

    /// A module event the plugin subscribed to was delivered.
    fn on_module_event(_module_id: &str, _event_id: &str, _payload: Value) {}
}

/// Why an action failed.
#[derive(Debug, Clone)]
pub enum ActionError {
    /// The action was aborted (cancellation) — the host won't treat it as an
    /// error to report.
    Aborted,
    /// The action failed with a message.
    Failed(String),
}

impl ActionError {
    pub fn failed(message: impl Into<String>) -> Self {
        ActionError::Failed(message.into())
    }
}

// ---------------------------------------------------------------------------
// Manifest — serializes to the JSON the host decodes (mirrors the host's
// `extensions::PluginManifest`). Field names/shape MUST stay in sync.
// ---------------------------------------------------------------------------

/// A plugin's declared contract.
#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub actions: Vec<ActionDecl>,
    pub triggers: Vec<TriggerDecl>,
    pub state: Vec<StateDecl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Schema>,
    #[serde(rename = "requires")]
    pub required_modules: Vec<ModuleRequirement>,
    /// UI contributions injected into named interface slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui: Vec<UiDecl>,
}

/// A plugin's contribution to a named UI slot (e.g. `webview.toolbar`). Two
/// shapes: a **button** (built-in — `label`/`icon`, runs `action` on click with
/// the slot's context merged into `config`), or **custom** UI (`entry` = an HTML
/// file the plugin ships, rendered in a sandboxed iframe fed the same context).
#[derive(Debug, Clone, Serialize)]
pub struct UiDecl {
    pub slot: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

impl UiDecl {
    /// A built-in button in `slot` that runs the plugin's `action` on click.
    pub fn button(slot: impl Into<String>, action: impl Into<String>) -> Self {
        UiDecl {
            slot: slot.into(),
            label: None,
            icon: None,
            action: Some(action.into()),
            config: None,
            entry: None,
        }
    }
    /// A custom UI fragment (an HTML file shipped by the plugin) in `slot`.
    pub fn custom(slot: impl Into<String>, entry: impl Into<String>) -> Self {
        UiDecl {
            slot: slot.into(),
            label: None,
            icon: None,
            action: None,
            config: None,
            entry: Some(entry.into()),
        }
    }
    pub fn label(mut self, key: impl Into<String>) -> Self {
        self.label = Some(key.into());
        self
    }
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn config(mut self, config: serde_json::Value) -> Self {
        self.config = Some(config);
        self
    }
}

impl Manifest {
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Manifest {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            icon: None,
            color: None,
            actions: Vec::new(),
            triggers: Vec::new(),
            state: Vec::new(),
            settings: None,
            secrets: None,
            required_modules: Vec::new(),
            ui: Vec::new(),
        }
    }

    /// Add a UI contribution (a button or custom fragment) to a named slot.
    pub fn ui(mut self, ui: UiDecl) -> Self {
        self.ui.push(ui);
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
    pub fn action(mut self, action: ActionDecl) -> Self {
        self.actions.push(action);
        self
    }
    pub fn trigger(mut self, trigger: TriggerDecl) -> Self {
        self.triggers.push(trigger);
        self
    }
    pub fn state_field(mut self, state: StateDecl) -> Self {
        self.state.push(state);
        self
    }
    pub fn requires(mut self, module: impl Into<String>, version: impl Into<String>) -> Self {
        self.required_modules.push(ModuleRequirement {
            module: module.into(),
            version: version.into(),
        });
        self
    }

    /// Declare the plugin's user-facing settings schema. The host renders a form
    /// for it (post-install onboarding + Settings), persists the values, and
    /// delivers them via `on_settings_changed`.
    pub fn settings(mut self, schema: Schema) -> Self {
        self.settings = Some(schema);
        self
    }

    /// Serialize to the manifest JSON string returned from `init`.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("manifest serialization cannot fail")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionDecl {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Schema>,
}

impl ActionDecl {
    pub fn new(id: impl Into<String>) -> Self {
        ActionDecl {
            id: id.into(),
            config: None,
            result: None,
        }
    }
    pub fn config(mut self, schema: Schema) -> Self {
        self.config = Some(schema);
        self
    }
    pub fn result(mut self, schema: Schema) -> Self {
        self.result = Some(schema);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerDecl {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Schema>,
}

impl TriggerDecl {
    pub fn new(id: impl Into<String>) -> Self {
        TriggerDecl {
            id: id.into(),
            context: None,
        }
    }
    /// The JSON shape of the context an occurrence carries (fields the bound
    /// automation can reference, e.g. `title`, `artist`).
    pub fn context(mut self, schema: Schema) -> Self {
        self.context = Some(schema);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StateDecl {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
}

impl StateDecl {
    pub fn new(id: impl Into<String>) -> Self {
        StateDecl {
            id: id.into(),
            schema: None,
        }
    }
    pub fn schema(mut self, schema: Schema) -> Self {
        self.schema = Some(schema);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleRequirement {
    pub module: String,
    pub version: String,
}

/// Wire a [`Plugin`] impl to the generated `plugin-guest` export and emit the
/// component export shims.
///
/// Invoke at the crate root **after** `wit_bindgen::generate!({ world: "plugin",
/// .. })`, so the generated `exports::…` paths and `export!` macro are in scope:
///
/// ```ignore
/// wit_bindgen::generate!({ path: "../../wit", world: "plugin" });
/// struct MyPlugin;
/// impl quicktys_plugin_sdk::Plugin for MyPlugin { /* ... */ }
/// quicktys_plugin_sdk::define_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! define_plugin {
    ($plugin:ty) => {
        #[doc(hidden)]
        struct __QuicktysPluginComponent;

        impl exports::quicktys::api::plugin_guest::Guest for __QuicktysPluginComponent {
            fn init() -> ::std::string::String {
                <$plugin as $crate::Plugin>::manifest().to_json()
            }

            fn on_load() -> ::std::result::Result<(), ::std::string::String> {
                <$plugin as $crate::Plugin>::on_load()
            }

            fn on_unload() {
                <$plugin as $crate::Plugin>::on_unload()
            }

            fn invoke_action(
                action_id: ::std::string::String,
                config_json: ::std::string::String,
                context_json: ::std::string::String,
            ) -> ::std::result::Result<
                ::std::string::String,
                exports::quicktys::api::plugin_guest::InvokeError,
            > {
                let config = $crate::serde_json::from_str(&config_json)
                    .unwrap_or($crate::Value::Null);
                let context = $crate::serde_json::from_str(&context_json)
                    .unwrap_or($crate::Value::Null);
                match <$plugin as $crate::Plugin>::invoke_action(&action_id, config, context) {
                    ::std::result::Result::Ok(value) => ::std::result::Result::Ok(
                        $crate::serde_json::to_string(&value)
                            .unwrap_or_else(|_| ::std::string::String::from("null")),
                    ),
                    ::std::result::Result::Err($crate::ActionError::Aborted) => {
                        ::std::result::Result::Err(
                            exports::quicktys::api::plugin_guest::InvokeError::Aborted,
                        )
                    }
                    ::std::result::Result::Err($crate::ActionError::Failed(message)) => {
                        ::std::result::Result::Err(
                            exports::quicktys::api::plugin_guest::InvokeError::Failed(message),
                        )
                    }
                }
            }

            fn on_settings_changed(settings_json: ::std::string::String) {
                let settings = $crate::serde_json::from_str(&settings_json)
                    .unwrap_or($crate::Value::Null);
                <$plugin as $crate::Plugin>::on_settings_changed(settings);
            }

            fn on_module_event(
                module_id: ::std::string::String,
                event_id: ::std::string::String,
                payload_json: ::std::string::String,
            ) {
                let payload = $crate::serde_json::from_str(&payload_json)
                    .unwrap_or($crate::Value::Null);
                <$plugin as $crate::Plugin>::on_module_event(&module_id, &event_id, payload);
            }
        }

        export!(__QuicktysPluginComponent);
    };
}
