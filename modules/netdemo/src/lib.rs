//! `netdemo` module — a certified driver that exercises the `net` system
//! capability by performing a real HTTP GET through the host.
//!
//! Being certified and declaring `net`, the host grants it network access (a
//! plugin never could). Used to validate real system I/O across the certified
//! boundary.

use quicktys_module_sdk::{Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::net::{fetch, HttpRequest};

struct NetDemo;

impl Module for NetDemo {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("netdemo", "Net Demo", "0.1.0")
            .capability(Capability::Net)
            .service(
                ServiceDecl::new("get")
                    .args(Schema::object([("url".to_string(), Schema::string())])),
            )
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "get" => {
                let url = args
                    .get("url")
                    .and_then(Value::as_str)
                    .ok_or("missing url")?;
                let request = HttpRequest {
                    method: "GET".to_string(),
                    url: url.to_string(),
                    headers: Vec::new(),
                    body: None,
                };
                let response = fetch(&request).map_err(|e| format!("fetch failed: {e:?}"))?;
                let body = String::from_utf8_lossy(&response.body).to_string();
                Ok(quicktys_module_sdk::json!({
                    "status": response.status,
                    "body": body,
                }))
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(NetDemo);
