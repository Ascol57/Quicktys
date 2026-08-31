//! The `webview` module contributes a full app **view/tab** (its overlay-editor
//! page). This proves the `view` declaration round-trips from the module's WASM
//! manifest through the host — the app reads it to add the sidebar tab and render
//! the page (`moduleui://webview/index.html`).

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

fn webview_wasm() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../modules/webview");
    let wasm = dir.join("target/wasm32-wasip2/release/webview_module.wasm");
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&dir)
            .status()
            .expect("build webview module");
        assert!(status.success());
    }
    wasm
}

#[test]
fn webview_module_declares_a_view() {
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    // Safe caps only (storage/logging/events) → loads uncertified.
    let manifest = host.load_module(&webview_wasm(), false).expect("load webview module");

    let view = manifest.view.expect("webview declares a contributed view");
    assert_eq!(view.icon, "pi pi-desktop");
    assert_eq!(view.entry, "index.html");
    assert_eq!(view.label.as_deref(), Some("core.menu.webview"));
}
