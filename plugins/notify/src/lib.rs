//! `notify` plugin — write a log line, or show a toast in the UI.
//!
//! Both actions take a **template** `message` (`@var` / `{{ }}` resolved by the
//! host before invoke). `log` uses the safe `logging` capability; `toast` uses
//! `reactivity.notify`, which the app forwards to the WebView as a toast.

use quicktys_plugin_sdk::{ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};
use quicktys::api::reactivity::notify;

struct NotifyPlugin;

fn log_level(level: &str) -> Level {
    match level {
        "trace" => Level::Trace,
        "debug" => Level::Debug,
        "warn" | "warning" => Level::Warn,
        "error" => Level::Error,
        _ => Level::Info,
    }
}

impl Plugin for NotifyPlugin {
    fn manifest() -> Manifest {
        let fields = || {
            Schema::object([
                ("message".to_string(), Schema::template_string()),
                ("level".to_string(), Schema::string()),
            ])
        };
        Manifest::new("notify", "Notify", "0.1.0")
            .icon("mdi mdi-bell")
            .color("#e0a458")
            .action(ActionDecl::new("log").config(fields()))
            .action(ActionDecl::new("toast").config(fields()))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let message = config
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let level = config.get("level").and_then(Value::as_str).unwrap_or("info");
        match action {
            "log" => log(log_level(level), &message),
            "toast" => notify(&message, level),
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        }
        Ok(Value::Null)
    }
}

quicktys_plugin_sdk::define_plugin!(NotifyPlugin);
