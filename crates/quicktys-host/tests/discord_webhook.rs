//! The `discord` plugin has no network access; it routes through the generic
//! `http` module. This proves that path end to end: invoking the `webhook` action
//! makes the `http` module POST the Discord payload (via `net.fetch`) to the URL.
//! A local stand-in HTTP server stands in for Discord (so no real request).

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

/// A one-shot server that captures the request body and replies 204.
fn spawn_capture(tx: mpsc::Sender<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/webhook/abc", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            let n = stream.read(&mut tmp).unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            // Once headers + declared body are in, stop.
            if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..p]).to_string();
                let len = head
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                    .unwrap_or(0);
                if buf.len() >= p + 4 + len {
                    let body = String::from_utf8_lossy(&buf[p + 4..p + 4 + len]).to_string();
                    let _ = tx.send(format!("{head}\n\n{body}"));
                    break;
                }
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    });
    url
}

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
async fn discord_webhook_posts_through_the_http_module() {
    let (tx, rx) = mpsc::channel();
    let url = spawn_capture(tx);
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    host.load_module(&build("../../modules/http", "http_module"), true)
        .expect("load http module");
    host.load_plugin(&build("../../plugins/discord", "discord_plugin"))
        .expect("load discord plugin");

    host.invoker()
        .invoke(
            "discord",
            "webhook",
            json!({ "url": url, "content": "gg!", "username": "Quicktys" }),
        )
        .await
        .expect("webhook action ok");

    let captured = rx.recv_timeout(Duration::from_secs(3)).expect("server got the request");
    assert!(captured.starts_with("POST /webhook/abc"), "method/path: {captured}");
    assert!(captured.contains("Content-Type: application/json"), "content-type: {captured}");
    let body = captured.rsplit("\n\n").next().unwrap();
    let json: Value = serde_json::from_str(body).expect("json body");
    assert_eq!(json.get("content").and_then(Value::as_str), Some("gg!"));
    assert_eq!(json.get("username").and_then(Value::as_str), Some("Quicktys"));
}
