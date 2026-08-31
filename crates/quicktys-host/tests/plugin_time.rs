//! End-to-end ABI test: build the `time` plugin to a WASM component, load it in
//! the host, invoke its `delay` action, and observe the state it published
//! through the `reactivity` host capability.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

/// Path to the built `time` component, building it on demand if absent.
fn time_wasm() -> PathBuf {
    let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/time");
    let wasm = plugin_dir.join("target/wasm32-wasip2/release/time.wasm");
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&plugin_dir)
            .status()
            .expect("failed to spawn cargo to build the time plugin");
        assert!(status.success(), "building the time plugin failed");
    }
    wasm
}

#[tokio::test]
async fn loads_manifest_and_runs_delay_through_the_abi() {
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let runtime = WasmHost::new(state.clone(), bus).expect("runtime");

    // Load the component and decode the manifest it returns from `init`.
    let manifest = runtime.load_plugin(&time_wasm()).expect("load time plugin");
    assert_eq!(manifest.id, "time");
    assert_eq!(manifest.name, "Time");
    assert!(manifest.actions.iter().any(|a| a.id == "delay"));
    assert!(runtime.has_plugin("time"));

    // Invoke the action through the same ActionInvoker the core runner uses.
    let invoker = runtime.invoker();
    let result = invoker
        .invoke("time", "delay", json!({ "duration": 5 }))
        .await
        .expect("invoke delay");
    assert_eq!(result, json!({ "delayed": 5.0 }));

    // The plugin published reactive state via the host `reactivity` capability,
    // namespaced by plugin id.
    assert_eq!(state.get("time.last_delay"), Some(json!(5.0)));
}

#[tokio::test]
async fn unknown_action_returns_an_error() {
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let runtime = WasmHost::new(state, bus).unwrap();
    runtime.load_plugin(&time_wasm()).unwrap();

    let err = runtime
        .invoker()
        .invoke("time", "nope", json!({}))
        .await
        .unwrap_err();
    assert!(err.contains("unknown action"), "got: {err}");
}
