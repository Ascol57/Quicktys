//! End-to-end test of the `twitch` integration against a stand-in Helix API:
//! the `twitch` plugin's actions reach the certified `twitch` module, which
//! authenticates (bearer + client-id) and calls Helix over the `net` capability.
//! We never touch the real api.twitch.tv (base URL is redirected to the mock).

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
    client_id: Option<String>,
    body: String,
}

/// A stand-in Helix API on a free port. Every request is recorded; POSTs get a
/// canned Helix-shaped response.
fn spawn_mock() -> (Arc<Mutex<Vec<Recorded>>>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { break };
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
                        l.strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
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
            let header = |name: &str| {
                text.lines()
                    .find(|l| l.to_ascii_lowercase().starts_with(name))
                    .map(|l| l.split_once(':').map(|(_, v)| v.trim()).unwrap_or("").to_string())
            };
            let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
            log2.lock().unwrap().push(Recorded {
                method: method.clone(),
                path: path.clone(),
                auth: header("authorization:"),
                client_id: header("client-id:"),
                body,
            });

            // Helix returns 200 with a `data` array for these endpoints.
            let response = if path.starts_with("/streams") {
                http_json(200, r#"{"data":[{"type":"live","viewer_count":42,"title":"Hi"}]}"#)
            } else {
                http_json(200, r#"{"data":[{"message_id":"m1","is_sent":true}]}"#)
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
async fn twitch_plugin_drives_helix_through_the_module() {
    let (log, base) = spawn_mock();

    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    let module = host
        .load_module(&build("../../modules/twitch", "twitch"), true)
        .expect("load twitch module (certified)");
    assert_eq!(module.id, "twitch");
    let plugin = host
        .load_plugin(&build("../../plugins/twitch", "twitch_plugin"))
        .expect("load twitch plugin");
    assert_eq!(plugin.required_modules[0].module, "twitch");

    // Point the module at the mock and inject auth.
    host.call_module("twitch", "set-base-url", json!({ "url": base }))
        .expect("set-base-url");
    host.call_module(
        "twitch",
        "set-auth",
        json!({
            "token": "usertok",
            "client_id": "clientabc",
            "broadcaster_id": "12345",
        }),
    )
    .expect("set-auth");

    let invoker = host.invoker();

    // Action "chat" → module send-chat → POST /chat/messages.
    invoker
        .invoke("twitch", "chat", json!({ "message": "hello chat" }))
        .await
        .expect("chat");
    // Action "announce" → POST /chat/announcements.
    invoker
        .invoke("twitch", "announce", json!({ "message": "big news", "color": "purple" }))
        .await
        .expect("announce");
    // Direct module query: get-stream returns the first stream object.
    let stream = host.call_module("twitch", "get-stream", json!({})).expect("get-stream");
    assert_eq!(stream.get("viewer_count").and_then(|v| v.as_i64()), Some(42));

    let calls = log.lock().unwrap().clone();

    // send-chat: POST with bearer + client-id, and the message/ids in the body.
    let chat = calls
        .iter()
        .find(|c| c.path == "/chat/messages")
        .expect("chat call");
    assert_eq!(chat.method, "POST");
    assert_eq!(chat.auth.as_deref(), Some("Bearer usertok"));
    assert_eq!(chat.client_id.as_deref(), Some("clientabc"));
    assert!(chat.body.contains("\"message\":\"hello chat\""), "body: {}", chat.body);
    assert!(chat.body.contains("\"broadcaster_id\":\"12345\""), "body: {}", chat.body);
    // sender_id defaults to the broadcaster.
    assert!(chat.body.contains("\"sender_id\":\"12345\""), "body: {}", chat.body);

    // announce: broadcaster_id + moderator_id go in the query string.
    let ann = calls
        .iter()
        .find(|c| c.path.starts_with("/chat/announcements"))
        .expect("announce call");
    assert!(ann.path.contains("broadcaster_id=12345"), "path: {}", ann.path);
    assert!(ann.path.contains("moderator_id=12345"), "path: {}", ann.path);
    assert!(ann.body.contains("\"color\":\"purple\""), "body: {}", ann.body);

    // get-stream: GET /streams?user_id=12345
    let strm = calls
        .iter()
        .find(|c| c.path.starts_with("/streams"))
        .expect("stream call");
    assert_eq!(strm.method, "GET");
    assert!(strm.path.contains("user_id=12345"), "path: {}", strm.path);
}

#[tokio::test]
async fn uncertified_twitch_module_is_refused() {
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    let err = host
        .load_module(&build("../../modules/twitch", "twitch"), false)
        .expect_err("uncertified module declaring `net` must be refused");
    assert!(
        err.to_string().contains("not certified"),
        "unexpected error: {err}"
    );
}
