//! `discord` plugin — send a message to a Discord channel via an **incoming
//! webhook**. No Discord module: the plugin has no network access, so it formats
//! the webhook payload and sends it through the generic certified `http` module
//! (a POST of `{content, username?, avatar_url?}` to the webhook URL).

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::call as module_call;
use std::cell::RefCell;

// The default webhook URL, from the plugin's settings (used when an action leaves
// its `url` blank). Set via `on_settings_changed`.
thread_local!(static WEBHOOK: RefCell<String> = const { RefCell::new(String::new()) });

struct Discord;

impl Plugin for Discord {
    fn manifest() -> Manifest {
        Manifest::new("discord", "Discord", "0.1.0")
            .icon("mdi mdi-discord")
            .color("#5865f2")
            .requires("http", "^0")
            // User-facing settings: a default webhook URL, offered right after
            // install (post-install onboarding).
            .settings(Schema::object([(
                "webhook".to_string(),
                Schema::string().named("plugin.discord.settings.webhook"),
            )]))
            .action(ActionDecl::new("webhook").config(Schema::object([
                ("url".to_string(), Schema::string().named("plugin.discord.fields.url")),
                (
                    "content".to_string(),
                    Schema::template_string().named("plugin.discord.fields.content"),
                ),
                (
                    "username".to_string(),
                    Schema::template_string().named("plugin.discord.fields.username"),
                ),
                (
                    "avatar".to_string(),
                    Schema::string().named("plugin.discord.fields.avatar"),
                ),
            ])))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        if action != "webhook" {
            return Err(ActionError::failed(format!("unknown action: {action}")));
        }
        let s = |k: &str| config.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
        // The action's URL, or the default from settings if left blank.
        let mut url = s("url");
        if url.is_empty() {
            url = WEBHOOK.with(|w| w.borrow().clone());
        }
        if url.is_empty() {
            return Err(ActionError::failed("webhook URL is required (set one in the plugin settings)".to_string()));
        }

        // Build the Discord payload, omitting empty optional fields.
        let mut payload = quicktys_plugin_sdk::serde_json::Map::new();
        payload.insert("content".into(), Value::String(s("content")));
        if !s("username").is_empty() {
            payload.insert("username".into(), Value::String(s("username")));
        }
        if !s("avatar").is_empty() {
            payload.insert("avatar_url".into(), Value::String(s("avatar")));
        }

        let req = json!({
            "method": "POST",
            "url": url,
            "headers": { "Content-Type": "application/json" },
            "body": Value::Object(payload).to_string(),
        });
        let args = quicktys_plugin_sdk::serde_json::to_string(&req).unwrap_or_else(|_| "null".into());
        let resp = module_call("http", "request", &args)
            .map_err(|e| ActionError::failed(format!("discord webhook failed: {e:?}")))?;
        // Discord returns 204 on success; surface a 4xx/5xx as an error.
        let status = quicktys_plugin_sdk::serde_json::from_str::<Value>(&resp)
            .ok()
            .and_then(|v| v.get("status").and_then(Value::as_u64))
            .unwrap_or(0);
        if status >= 400 {
            return Err(ActionError::failed(format!("discord webhook returned {status}")));
        }
        Ok(Value::Null)
    }

    fn on_settings_changed(settings: Value) {
        let webhook = settings.get("webhook").and_then(Value::as_str).unwrap_or("").to_string();
        WEBHOOK.with(|w| *w.borrow_mut() = webhook);
    }
}

quicktys_plugin_sdk::define_plugin!(Discord);
