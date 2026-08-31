//! `time` plugin — the end-to-end reference plugin.
//!
//! Validates the whole plugin ABI: the host loads this component, reads its
//! manifest from `init`, invokes the `delay` action, and observes the state it
//! publishes through the `reactivity` host capability. Pure logic — no module
//! required.

use quicktys_plugin_sdk::{ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, Value};

// Generate the `plugin` world bindings (host imports + the `plugin-guest`
// export trait + the `export!` macro) directly in this cdylib crate.
wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};
use quicktys::api::reactivity::set_state;

/// The plugin's logic, written against the ergonomic SDK trait.
struct TimePlugin;

impl Plugin for TimePlugin {
    fn manifest() -> Manifest {
        Manifest::new("time", "Time", "0.1.0")
            .icon("mdi mdi-clock")
            .color("#8DC1A9")
            .action(ActionDecl::new("delay").config(duration_config()))
            .state_field(StateDecl::new("last_delay").schema(duration_schema()))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            "delay" => {
                let seconds = config.get("duration").and_then(Value::as_f64).unwrap_or(0.0);
                log(Level::Info, &format!("time.delay: {seconds}s"));
                // Publish the last delay as reactive state (a JSON number).
                set_state("last_delay", &quicktys_plugin_sdk::json!(seconds).to_string());
                Ok(quicktys_plugin_sdk::json!({ "delayed": seconds }))
            }
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }
}

fn duration_schema() -> Schema {
    Schema::Duration {
        common: Default::default(),
        default: None,
        min: Some(0.0),
        max: None,
        template: true,
    }
}

fn duration_config() -> Schema {
    Schema::object([("duration".to_string(), duration_schema())])
}

// Wire the generated `plugin-guest` export to the `Plugin` impl above.
quicktys_plugin_sdk::define_plugin!(TimePlugin);
