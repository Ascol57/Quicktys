//! `input` module — certified driver for synthetic keyboard / mouse input
//! (cross-platform, host-side via a synthesis backend). Holds the `input`
//! capability; the `input` plugin exposes type-text / hotkey / click actions.

use quicktys_module_sdk::{Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::input::{key_combo, mouse_click, type_text};

struct Input;

impl Module for Input {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("input", "Input", "0.1.0")
            .capability(Capability::Input)
            .service(ServiceDecl::new("type").args(Schema::object([(
                "text".to_string(),
                Schema::string(),
            )])))
            .service(ServiceDecl::new("key-combo").args(Schema::object([(
                "keys".to_string(),
                Schema::array(Schema::string()),
            )])))
            .service(ServiceDecl::new("mouse-click").args(Schema::object([(
                "button".to_string(),
                Schema::number(),
            )])))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "type" => {
                let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
                type_text(text)?;
                Ok(Value::Null)
            }
            "key-combo" => {
                let keys: Vec<String> = args
                    .get("keys")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                key_combo(&keys)?;
                Ok(Value::Null)
            }
            "mouse-click" => {
                let button = args.get("button").and_then(Value::as_u64).unwrap_or(0) as u8;
                mouse_click(button)?;
                Ok(Value::Null)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Input);
