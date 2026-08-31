//! The exact slice the desktop UI's "Run automation" button drives:
//! the core Runner invokes a WASM plugin action through the host invoker, and
//! the state the plugin publishes lands in the reactive store.

use quicktys_core::{Runner, StateStore, TriggerBus};
use quicktys_expr::ExprEngine;
use quicktys_host::WasmHost;
use quicktys_schema::node::ActionNode;
use quicktys_schema::Node;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

fn time_wasm() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/time");
    let wasm = dir.join("target/wasm32-wasip2/release/time.wasm");
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&dir)
            .status()
            .expect("build time plugin");
        assert!(status.success());
    }
    wasm
}

#[tokio::test]
async fn runner_invokes_a_wasm_plugin_action() {
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus.clone()).unwrap();
    host.load_plugin(&time_wasm()).unwrap();

    // A runner wired to the host invoker — exactly what `quicktys-app` builds.
    let runner = Runner::with_state(Arc::new(ExprEngine::new()), host.invoker(), state.clone());

    // An automation that invokes the plugin action (as the UI's demo does).
    let root = Node::Action(ActionNode {
        plugin: "time".into(),
        action: "delay".into(),
        config: json!({ "duration": 7 }),
        bind: None,
    });
    runner.run(&root, Default::default()).await.unwrap();

    // The plugin published reactive state through the host capability.
    assert_eq!(state.get("time.last_delay"), Some(json!(7.0)));
}
