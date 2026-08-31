//! `counter-client` plugin — consumes the `counter` module.
//!
//! Validates the two-tier contract: its `bump` action calls the counter
//! module's `add` service through the host `modules` interface, and it
//! subscribes to the module's `changed` event, mirroring the latest total into
//! reactive state when the event is delivered to `on_module_event`.

use quicktys_plugin_sdk::{ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::{call as module_call, subscribe as module_subscribe};
use quicktys::api::reactivity::set_state;

struct CounterClient;

impl Plugin for CounterClient {
    fn manifest() -> Manifest {
        Manifest::new("counter-client", "Counter Client", "0.1.0")
            .requires("counter", "^0")
            .action(ActionDecl::new("bump").config(Schema::object([(
                "amount".to_string(),
                Schema::number(),
            )])))
            .state_field(StateDecl::new("last_total").schema(Schema::number()))
    }

    fn on_load() -> Result<(), String> {
        // Subscribe to the module's event; deliveries arrive at on_module_event.
        module_subscribe("counter", "changed").map_err(|e| format!("subscribe failed: {e:?}"))?;
        Ok(())
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            "bump" => {
                let amount = config
                    .get("amount")
                    .cloned()
                    .unwrap_or_else(|| quicktys_plugin_sdk::json!(0));
                let args = quicktys_plugin_sdk::json!({ "amount": amount }).to_string();
                let result = module_call("counter", "add", &args)
                    .map_err(|e| ActionError::failed(format!("counter.add failed: {e:?}")))?;
                Ok(quicktys_plugin_sdk::serde_json::from_str(&result).unwrap_or(Value::Null))
            }
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }

    fn on_module_event(module_id: &str, event_id: &str, payload: Value) {
        if module_id == "counter" && event_id == "changed" {
            if let Some(total) = payload.get("total") {
                set_state("last_total", &total.to_string());
            }
        }
    }
}

quicktys_plugin_sdk::define_plugin!(CounterClient);
