//! The `notify` plugin: its `toast` action reaches the host `reactivity.notify`
//! capability, which the app forwards to the UI. Here we capture the sink and
//! assert the message/level arrive; `log` just exercises the logging cap.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

fn build(dir_rel: &str, name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir_rel);
    let wasm = dir.join(format!("target/wasm32-wasip2/release/{name}.wasm"));
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&dir)
            .status()
            .unwrap_or_else(|_| panic!("build {name}"));
        assert!(status.success());
    }
    wasm
}

#[tokio::test]
async fn toast_action_reaches_the_notify_sink() {
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    host.load_plugin(&build("../../plugins/notify", "notify"))
        .expect("load notify plugin");

    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    host.set_notify(move |message, level| seen2.lock().unwrap().push((message, level)));

    let invoker = host.invoker();
    invoker
        .invoke("notify", "toast", json!({ "message": "Hello!", "level": "success" }))
        .await
        .expect("toast");
    // `log` uses the logging cap only (no sink) — just shouldn't error.
    invoker
        .invoke("notify", "log", json!({ "message": "logged", "level": "info" }))
        .await
        .expect("log");

    assert_eq!(
        seen.lock().unwrap().clone(),
        vec![("Hello!".to_string(), "success".to_string())]
    );
}
