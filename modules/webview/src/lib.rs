//! `webview` module — owns the **overlay** definitions (browser-source / remote
//! interface layouts). It is the single source of truth for overlays: the
//! frontend edits them through its services, and the core webserver queries it to
//! serve `/overlay/<id>`. Data lives in the persistent `storage` capability
//! (`overlays` key = a JSON map `id → overlay`); no system caps, so uncertified.

use quicktys_module_sdk::serde_json::Map;
use quicktys_module_sdk::{Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::storage::{kv_get, kv_set};

struct Webview;

/// Load the `id → overlay` map from storage.
fn overlays() -> Map<String, Value> {
    kv_get("overlays")
        .and_then(|s| quicktys_module_sdk::serde_json::from_str::<Map<String, Value>>(&s).ok())
        .unwrap_or_default()
}
fn save(map: &Map<String, Value>) {
    kv_set("overlays", &Value::Object(map.clone()).to_string());
}

impl Module for Webview {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("webview", "Webview", "0.1.0")
            .service(ServiceDecl::new("list-overlays"))
            .service(
                ServiceDecl::new("get-overlay")
                    .args(Schema::object([("id".to_string(), Schema::string())])),
            )
            // Upsert takes the whole overlay object (must carry an `id`).
            .service(ServiceDecl::new("upsert-overlay").args(Schema::object([(
                "id".to_string(),
                Schema::string(),
            )])))
            .service(
                ServiceDecl::new("delete-overlay")
                    .args(Schema::object([("id".to_string(), Schema::string())])),
            )
            // The module ships its own tab + page (the overlay editor): the app
            // renders `ui/index.html` and bridges it to these services.
            // `entry` is relative to the bundle root; the bundler copies the
            // module's `ui/` CONTENTS there (`cp ui/. dest/`), so it's `index.html`.
            .view(
                "pi pi-desktop",
                Some("core.menu.webview".to_string()),
                "index.html",
            )
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "list-overlays" => Ok(Value::Array(overlays().into_values().collect())),
            "get-overlay" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
                Ok(overlays().get(id).cloned().unwrap_or(Value::Null))
            }
            "upsert-overlay" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| "overlay id is required".to_string())?
                    .to_string();
                let mut map = overlays();
                map.insert(id, args);
                save(&map);
                Ok(Value::Null)
            }
            "delete-overlay" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
                let mut map = overlays();
                map.remove(id);
                save(&map);
                Ok(Value::Null)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Webview);
