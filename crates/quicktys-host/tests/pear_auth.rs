//! The `pear` module's **authentication** against a stand-in Pear Desktop API.
//!
//! Mirrors pear-desktop's `src/plugins/api-server`: `POST /auth/{id}` mints a
//! JWT (or answers **403** when the user presses Deny in the player's native
//! dialog), and `/api/*` rejects a stale token with **401**.
//!
//! What matters here is the *dialog-storm* guarantee: the host polls this module
//! every 3 seconds, and under the player's default `AUTH_AT_FIRST` strategy each
//! `/auth/{id}` for an unknown client opens a modal dialog on the user's screen.
//! A denial must therefore be **sticky** — retried only when the user explicitly
//! asks — while a merely stale token must still recover on its own.

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
}

/// Stand-in player state the test drives.
#[derive(Debug)]
struct Mock {
    /// Status `/auth/{id}` answers with (200 = allow, 403 = the user denied).
    auth_status: u16,
    /// The only token `/api/*` currently accepts; `None` = everything is stale
    /// (what a player restart with a fresh `secret` looks like).
    issued: Option<String>,
    minted: u32,
    calls: Vec<Recorded>,
}

impl Mock {
    fn auth_calls(&self) -> usize {
        self.calls.iter().filter(|c| c.path.starts_with("/auth/")).count()
    }
}

fn spawn_mock() -> (Arc<Mutex<Mock>>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new(Mutex::new(Mock {
        auth_status: 200,
        issued: None,
        minted: 0,
        calls: Vec::new(),
    }));
    let shared = state.clone();

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
            let auth = text
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                .map(|l| l.trim().to_string());

            let mut m = shared.lock().unwrap();
            m.calls.push(Recorded {
                method: method.clone(),
                path: path.clone(),
                auth: auth.clone(),
            });

            let response = if path.starts_with("/auth/") {
                if m.auth_status == 200 {
                    m.minted += 1;
                    let tok = format!("tok-{}", m.minted);
                    m.issued = Some(tok.clone());
                    http_body(200, &format!(r#"{{"accessToken":"{tok}"}}"#))
                } else {
                    // The player answers 403 with an empty body on Deny.
                    http_body(m.auth_status, "")
                }
            } else {
                // `/api/*` accepts only the token currently issued.
                let expected = m.issued.as_ref().map(|t| format!("Authorization: Bearer {t}"));
                if expected.is_some() && auth == expected {
                    http_body(200, r#"{"ok":true}"#)
                } else {
                    http_body(401, "")
                }
            };
            drop(m);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (state, base)
}

fn http_body(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        403 => "Forbidden",
        _ => "Status",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

fn host_with_pear(base: &str) -> WasmHost {
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    host.load_module(&build("../../modules/pear", "pear"), true)
        .expect("load pear module (certified)");
    // Point the module at the stand-in API, never the user's real player.
    host.call_module("pear", "set-base-url", json!({ "url": base }))
        .expect("set-base-url");
    host
}

/// A Deny in the player must stop us asking: one `/auth` call, no matter how
/// many API calls follow. Without this the 3s poller reopens the player's modal
/// dialog every 3 seconds, forever.
#[test]
fn a_denied_authorization_is_sticky() {
    let (mock, base) = spawn_mock();
    mock.lock().unwrap().auth_status = 403;
    let host = host_with_pear(&base);

    for _ in 0..3 {
        assert!(
            host.call_module("pear", "play", json!({})).is_err(),
            "play must fail while the client is denied"
        );
    }

    let m = mock.lock().unwrap();
    assert_eq!(m.auth_calls(), 1, "expected exactly one /auth call, got: {:?}", m.calls);
    let auth = m.calls.iter().find(|c| c.path.starts_with("/auth/")).expect("the auth call");
    assert_eq!(auth.method, "POST", "the player registers /auth/{{id}} as a POST");

    drop(m);
    let status = host.call_module("pear", "auth-status", json!({})).expect("auth-status");
    assert_eq!(status["state"], json!("denied"));
    assert_eq!(status["authorized"], json!(false));
}

/// ...but the user can change their mind: an explicit `authorize` bypasses the
/// sticky denial, and normal calls work again afterwards.
#[test]
fn explicit_authorize_retries_after_a_denial() {
    let (mock, base) = spawn_mock();
    mock.lock().unwrap().auth_status = 403;
    let host = host_with_pear(&base);

    assert!(host.call_module("pear", "play", json!({})).is_err());
    assert!(
        host.call_module("pear", "authorize", json!({})).is_err(),
        "still denied by the player"
    );

    // The user presses Allow this time.
    mock.lock().unwrap().auth_status = 200;
    host.call_module("pear", "authorize", json!({})).expect("authorize");

    let status = host.call_module("pear", "auth-status", json!({})).expect("auth-status");
    assert_eq!(status["state"], json!("authorized"));
    assert_eq!(status["authorized"], json!(true));

    host.call_module("pear", "play", json!({})).expect("play after authorize");
}

/// A stale token (the player restarted with a fresh `secret`) is not a denial:
/// the 401 must trigger one silent re-auth and a replay of the original call.
#[test]
fn a_stale_token_is_refreshed_and_the_call_replayed() {
    let (mock, base) = spawn_mock();
    let host = host_with_pear(&base);

    host.call_module("pear", "play", json!({})).expect("first play");
    assert_eq!(mock.lock().unwrap().auth_calls(), 1);

    // Every existing token is now rejected — what a player restart looks like.
    mock.lock().unwrap().issued = None;

    host.call_module("pear", "play", json!({}))
        .expect("play must recover through a silent re-auth");

    let m = mock.lock().unwrap();
    assert_eq!(m.auth_calls(), 2, "expected a second /auth, got: {:?}", m.calls);
    // The replay carried the *new* token.
    let last = m.calls.last().expect("a last call");
    assert_eq!(last.path, "/api/v1/play");
    assert_eq!(last.auth.as_deref(), Some("Authorization: Bearer tok-2"));
}

/// The client id is what the player shows in its dialog and remembers in
/// `authorizedClients`; changing it re-authenticates under the new identity.
#[test]
fn the_client_id_is_configurable() {
    let (mock, base) = spawn_mock();
    let host = host_with_pear(&base);

    host.call_module("pear", "play", json!({})).expect("play");
    assert!(mock.lock().unwrap().calls.iter().any(|c| c.path == "/auth/quicktys"));

    host.call_module("pear", "set-client-id", json!({ "id": "deck-1" }))
        .expect("set-client-id");
    let status = host.call_module("pear", "auth-status", json!({})).expect("auth-status");
    assert_eq!(status["clientId"], json!("deck-1"));
    assert_eq!(status["hasToken"], json!(false), "the old token is dropped");

    host.call_module("pear", "play", json!({})).expect("play under the new id");
    assert!(mock.lock().unwrap().calls.iter().any(|c| c.path == "/auth/deck-1"));
}
