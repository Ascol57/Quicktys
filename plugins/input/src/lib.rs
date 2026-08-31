//! `input` plugin — type text, press a hotkey, or click the mouse. Routes
//! through the certified `input` module.

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::call as module_call;

struct InputPlugin;

impl Plugin for InputPlugin {
    fn manifest() -> Manifest {
        Manifest::new("input", "Input", "0.1.0")
            .icon("mdi mdi-keyboard")
            .color("#9a5fd0")
            .requires("input", "^0")
            .action(ActionDecl::new("type-text").config(Schema::object([(
                "text".to_string(),
                Schema::template_string().named("plugin.input.fields.text"),
            )])))
            // A hotkey like "ctrl+shift+c".
            .action(ActionDecl::new("hotkey").config(Schema::object([(
                "keys".to_string(),
                Schema::string().named("plugin.input.fields.keys"),
            )])))
            .action(ActionDecl::new("mouse-click").config(Schema::object([(
                "button".to_string(),
                Schema::string()
                    .named("plugin.input.fields.button")
                    .options(["left", "middle", "right"]),
            )])))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let s = |k: &str| config.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let (service, args) = match action {
            "type-text" => ("type", json!({ "text": s("text") })),
            "hotkey" => {
                // "ctrl+shift+c" → ["ctrl","shift","c"].
                let raw = s("keys");
                let keys: Vec<String> = raw
                    .split('+')
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                    .map(str::to_string)
                    .collect();
                ("key-combo", json!({ "keys": keys }))
            }
            "mouse-click" => {
                let button = match s("button").as_str() {
                    "middle" => 1,
                    "right" => 2,
                    _ => 0,
                };
                ("mouse-click", json!({ "button": button }))
            }
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        };
        let json = quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        module_call("input", service, &json)
            .map_err(|e| ActionError::failed(format!("input.{service} failed: {e:?}")))?;
        Ok(Value::Null)
    }
}

quicktys_plugin_sdk::define_plugin!(InputPlugin);
