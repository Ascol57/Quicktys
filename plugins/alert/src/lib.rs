//! `alert` plugin — fire a named **alert box** (an overlay/dashboard widget).
//!
//! The `show` action targets an alert widget by its `name` (offered as
//! autocomplete in the editor, sourced from the alert widgets the user has
//! placed) and pushes a `{title, content, image}` payload to it by setting the
//! reactive state key `alert.<name>`. The matching `alert` widget watches that
//! key and animates the box in. `title`/`content`/`image` are templates
//! (`@var` / `{{ }}`), resolved by the host before invoke.

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::reactivity::set_state;
use std::cell::RefCell;

thread_local! {
    /// Monotonic fire id, so the widget re-triggers even on identical content.
    static SEQ: RefCell<u64> = const { RefCell::new(0) };
}

struct AlertPlugin;

impl Plugin for AlertPlugin {
    fn manifest() -> Manifest {
        Manifest::new("alert", "Alerts", "0.1.0")
            .icon("mdi mdi-bell-ring")
            .color("#d3695f")
            .action(
                ActionDecl::new("show").config(Schema::object([
                    (
                        "name".to_string(),
                        Schema::string()
                            .suggest("alert")
                            .named("plugin.alert.actions.show.config.name"),
                    ),
                    (
                        "title".to_string(),
                        Schema::template_string().named("plugin.alert.actions.show.config.title"),
                    ),
                    (
                        "content".to_string(),
                        Schema::template_string().named("plugin.alert.actions.show.config.content"),
                    ),
                    (
                        "image".to_string(),
                        Schema::template_string().named("plugin.alert.actions.show.config.image"),
                    ),
                ])),
            )
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            "show" => {
                let name = config
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if name.is_empty() {
                    return Err(ActionError::failed("alert name is required".to_string()));
                }
                let field = |k: &str| config.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let seq = SEQ.with(|s| {
                    let mut s = s.borrow_mut();
                    *s += 1;
                    *s
                });
                let payload = json!({
                    "title": field("title"),
                    "content": field("content"),
                    "image": field("image"),
                    "fired": seq,
                });
                // Host namespaces this as `alert.<name>` in the state store.
                set_state(&name, &payload.to_string());
                Ok(Value::Null)
            }
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }
}

quicktys_plugin_sdk::define_plugin!(AlertPlugin);
