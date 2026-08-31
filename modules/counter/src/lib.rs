//! `counter` module — a self-contained reference driver used to validate the
//! two-tier ABI (plugin → module call + module → plugin event).
//!
//! It uses no system capability: it keeps a running total in its host `storage`
//! and emits a `changed` event whenever the total moves. A plugin calls its
//! `add` service and subscribes to `changed`.

use quicktys_module_sdk::{EventDecl, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::events::emit_event;
use quicktys::api::storage::{kv_get, kv_set};

struct Counter;

impl Module for Counter {
    fn manifest() -> ModuleManifest {
        let total = || Schema::object([("total".to_string(), Schema::number())]);
        ModuleManifest::new("counter", "Counter", "0.1.0")
            .service(
                ServiceDecl::new("add")
                    .args(Schema::object([("amount".to_string(), Schema::number())]))
                    .result(total()),
            )
            .event(EventDecl::new("changed").payload(total()))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "add" => {
                let amount = args.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
                let previous = kv_get("total")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let total = previous + amount;
                kv_set("total", &total.to_string());
                // Notify subscribers (plugins) that the total changed.
                emit_event(
                    "changed",
                    &quicktys_module_sdk::json!({ "total": total }).to_string(),
                );
                Ok(quicktys_module_sdk::json!({ "total": total }))
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Counter);
