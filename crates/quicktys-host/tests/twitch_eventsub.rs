//! EventSub over WebSocket, end to end against stand-in servers.
//!
//! The flow Twitch documents: connect, receive `session_welcome`, create the
//! subscriptions over Helix with `transport: {method:"websocket", session_id}`
//! **within 10 seconds**, then receive `notification` frames on the same socket.
//! This checks both halves — that we subscribe with the right types, versions
//! and conditions, and that a notification becomes a Quicktys trigger.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn text_frame(payload: &str) -> Vec<u8> {
    let bytes = payload.as_bytes();
    let mut frame = vec![0x81];
    if bytes.len() < 126 {
        frame.push(bytes.len() as u8);
    } else {
        frame.push(126);
        frame.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(bytes);
    frame
}

fn read_http(stream: &mut std::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while let Ok(n) = stream.read(&mut tmp) {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..p]).to_lowercase();
            let len = head.lines().find_map(|l| {
                l.strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            });
            if let Some(len) = len {
                while buf.len() < p + 4 + len {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
            }
            break;
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

/// Stand-in Helix: answers `/users` and records every subscription POST.
fn spawn_helix(seen: Arc<Mutex<Vec<String>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut s) = conn else { break };
            let req = read_http(&mut s);
            let first = req.lines().next().unwrap_or("").to_string();
            let body = if first.contains("/users") {
                // Both the channel lookup and "who am I" land here; one id is
                // enough for the test.
                r#"{"data":[{"id":"4242","login":"ascol57"}]}"#.to_string()
            } else {
                seen.lock().unwrap().push(req.clone());
                r#"{"data":[]}"#.to_string()
            };
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    format!("http://{addr}")
}

/// Stand-in EventSub socket: welcome, then the frames it was handed.
fn spawn_eventsub(frames: Vec<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let Ok((mut s, _)) = listener.accept() else { return };
        let _ = read_http(&mut s);
        let _ = s.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        let welcome = json!({
            "metadata": { "message_type": "session_welcome" },
            "payload": { "session": { "id": "session-abc", "status": "connected" } }
        })
        .to_string();
        let _ = s.write_all(&text_frame(&welcome));
        for f in frames {
            std::thread::sleep(Duration::from_millis(60));
            let _ = s.write_all(&text_frame(&f));
        }
        std::thread::sleep(Duration::from_millis(800));
    });
    format!("ws://{addr}/ws")
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

fn eventually<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out waiting for EventSub");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn welcome_creates_subscriptions_and_notifications_become_triggers() {
    let posted = Arc::new(Mutex::new(Vec::new()));
    let helix = spawn_helix(posted.clone());
    let socket = spawn_eventsub(vec![
        // Keepalives must not be mistaken for events.
        json!({ "metadata": { "message_type": "session_keepalive" }, "payload": {} }).to_string(),
        json!({
            "metadata": { "message_type": "notification", "subscription_type": "channel.follow" },
            "payload": {
                "subscription": { "type": "channel.follow" },
                "event": { "user_name": "Nina", "user_login": "nina", "broadcaster_user_name": "Ascol57" }
            }
        })
        .to_string(),
        json!({
            "metadata": { "message_type": "notification", "subscription_type": "channel.raid" },
            "payload": {
                "subscription": { "type": "channel.raid" },
                "event": { "from_broadcaster_user_name": "lea_streams", "viewers": 132 }
            }
        })
        .to_string(),
    ]);

    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state, bus.clone()).unwrap();
    let mut fired = bus.subscribe();

    host.load_module(&build("../../modules/twitch", "twitch"), true)
        .expect("load twitch module");
    host.load_plugin(&build("../../plugins/twitch", "twitch_plugin"))
        .expect("load twitch plugin");

    // Everything points at the stand-ins; the real Twitch is never touched.
    host.call_module("twitch", "set-base-url", json!({ "url": helix })).unwrap();
    host.call_module("twitch", "set-eventsub-url", json!({ "url": socket })).unwrap();
    // A token so Helix calls are attempted at all.
    host.call_module("twitch", "set-auth", json!({ "token": "t", "client_id": "c" }))
        .unwrap();
    host.call_module("twitch", "connect-events", json!({ "channel": "Ascol57" }))
        .expect("connect-events");

    // The welcome must produce the three subscriptions, with the documented
    // versions and conditions.
    let subs = eventually(|| {
        let seen = posted.lock().unwrap();
        (seen.len() >= 3).then(|| seen.clone())
    });
    let bodies: Vec<Value> = subs
        .iter()
        .filter_map(|req| req.split("\r\n\r\n").nth(1))
        .filter_map(|body| serde_json::from_str(body).ok())
        .collect();
    let of = |kind: &str| {
        bodies
            .iter()
            .find(|b| b["type"] == json!(kind))
            .unwrap_or_else(|| panic!("no subscription for {kind}: {bodies:?}"))
    };
    // channel.follow is v2 (v1 is deprecated) and needs BOTH ids.
    assert_eq!(of("channel.follow")["version"], json!("2"));
    assert_eq!(of("channel.follow")["condition"]["broadcaster_user_id"], json!("4242"));
    assert_eq!(of("channel.follow")["condition"]["moderator_user_id"], json!("4242"));
    assert_eq!(of("channel.subscribe")["version"], json!("1"));
    // Raids INTO us, so `to_` — `from_` would watch the wrong direction.
    assert_eq!(of("channel.raid")["condition"]["to_broadcaster_user_id"], json!("4242"));
    for kind in ["channel.follow", "channel.subscribe", "channel.raid"] {
        assert_eq!(of(kind)["transport"], json!({ "method": "websocket", "session_id": "session-abc" }));
    }

    // …and the notifications became triggers, keepalive excluded.
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.len() < 2 {
        match fired.try_recv() {
            Ok(event) => seen.push(event),
            Err(_) => {
                assert!(Instant::now() < deadline, "only got {} trigger(s)", seen.len());
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
    assert_eq!(seen[0].id, "twitch.follow");
    assert_eq!(seen[0].context["user_name"], json!("Nina"));
    assert_eq!(seen[1].id, "twitch.raid");
    assert_eq!(seen[1].context["viewers"], json!(132));
}
