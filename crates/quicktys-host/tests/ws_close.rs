//! Re-joining a chat must not double every message.
//!
//! Several widgets can ask to watch the same channel — a dashboard box *and* an
//! overlay — so `connect-chat` runs more than once and closes the socket it
//! opened before. That only works if `net.close` genuinely closes: forgetting
//! the write half leaves the reader thread parked on a live socket, still
//! delivering, and every message lands in the buffer twice.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

fn handshake(s: &mut TcpStream) {
    let mut req = Vec::new();
    let mut tmp = [0u8; 1024];
    while let Ok(n) = s.read(&mut tmp) {
        if n == 0 {
            break;
        }
        req.extend_from_slice(&tmp[..n]);
        if req.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let _ = s.write_all(
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
    );
}

/// Accepts two connections and hands both back, so the test can push a message
/// down each and see how many reach the buffer.
fn spawn_two() -> (String, Arc<Mutex<Vec<TcpStream>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let socks = Arc::new(Mutex::new(Vec::new()));
    let out = socks.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming().take(2) {
            let Ok(mut s) = conn else { break };
            handshake(&mut s);
            out.lock().unwrap().push(s);
        }
        std::thread::sleep(Duration::from_millis(1500));
    });
    (format!("ws://{addr}/"), socks)
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

fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(25));
    }
}

const MESSAGE: &str = concat!(
    "@badges=;color=;display-name=nina;emotes=;id=one ",
    ":nina!nina@nina.tmi.twitch.tv PRIVMSG #ascol57 :bonjour\r\n"
);

#[test]
fn re_joining_closes_the_previous_socket_so_messages_are_not_doubled() {
    let (url, socks) = spawn_two();

    let state = Arc::new(StateStore::new());
    let host = WasmHost::new(state.clone(), Arc::new(TriggerBus::new())).unwrap();
    host.load_module(&build("../../modules/twitch", "twitch"), true)
        .expect("load twitch module");
    host.load_plugin(&build("../../plugins/twitch", "twitch_plugin"))
        .expect("load twitch plugin");
    host.call_module("twitch", "set-chat-url", json!({ "url": url })).unwrap();

    // Two widgets asking to watch the same channel.
    host.call_module("twitch", "connect-chat", json!({ "channel": "ascol57" })).unwrap();
    host.call_module("twitch", "connect-chat", json!({ "channel": "ascol57" })).unwrap();
    let two = wait_for(|| {
        let s = socks.lock().unwrap();
        (s.len() == 2).then(|| ())
    });
    let _ = two;

    // Push the SAME message down both sockets. Only the second connection is
    // still live, so exactly one copy may reach the buffer.
    {
        let mut s = socks.lock().unwrap();
        for sock in s.iter_mut() {
            let _ = sock.write_all(&text_frame(MESSAGE));
        }
    }

    let list = wait_for(|| match state.get("twitch.chat") {
        Some(Value::Array(l)) if !l.is_empty() => Some(l),
        _ => None,
    });
    // Give any surviving second reader a chance to deliver its copy before we
    // conclude — otherwise the assertion could pass by being too quick.
    std::thread::sleep(Duration::from_millis(400));
    let list = match state.get("twitch.chat") {
        Some(Value::Array(l)) => l,
        _ => list,
    };
    assert_eq!(
        list.len(),
        1,
        "the closed socket is still delivering — messages doubled: {list:?}"
    );
    assert_eq!(list[0]["text"], json!("bonjour"));
}
