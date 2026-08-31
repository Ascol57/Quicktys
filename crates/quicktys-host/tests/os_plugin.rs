//! The `os` plugin → `os` module → `process` capability, end to end: running a
//! shell command actually executes it host-side (writes a sentinel file).

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
async fn os_shell_runs_the_command() {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let marker = std::env::temp_dir().join(format!("fm-os-{nanos}-{}", CTR.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_file(&marker);

    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    host.load_module(&build("../../modules/os", "os_module"), true).expect("load os module");
    host.load_plugin(&build("../../plugins/os", "os_plugin")).expect("load os plugin");

    host.invoker()
        .invoke("os", "shell", json!({ "command": format!("echo hi > '{}'", marker.display()) }))
        .await
        .expect("shell action ok");

    // run-shell spawns and returns; give the command a moment to finish.
    let mut ok = false;
    for _ in 0..40 {
        if marker.exists() {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let _ = std::fs::remove_file(&marker);
    assert!(ok, "the shell command should have created the marker file");
}
