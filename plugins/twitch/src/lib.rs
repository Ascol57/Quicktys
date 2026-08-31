//! `twitch` plugin — user-facing Twitch actions (send a chat message, make an
//! announcement). Sandboxed: it declares `requires: twitch` and reaches the
//! Helix API through the certified `twitch` module's services; it never touches
//! the network itself.
//!
//! Message fields are **templates** (`@var` / `{{ }}` resolved by the host), so
//! you can post `Now playing {{ pear.title }}`.
//!
//! Triggers (chat/follow/sub via EventSub) are intentionally **not** declared
//! yet: live delivery needs the host's streaming `ws-connect → on-host-event`
//! path, which isn't wired. Adding a dead trigger would mislead, so this ships
//! the request/response half (actions) only.

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::call as module_call;

struct TwitchPlugin;

impl Plugin for TwitchPlugin {
    fn manifest() -> Manifest {
        Manifest::new("twitch", "Twitch", "0.1.0")
            .icon("mdi mdi-twitch")
            .color("#9146ff")
            .requires("twitch", "^0")
            .action(
                ActionDecl::new("chat")
                    .config(Schema::object([(
                        "message".to_string(),
                        Schema::template_string(),
                    )])),
            )
            .action(ActionDecl::new("announce").config(Schema::object([
                ("message".to_string(), Schema::template_string()),
                ("color".to_string(), Schema::string()),
            ])))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let message = config
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if message.is_empty() {
            return Err(ActionError::failed("message is empty"));
        }

        let (method, args) = match action {
            "chat" => ("send-chat", json!({ "message": message })),
            "announce" => (
                "announce",
                json!({
                    "message": message,
                    "color": config.get("color").cloned().unwrap_or(json!("primary")),
                }),
            ),
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        };

        let args_json =
            quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        let result = module_call("twitch", method, &args_json)
            .map_err(|e| ActionError::failed(format!("twitch.{method} failed: {e:?}")))?;
        Ok(quicktys_plugin_sdk::serde_json::from_str(&result).unwrap_or(Value::Null))
    }
}

quicktys_plugin_sdk::define_plugin!(TwitchPlugin);
