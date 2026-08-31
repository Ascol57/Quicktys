//! `http` module — a generic certified **net driver**. Plugins can't touch the
//! network (safe caps only), so anything that needs an HTTP(S) request (Discord
//! webhooks, generic REST calls, …) goes through this module's `request` service.
//! Being certified it holds the `net` capability; it is deliberately generic (not
//! tied to any single service) and reusable by any plugin that declares it.

use quicktys_module_sdk::{json, Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::net::{fetch, HttpRequest};

struct Http;

impl Module for Http {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("http", "HTTP", "0.1.0")
            .capability(Capability::Net)
            // request { method, url, body?, headers? } → { status, body }.
            .service(ServiceDecl::new("request").args(Schema::object([
                ("method".to_string(), Schema::string()),
                ("url".to_string(), Schema::string()),
                ("body".to_string(), Schema::string()),
                ("headers".to_string(), Schema::object([])),
            ])))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "request" => {
                let url = args.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
                if url.is_empty() {
                    return Err("url is required".to_string());
                }
                let verb = args.get("method").and_then(Value::as_str).unwrap_or("GET").to_string();
                let body = args
                    .get("body")
                    .and_then(Value::as_str)
                    .filter(|b| !b.is_empty())
                    .map(|b| b.as_bytes().to_vec());
                let headers = args
                    .get("headers")
                    .and_then(Value::as_object)
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let resp = fetch(&HttpRequest { method: verb, url, headers, body })
                    .map_err(|e| format!("network error: {e:?}"))?;
                Ok(json!({
                    "status": resp.status,
                    "body": String::from_utf8_lossy(&resp.body),
                }))
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Http);
