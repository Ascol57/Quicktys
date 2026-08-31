//! Live Twitch chat, end to end against a stand-in IRC server.
//!
//! The real feed is `wss://irc-ws.chat.twitch.tv` (Twitch decommissioned the
//! unencrypted endpoint), so this exercises the whole chain the widget depends
//! on: the module opens a WebSocket, speaks IRC, parses the IRCv3 tags, and
//! emits `chat`; the plugin buffers those into the reactive `twitch.chat` list
//! and fires the `twitch.chat-message` trigger.
//!
//! The socket here is plain `ws://` pointed at a local server — the TLS path is
//! the same code, and we never touch Twitch from a test.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One masked-free server→client text frame (servers must not mask).
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

/// Decode one client→server frame. Client frames are always masked (RFC 6455).
fn read_client_frame(stream: &mut std::net::TcpStream) -> Option<String> {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).ok()?;
    let masked = head[1] & 0x80 != 0;
    let mut len = (head[1] & 0x7f) as usize;
    if len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext).ok()?;
        len = u16::from_be_bytes(ext) as usize;
    }
    let mut mask = [0u8; 4];
    if masked {
        stream.read_exact(&mut mask).ok()?;
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).ok()?;
    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
    }
    Some(String::from_utf8_lossy(&payload).to_string())
}

/// A stand-in Twitch IRC server: completes the WebSocket handshake, records what
/// the client says, then pushes the lines it was given.
fn spawn_irc(lines: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let said = Arc::new(Mutex::new(Vec::new()));
    let heard = said.clone();
    std::thread::spawn(move || {
        let Ok((mut s, _)) = listener.accept() else { return };
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
        // What the client sends the moment it connects — the IRC login. Twitch
        // stays silent until it arrives, so a lost CAP/NICK/JOIN means a chat
        // that never produces anything.
        let mut reader = s.try_clone().expect("clone");
        std::thread::spawn(move || {
            while let Some(line) = read_client_frame(&mut reader) {
                heard.lock().unwrap().push(line);
            }
        });
        for line in lines {
            let _ = s.write_all(&text_frame(&line));
            std::thread::sleep(Duration::from_millis(20));
        }
        // Hold the socket open so the reader thread doesn't see a close.
        std::thread::sleep(Duration::from_millis(800));
    });
    (format!("ws://{addr}/"), said)
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

/// Wait for `f` to return a value, polling — the feed arrives on the host's
/// WebSocket reader thread, so it is inherently asynchronous.
fn eventually<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the chat feed");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn irc_messages_reach_the_reactive_buffer_and_the_trigger() {
    // A realistic tagged PRIVMSG, plus a PING (which must not become a message)
    // and a JOIN acknowledgement (ditto).
    let (url, said) = spawn_irc(vec![
        ":tmi.twitch.tv 001 justinfan1 :Welcome, GLHF!\r\n".to_string(),
        "PING :tmi.twitch.tv\r\n".to_string(),
        concat!(
            "@badge-info=;badges=broadcaster/1,subscriber/12;color=#1E90FF;",
            "display-name=Ascol57;emotes=25:6-10;id=abc-123;mod=0;user-id=42 ",
            ":ascol57!ascol57@ascol57.tmi.twitch.tv PRIVMSG #ascol57 :hello Kappa everyone\r\n"
        )
        .to_string(),
        // Escaped tag value: `\s` is a space.
        concat!(
            "@badges=;color=;display-name=nina\\sb;emotes=;id=def-456 ",
            ":nina!nina@nina.tmi.twitch.tv PRIVMSG #ascol57 :salut !\r\n"
        )
        .to_string(),
    ]);

    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus.clone()).unwrap();
    let mut fired = bus.subscribe();

    host.load_module(&build("../../modules/twitch", "twitch"), true)
        .expect("load twitch module (certified)");
    host.load_plugin(&build("../../plugins/twitch", "twitch_plugin"))
        .expect("load twitch plugin");

    // Never point at the real Twitch from a test.
    host.call_module("twitch", "set-chat-url", json!({ "url": url }))
        .expect("set-chat-url");
    host.call_module("twitch", "connect-chat", json!({ "channel": "Ascol57" }))
        .expect("connect-chat");

    // The plugin republishes `twitch.chat` on every message.
    let messages = eventually(|| match state.get("twitch.chat") {
        Some(Value::Array(list)) if list.len() >= 2 => Some(list),
        _ => None,
    });

    let first = &messages[0];
    assert_eq!(first["displayName"], json!("Ascol57"));
    assert_eq!(first["user"], json!("ascol57"));
    assert_eq!(first["color"], json!("#1E90FF"));
    assert_eq!(first["text"], json!("hello Kappa everyone"));
    // `set/version` is kept whole: the version indexes the badge image map, so
    // `subscriber/12` must not be flattened to `subscriber`.
    assert_eq!(first["badges"], json!(["broadcaster/1", "subscriber/12"]));
    // `emotes=25:6-10` → the emote spans "Kappa".
    assert_eq!(first["emotes"], json!([{ "id": "25", "start": 6, "end": 10 }]));

    // `\s` in a tag value decodes to a space.
    assert_eq!(messages[1]["displayName"], json!("nina b"));
    assert_eq!(messages[1]["text"], json!("salut !"));
    // An empty `color` is left empty — the widget picks a stable hue itself.
    assert_eq!(messages[1]["color"], json!(""));

    // The IRC login must actually have reached the server. `ws-connect` used to
    // return a handle before the socket existed, so these were dropped and the
    // chat stayed silent for ever.
    let login = said.lock().unwrap().join("");
    assert!(login.contains("CAP REQ :twitch.tv/tags"), "no CAP REQ sent: {login:?}");
    assert!(login.contains("NICK justinfan"), "no NICK sent: {login:?}");
    assert!(login.contains("JOIN #ascol57"), "no JOIN sent: {login:?}");

    // …and each message fired the trigger a profile can bind to.
    let event = fired.try_recv().expect("a chat-message trigger");
    assert_eq!(event.id, "twitch.chat-message");
    assert_eq!(event.context["displayName"], json!("Ascol57"));
}
