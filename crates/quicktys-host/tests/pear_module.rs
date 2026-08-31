//! End-to-end test of the real `pear` integration against a stand-in Pear
//! Desktop HTTP API: the `pear` plugin's actions reach the certified `pear`
//! module, which authenticates and calls the API over the `net` capability.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    auth: Option<String>,
    body: String,
}

/// A stand-in Pear Desktop API on a free port (so we never touch the user's
/// real, possibly-running player). Returns the log and the base URL.
fn spawn_mock() -> (Arc<Mutex<Vec<Recorded>>>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { break };
            // Read request headers + any Content-Length body (the client no
            // longer half-closes its write side to signal EOF).
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while let Ok(n) = stream.read(&mut tmp) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..p]).to_lowercase();
                    let cl = head.lines().find_map(|l| {
                        l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())
                    });
                    if let Some(cl) = cl {
                        while buf.len() < p + 4 + cl {
                            match stream.read(&mut tmp) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                            }
                        }
                    }
                    break;
                }
            }
            let text = String::from_utf8_lossy(&buf);
            let first = text.lines().next().unwrap_or("");
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let auth = text
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                .map(|l| l.trim().to_string());
            let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
            log2.lock().unwrap().push(Recorded {
                method: method.clone(),
                path: path.clone(),
                auth,
                body,
            });

            let response = if path.starts_with("/auth/") {
                http_json(200, r#"{"accessToken":"tok-1"}"#)
            } else if method == "GET" && path.ends_with("/song") {
                http_json(200, r#"{"title":"Test Song"}"#)
            } else {
                "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_string()
            };
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (log, base)
}

fn http_json(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
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
async fn pear_plugin_controls_the_api_through_the_module() {
    let (log, base) = spawn_mock();

    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    let module = host
        .load_module(&build("../../modules/pear", "pear"), true)
        .expect("load pear module (certified)");
    assert_eq!(module.id, "pear");
    let plugin = host
        .load_plugin(&build("../../plugins/pear", "pear_plugin"))
        .expect("load pear plugin");
    assert_eq!(plugin.required_modules[0].module, "pear");

    // Point the module at the stand-in API (not the user's real player).
    host.call_module("pear", "set-base-url", json!({ "url": base }))
        .expect("set-base-url");

    let invoker = host.invoker();

    // Action "play" → module → auth (first) then POST /play.
    invoker.invoke("pear", "play", json!({})).await.expect("play");
    // Action "set-volume" → POST /volume with the JSON body.
    invoker
        .invoke("pear", "set-volume", json!({ "volume": 70 }))
        .await
        .expect("set-volume");
    // Query the current song directly through the module.
    let song = host
        .call_module("pear", "current-song", json!({}))
        .expect("current-song");
    assert_eq!(song, json!({ "title": "Test Song" }));

    let calls = log.lock().unwrap().clone();
    // The module authenticated once.
    assert!(
        calls.iter().any(|c| c.method == "POST" && c.path == "/auth/quicktys"),
        "expected an auth call, got: {calls:?}"
    );
    // /play was called with the bearer token.
    let play = calls
        .iter()
        .find(|c| c.path == "/api/v1/play")
        .expect("play call");
    assert_eq!(play.method, "POST");
    assert_eq!(play.auth.as_deref(), Some("Authorization: Bearer tok-1"));
    // /volume carried the JSON body.
    let vol = calls
        .iter()
        .find(|c| c.path == "/api/v1/volume")
        .expect("volume call");
    assert!(vol.body.contains("\"volume\":70"), "body: {}", vol.body);
    // /song was queried.
    assert!(calls.iter().any(|c| c.method == "GET" && c.path == "/api/v1/song"));
}
