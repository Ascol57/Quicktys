//! `overlays` plugin — the user-facing face of the overlay feature, sitting on
//! top of the `webview` module.
//!
//! **Why it exists.** Overlays are stored by the `webview` module, which the
//! core itself consumes (the webserver serves `/overlay/<id>` from it, and the
//! app renders the editor view the module ships). But a module is never
//! browsable in the marketplace — it is only ever pulled in as a *dependency of
//! a plugin* — and no plugin required `webview`, so on a registry install it
//! simply never arrived and the whole overlay feature was missing. This plugin
//! declares `requires: webview`, which puts the module back on the normal
//! resolution path: install "Overlays" and the module comes with it.
//!
//! Because of that, uninstalling this plugin also drops the module (nothing else
//! depends on it), and with it the overlay editor and `/overlay/<id>`. That is
//! deliberate — this plugin *is* the overlay feature as far as the user is
//! concerned — but it means the core degrades gracefully when it is absent:
//! `webserver.rs` and the frontend both tolerate the module not being there.
//!
//! On top of carrying the dependency it exposes the overlay list as reactive
//! state (`overlays.count`, `overlays.ids`) and thin actions over the module's
//! services, so automations can create or remove an overlay.

use quicktys_plugin_sdk::{
    json, ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, Value,
};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};
use quicktys::api::modules::call as module_call;
use quicktys::api::reactivity::set_state;

/// The module this plugin exists to pull in and drive.
const MODULE: &str = "webview";

fn call(method: &str, args: Value) -> Result<Value, String> {
    let raw = module_call(MODULE, method, &args.to_string())
        .map_err(|e| format!("{MODULE}.{method} failed: {e:?}"))?;
    Ok(quicktys_plugin_sdk::serde_json::from_str(&raw).unwrap_or(Value::Null))
}

/// Republish `overlays.count` / `overlays.ids` from the module's current list.
/// Returns the list so an action can hand it back to the sequence.
fn sync() -> Result<Value, String> {
    let list = call("list-overlays", json!({}))?;
    let ids: Vec<Value> = list
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|o| o.get("id").cloned())
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();
    set_state("count", &json!(ids.len()).to_string());
    set_state("ids", &Value::Array(ids).to_string());
    Ok(list)
}

/// Read a trimmed, non-empty string field from an action config.
fn field(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

struct OverlaysPlugin;

impl Plugin for OverlaysPlugin {
    fn manifest() -> Manifest {
        Manifest::new("overlays", "Overlays", "0.1.0")
            .icon("mdi mdi-monitor-dashboard")
            .color("#6f9ad3")
            // The whole point: this is what pulls `module/webview` in.
            .requires(MODULE, "^0")
            .action(
                ActionDecl::new("create").config(Schema::object([
                    (
                        "id".to_string(),
                        Schema::template_string()
                            .named("plugin.overlays.actions.create.config.id"),
                    ),
                    (
                        "title".to_string(),
                        Schema::template_string()
                            .named("plugin.overlays.actions.create.config.title"),
                    ),
                ])),
            )
            .action(
                ActionDecl::new("delete").config(Schema::object([(
                    "id".to_string(),
                    Schema::template_string().named("plugin.overlays.actions.delete.config.id"),
                )])),
            )
            .action(ActionDecl::new("refresh"))
            .state_field(StateDecl::new("count").schema(Schema::number()))
            .state_field(StateDecl::new("ids").schema(Schema::array(Schema::string())))
    }

    /// Publish the initial state. A failure here must NOT fail the load: the
    /// module is loaded before plugins, but if it is missing (someone removed it
    /// by hand) the plugin should still come up with an empty list rather than
    /// take the whole boot down.
    fn on_load() -> Result<(), String> {
        if let Err(e) = sync() {
            log(Level::Warn, &format!("overlays: initial sync failed ({e})"));
            set_state("count", "0");
            set_state("ids", "[]");
        }
        Ok(())
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            // Create an overlay, or update the title of one that already exists.
            // `upsert-overlay` replaces the whole object, so read the current one
            // first and merge into it — creating over an existing id must not
            // silently wipe its layout.
            "create" => {
                let id = field(&config, "id")
                    .ok_or_else(|| ActionError::failed("overlay id is required".to_string()))?;
                let existing = call("get-overlay", json!({ "id": id })).map_err(ActionError::failed)?;
                let mut overlay = match existing {
                    Value::Object(map) => Value::Object(map),
                    _ => json!({}),
                };
                overlay["id"] = json!(id);
                if let Some(title) = field(&config, "title") {
                    overlay["title"] = json!(title);
                } else if overlay.get("title").is_none() {
                    overlay["title"] = json!(id);
                }
                call("upsert-overlay", overlay.clone()).map_err(ActionError::failed)?;
                sync().map_err(ActionError::failed)?;
                Ok(overlay)
            }
            "delete" => {
                let id = field(&config, "id")
                    .ok_or_else(|| ActionError::failed("overlay id is required".to_string()))?;
                call("delete-overlay", json!({ "id": id })).map_err(ActionError::failed)?;
                sync().map_err(ActionError::failed)?;
                Ok(json!({ "deleted": id }))
            }
            // Re-read the module and republish the state (after an edit made in
            // the overlay editor, say).
            "refresh" => sync().map_err(ActionError::failed),
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }
}

quicktys_plugin_sdk::define_plugin!(OverlaysPlugin);
