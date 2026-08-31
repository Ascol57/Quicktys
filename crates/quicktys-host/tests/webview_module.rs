//! The `webview` module owns overlay definitions via the **persistent storage**
//! capability. This proves the CRUD services and that overlays survive a host
//! restart (a fresh `WasmHost` over the same storage dir reloads them) — the
//! guarantee that lets the core webserver query the module for `/overlay/<id>`.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn build() -> PathBuf {
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

fn unique_dir() -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("fm-webview-{n}-{}", CTR.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn host_over(storage: &std::path::Path) -> WasmHost {
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    host.set_storage_dir(storage);
    host.load_module(&build(), false).expect("load webview module");
    host
}

#[test]
fn overlay_crud() {
    let storage = unique_dir();
    let host = host_over(&storage);
    let overlay = json!({ "id": "ov-1", "name": "Main", "aspect": "16:9", "widgets": [] });

    host.call_module("webview", "upsert-overlay", overlay.clone()).unwrap();
    let list = host.call_module("webview", "list-overlays", json!({})).unwrap();
    assert_eq!(list.as_array().map(|a| a.len()), Some(1));

    let got = host.call_module("webview", "get-overlay", json!({ "id": "ov-1" })).unwrap();
    assert_eq!(got.get("name").and_then(|v| v.as_str()), Some("Main"));

    // Update in place.
    let renamed = json!({ "id": "ov-1", "name": "Renamed", "aspect": "9:16", "widgets": [] });
    host.call_module("webview", "upsert-overlay", renamed).unwrap();
    let got = host.call_module("webview", "get-overlay", json!({ "id": "ov-1" })).unwrap();
    assert_eq!(got.get("name").and_then(|v| v.as_str()), Some("Renamed"));
    assert_eq!(host.call_module("webview", "list-overlays", json!({})).unwrap().as_array().map(|a| a.len()), Some(1));

    host.call_module("webview", "delete-overlay", json!({ "id": "ov-1" })).unwrap();
    assert!(host.call_module("webview", "get-overlay", json!({ "id": "ov-1" })).unwrap().is_null());
}

#[test]
fn overlays_persist_across_host_restart() {
    let storage = unique_dir();
    {
        let host = host_over(&storage);
        host.call_module(
            "webview",
            "upsert-overlay",
            json!({ "id": "ov-keep", "name": "Persisted", "widgets": [] }),
        )
        .unwrap();
    }
    // A brand-new host over the same storage dir must reload the overlay.
    let host2 = host_over(&storage);
    let got = host2.call_module("webview", "get-overlay", json!({ "id": "ov-keep" })).unwrap();
    assert_eq!(got.get("name").and_then(|v| v.as_str()), Some("Persisted"));
}
