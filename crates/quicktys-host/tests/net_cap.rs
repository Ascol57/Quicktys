//! End-to-end system-capability test: a CERTIFIED module performs real HTTP
//! through the host `net` capability, and an UNCERTIFIED module that declares a
//! system capability is refused at load (the trust boundary).

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

fn netdemo_wasm() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../modules/netdemo");
    let wasm = dir.join("target/wasm32-wasip2/release/netdemo.wasm");
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&dir)
            .status()
            .expect("build netdemo");
        assert!(status.success());
    }
    wasm
}

/// One-shot local HTTP server returning `body`; returns its base URL.
fn spawn_server(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Read the request headers (client doesn't half-close its write side).
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while let Ok(n) = stream.read(&mut tmp) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

fn host() -> WasmHost {
    WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap()
}

#[test]
fn certified_module_performs_real_http_fetch() {
    let host = host();
    let manifest = host.load_module(&netdemo_wasm(), true).expect("load netdemo");
    assert_eq!(manifest.id, "netdemo");

    let base = spawn_server(r#"{"hello":"world"}"#);
    let result = host
        .call_module("netdemo", "get", json!({ "url": format!("{base}/data") }))
        .expect("call netdemo.get");

    assert_eq!(result["status"], 200);
    assert_eq!(result["body"], r#"{"hello":"world"}"#);
}

#[test]
fn revoked_consent_denies_the_capability_at_dispatch() {
    let host = host();
    host.load_module(&netdemo_wasm(), true).expect("load netdemo");
    // The module is certified, but the user revoked `net` (M6.7): the consent
    // check denies it, so the fetch must fail even though certification allows it.
    host.set_consent(|id, cap| !(id == "netdemo" && cap == "net"));

    let base = spawn_server(r#"{"hello":"world"}"#);
    let result = host.call_module("netdemo", "get", json!({ "url": format!("{base}/data") }));
    match result {
        Err(_) => {}
        Ok(v) => assert_ne!(v["status"], 200, "fetch should have been denied, got {v}"),
    }
}

#[test]
fn uncertified_module_requesting_a_capability_is_refused() {
    let host = host();
    let err = host
        .load_module(&netdemo_wasm(), false)
        .expect_err("uncertified module with caps must be refused");
    assert!(
        err.to_string().contains("not certified"),
        "unexpected error: {err}"
    );
}
