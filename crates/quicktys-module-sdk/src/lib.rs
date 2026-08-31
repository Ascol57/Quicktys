//! Quicktys module SDK (guest side) — for the **certified** driver tier.
//!
//! A module is a driver: "how to talk to OBS / Twitch / the audio device / the
//! keyboard". Unlike a plugin it receives **system capabilities** (net, input,
//! audio, hid, files, process, timers) — but only after the host verifies its
//! certification. It exposes typed **services** (request/response methods) and
//! **events**, consumed by plugins through the host `modules` interface.
//!
//! ## Authoring a module
//! Implement [`Module`] on a unit type, then hand it to [`define_module!`], which
//! runs `wit_bindgen::generate!` for the `module` world, wires the generated
//! `module-guest` export to your impl, and emits the component export shims.

use serde::Serialize;
pub use serde_json::{self, json, Value};

pub use quicktys_schema::Schema;

/// The contract a module implements (associated functions: one instance).
pub trait Module {
    /// The manifest returned by `module-guest.init`.
    fn manifest() -> ModuleManifest;

    /// Called once after instantiation. Open connections here (the host has
    /// granted the declared capabilities by now).
    fn on_load() -> Result<(), String> {
        Ok(())
    }

    fn on_unload() {}

    /// A plugin called one of this module's service methods. `args` is JSON the
    /// host validated against the service's declared schema.
    fn call(method: &str, args: Value) -> Result<Value, String>;

    fn on_settings_changed(_settings: Value) {}

    /// A low-level event from a host capability the module owns (socket data,
    /// timer tick, HID report, ...). Turn it into a service event via the
    /// generated `events::emit_event`.
    fn on_host_event(_event_id: &str, _payload: Value) {}
}

// ---------------------------------------------------------------------------
// Manifest — serializes to the JSON the host decodes (mirrors the host's
// `extensions::ModuleManifest`). Field names/shape MUST stay in sync.
// ---------------------------------------------------------------------------

/// A module's declared contract.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub services: Vec<ServiceDecl>,
    pub events: Vec<EventDecl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Schema>,
    /// System capabilities this module requests (granted only when certified).
    pub capabilities: Vec<Capability>,
    /// A full app view/tab this module contributes (its own bundled page).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewDecl>,
}

/// A navigation tab + page the module contributes: an HTML `entry` in its bundle
/// that the app renders (served over `moduleui://<id>/…`) and bridges so the page
/// can call this module's own services.
#[derive(Debug, Clone, Serialize)]
pub struct ViewDecl {
    pub icon: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub entry: String,
}

impl ModuleManifest {
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        ModuleManifest {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            services: Vec::new(),
            events: Vec::new(),
            settings: None,
            secrets: None,
            capabilities: Vec::new(),
            view: None,
        }
    }

    /// Contribute an app view/tab: `entry` is an HTML file in the module bundle,
    /// `label` an i18n key (falls back to the module name).
    pub fn view(
        mut self,
        icon: impl Into<String>,
        label: Option<String>,
        entry: impl Into<String>,
    ) -> Self {
        self.view = Some(ViewDecl {
            icon: icon.into(),
            label,
            entry: entry.into(),
        });
        self
    }

    pub fn service(mut self, service: ServiceDecl) -> Self {
        self.services.push(service);
        self
    }
    pub fn event(mut self, event: EventDecl) -> Self {
        self.events.push(event);
        self
    }
    pub fn capability(mut self, cap: Capability) -> Self {
        self.capabilities.push(cap);
        self
    }

    /// Declare the module's user-facing settings schema (rendered by the host,
    /// persisted, and delivered via `on_settings_changed`).
    pub fn settings(mut self, schema: Schema) -> Self {
        self.settings = Some(schema);
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("manifest serialization cannot fail")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceDecl {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Schema>,
}

impl ServiceDecl {
    pub fn new(id: impl Into<String>) -> Self {
        ServiceDecl {
            id: id.into(),
            args: None,
            result: None,
        }
    }
    pub fn args(mut self, schema: Schema) -> Self {
        self.args = Some(schema);
        self
    }
    pub fn result(mut self, schema: Schema) -> Self {
        self.result = Some(schema);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EventDecl {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Schema>,
}

impl EventDecl {
    pub fn new(id: impl Into<String>) -> Self {
        EventDecl {
            id: id.into(),
            payload: None,
        }
    }
    pub fn payload(mut self, schema: Schema) -> Self {
        self.payload = Some(schema);
        self
    }
}

/// System capabilities a module may request. Serializes lowercase to match the
/// host's capability model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    Net,
    Input,
    Audio,
    Hid,
    Files,
    Process,
    Timers,
}

/// Wire a [`Module`] impl to the generated `module-guest` export and emit the
/// component export shims. Invoke at the crate root after
/// `wit_bindgen::generate!({ world: "module", .. })`.
#[macro_export]
macro_rules! define_module {
    ($module:ty) => {
        #[doc(hidden)]
        struct __QuicktysModuleComponent;

        impl exports::quicktys::api::module_guest::Guest for __QuicktysModuleComponent {
            fn init() -> ::std::string::String {
                <$module as $crate::Module>::manifest().to_json()
            }

            fn on_load() -> ::std::result::Result<(), ::std::string::String> {
                <$module as $crate::Module>::on_load()
            }

            fn on_unload() {
                <$module as $crate::Module>::on_unload()
            }

            fn call(
                method: ::std::string::String,
                args_json: ::std::string::String,
            ) -> ::std::result::Result<::std::string::String, ::std::string::String> {
                let args = $crate::serde_json::from_str(&args_json).unwrap_or($crate::Value::Null);
                match <$module as $crate::Module>::call(&method, args) {
                    ::std::result::Result::Ok(value) => ::std::result::Result::Ok(
                        $crate::serde_json::to_string(&value)
                            .unwrap_or_else(|_| ::std::string::String::from("null")),
                    ),
                    ::std::result::Result::Err(message) => ::std::result::Result::Err(message),
                }
            }

            fn on_settings_changed(settings_json: ::std::string::String) {
                let settings =
                    $crate::serde_json::from_str(&settings_json).unwrap_or($crate::Value::Null);
                <$module as $crate::Module>::on_settings_changed(settings);
            }

            fn on_host_event(event_id: ::std::string::String, payload_json: ::std::string::String) {
                let payload =
                    $crate::serde_json::from_str(&payload_json).unwrap_or($crate::Value::Null);
                <$module as $crate::Module>::on_host_event(&event_id, payload);
            }
        }

        export!(__QuicktysModuleComponent);
    };
}
