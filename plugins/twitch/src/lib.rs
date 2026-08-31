//! `twitch` plugin — user-facing Twitch actions (send a chat message, make an
//! announcement). Sandboxed: it declares `requires: twitch` and reaches the
//! Helix API through the certified `twitch` module's services; it never touches
//! the network itself.
//!
//! Message fields are **templates** (`@var` / `{{ }}` resolved by the host), so
//! you can post `Now playing {{ pear.title }}`.
//!
//! **Live events.** Chat comes over IRC (anonymous, no account needed) and is
//! mirrored into a rolling `twitch.chat` buffer — what the chat widget renders —
//! plus a `chat-message` trigger. Follow / subscribe / raid come over EventSub,
//! which needs a **connected account**: `watch-events` opens that socket and the
//! three triggers below fire from it.

use quicktys_plugin_sdk::{
    json, ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, TriggerDecl, Value,
};
use std::cell::RefCell;

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};
use quicktys::api::modules::{call as module_call, subscribe as module_subscribe};
use quicktys::api::reactivity::{emit_trigger, set_state};

/// How many messages the widget can scroll back through. Chat is a firehose;
/// the state store holds one JSON value, so this is a deliberate cap rather
/// than an unbounded log.
const BUFFER: usize = 200;

thread_local! {
    /// Newest-last ring of recent messages, republished on every arrival.
    static CHAT: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

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
            // Join a channel's chat so `twitch.chat` starts filling.
            .action(
                ActionDecl::new("watch-chat").config(Schema::object([(
                    "channel".to_string(),
                    Schema::string().named("plugin.twitch.actions.watch-chat.config.channel"),
                )])),
            )
            // EventSub: needs a connected account (see `watch-events`).
            .action(
                ActionDecl::new("watch-events").config(Schema::object([(
                    "channel".to_string(),
                    Schema::string().named("plugin.twitch.actions.watch-events.config.channel"),
                )])),
            )
            .trigger(
                TriggerDecl::new("follow").context(Schema::object([
                    ("user_name".to_string(), Schema::string()),
                    ("user_login".to_string(), Schema::string()),
                ])),
            )
            .trigger(
                TriggerDecl::new("subscribe").context(Schema::object([
                    ("user_name".to_string(), Schema::string()),
                    ("tier".to_string(), Schema::string()),
                    ("is_gift".to_string(), Schema::boolean()),
                ])),
            )
            .trigger(
                TriggerDecl::new("raid").context(Schema::object([
                    ("from_broadcaster_user_name".to_string(), Schema::string()),
                    ("viewers".to_string(), Schema::number()),
                ])),
            )
            .trigger(
                TriggerDecl::new("chat-message").context(Schema::object([
                    ("user".to_string(), Schema::string()),
                    ("displayName".to_string(), Schema::string()),
                    ("text".to_string(), Schema::string()),
                    ("color".to_string(), Schema::string()),
                ])),
            )
            // The rolling buffer the chat widget renders.
            .state_field(StateDecl::new("chat").schema(Schema::array(Schema::object([
                ("user".to_string(), Schema::string()),
                ("displayName".to_string(), Schema::string()),
                ("text".to_string(), Schema::string()),
                ("color".to_string(), Schema::string()),
            ]))))
            .state_field(StateDecl::new("chat-channel").schema(Schema::string()))
            // `set/version` → badge image URL, for the chat widget.
            .state_field(StateDecl::new("badges"))
    }

    fn on_load() -> Result<(), String> {
        for event in ["chat", "follow", "subscribe", "raid"] {
            module_subscribe("twitch", event)
                .map_err(|e| format!("subscribe {event} failed: {e:?}"))?;
        }
        set_state("chat", "[]");
        Ok(())
    }

    /// Chat is buffered *and* triggered; the EventSub events are bridged
    /// straight to their triggers under the same name.
    fn on_module_event(module_id: &str, event_id: &str, payload: Value) {
        if module_id != "twitch" {
            return;
        }
        if matches!(event_id, "follow" | "subscribe" | "raid") {
            emit_trigger(event_id, &payload.to_string());
            return;
        }
        if event_id != "chat" {
            return;
        }
        let list = CHAT.with(|c| {
            let mut c = c.borrow_mut();
            c.push(payload.clone());
            let overflow = c.len().saturating_sub(BUFFER);
            if overflow > 0 {
                c.drain(..overflow);
            }
            Value::Array(c.clone())
        });
        set_state("chat", &list.to_string());
        emit_trigger("chat-message", &payload.to_string());
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        // Open the EventSub socket for a channel. Requires a connected account.
        if action == "watch-events" {
            let channel = config
                .get("channel")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .ok_or_else(|| ActionError::failed("channel is required"))?;
            let args = json!({ "channel": channel }).to_string();
            let result = module_call("twitch", "connect-events", &args).map_err(|e| {
                ActionError::failed(format!(
                    "twitch.connect-events failed (is the account connected?): {e:?}"
                ))
            })?;
            log(Level::Info, &format!("twitch: watching events for #{channel}"));
            return Ok(quicktys_plugin_sdk::serde_json::from_str(&result).unwrap_or(Value::Null));
        }

        // Joining a channel takes no message — handle it before the check below.
        if action == "watch-chat" {
            let channel = config
                .get("channel")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .ok_or_else(|| ActionError::failed("channel is required"))?;
            let args = json!({ "channel": channel }).to_string();
            let result = module_call("twitch", "connect-chat", &args)
                .map_err(|e| ActionError::failed(format!("twitch.connect-chat failed: {e:?}")))?;
            set_state("chat-channel", &json!(channel).to_string());
            // Badge images need a Helix call, so they only work once an account
            // is connected. Never fail the join over it — the widget falls back
            // to text pills when the map is empty.
            match module_call("twitch", "chat-badges", &args) {
                Ok(map) => set_state("badges", &map),
                Err(e) => {
                    log(Level::Debug, &format!("twitch: no badge images ({e:?})"));
                    set_state("badges", "{}");
                }
            }
            log(Level::Info, &format!("twitch: watching #{channel}"));
            return Ok(quicktys_plugin_sdk::serde_json::from_str(&result).unwrap_or(Value::Null));
        }

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
