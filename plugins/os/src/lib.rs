//! `os` plugin — user-facing OS actions (run a shell command, open a file/URL,
//! launch a program). Routes through the certified `os` module.

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::call as module_call;

fn tf(key: &str) -> Schema {
    Schema::template_string().named(&format!("plugin.os.fields.{key}"))
}

struct OsPlugin;

impl Plugin for OsPlugin {
    fn manifest() -> Manifest {
        Manifest::new("os", "Operating System", "0.1.0")
            .icon("mdi mdi-console")
            .color("#5fa8d3")
            .requires("os", "^0")
            .action(
                ActionDecl::new("shell").config(Schema::object([("command".to_string(), tf("command"))])),
            )
            .action(ActionDecl::new("open").config(Schema::object([("target".to_string(), tf("target"))])))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let s = |k: &str| config.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let (service, args) = match action {
            "shell" => ("shell", json!({ "command": s("command") })),
            "open" => ("open", json!({ "target": s("target") })),
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        };
        let json = quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        module_call("os", service, &json)
            .map_err(|e| ActionError::failed(format!("os.{service} failed: {e:?}")))?;
        Ok(Value::Null)
    }
}

quicktys_plugin_sdk::define_plugin!(OsPlugin);
