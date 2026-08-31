//! End-to-end two-tier ABI test: a plugin calls a module's service through the
//! host (`modules.call`), and receives the module's event back
//! (`emit-event` → `on-module-event`), mirroring it into reactive state.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

/// Build (on demand) and locate a wasm32-wasip2 component.
fn build(dir_rel: &str, name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir_rel);
    let wasm = dir.join(format!("target/wasm32-wasip2/release/{name}.wasm"));
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&dir)
            .status()
            .unwrap_or_else(|_| panic!("failed to build {name}"));
        assert!(status.success(), "building {name} failed");
    }
    wasm
}

#[tokio::test]
async fn plugin_calls_module_and_receives_its_event() {
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus).unwrap();

    // Load the certified module first, then the plugin (which subscribes to the
    // module's event during on-load).
    let module = host
        .load_module(&build("../../modules/counter", "counter"), true)
        .expect("load counter module");
    assert_eq!(module.id, "counter");
    assert!(host.has_module("counter"));

    let plugin = host
        // Cargo replaces `-` with `_` in the cdylib artifact name.
        .load_plugin(&build("../../plugins/counter-client", "counter_client"))
        .expect("load counter-client plugin");
    assert_eq!(plugin.id, "counter-client");
    assert_eq!(plugin.required_modules[0].module, "counter");

    let invoker = host.invoker();

    // `bump` → plugin calls counter.add through the host → returns the new total.
    let result = invoker
        .invoke("counter-client", "bump", json!({ "amount": 5 }))
        .await
        .expect("bump");
    assert_eq!(result, json!({ "total": 5.0 }));
    // The module emitted `changed`; the host fanned it out to the subscribed
    // plugin's on-module-event, which mirrored it into reactive state.
    assert_eq!(state.get("counter-client.last_total"), Some(json!(5.0)));

    // State persists in the module across calls (its host kv).
    let result = invoker
        .invoke("counter-client", "bump", json!({ "amount": 3 }))
        .await
        .expect("bump again");
    assert_eq!(result, json!({ "total": 8.0 }));
    assert_eq!(state.get("counter-client.last_total"), Some(json!(8.0)));
}
