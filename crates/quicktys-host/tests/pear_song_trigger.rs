//! The `pear` "song changed" path, end to end: the module's `poll` service
//! detects a new song and emits `song-changed`; the plugin (subscribed) both
//! (a) mirrors the song's fields into `pear.*` reactive variables and (b) turns
//! the event into a host trigger (`pear.song-changed`) on the bus. Uses a
//! stand-in Pear API whose `/song` body the test controls, so we never touch a
//! real player.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Stand-in Pear API on a free port; `/song` returns whatever JSON `song` holds.
/// Enrichment endpoints (`/queue-info`, `/lyrics`) 404 so the plugin sees them
/// as absent (best-effort).
fn spawn_mock(song: Arc<Mutex<String>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { break };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let text = String::from_utf8_lossy(&buf);
            let first = text.lines().next().unwrap_or("");
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");
            let response = if path.starts_with("/auth/") {
                http_json(r#"{"accessToken":"tok-1"}"#)
            } else if method == "GET" && path.ends_with("/song") {
                http_json(&song.lock().unwrap())
            } else {
                "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n".to_string()
            };
            let _ = stream.write_all(response.as_bytes());
        }
    });
    base
}

fn http_json(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

/// Load the pear module + plugin against a stand-in API, returning the host, the
/// shared state store, and the `/song` body handle.
fn setup() -> (WasmHost, Arc<StateStore>, Arc<Mutex<String>>, Arc<TriggerBus>) {
    let song = Arc::new(Mutex::new(r#"{"title":"Song A"}"#.to_string()));
    let base = spawn_mock(song.clone());
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus.clone()).unwrap();
    host.load_module(&build("../../modules/pear", "pear"), true)
        .expect("load pear module");
    host.load_plugin(&build("../../plugins/pear", "pear_plugin"))
        .expect("load pear plugin");
    host.call_module("pear", "set-base-url", json!({ "url": base }))
        .expect("set-base-url");
    (host, state, song, bus)
}

#[tokio::test]
async fn first_poll_is_baseline_then_changes_fire_a_trigger() {
    let (host, _state, song, bus) = setup();
    let mut rx = bus.subscribe();

    // First poll establishes the baseline — NO trigger (launching Quicktys must
    // not fire "song changed").
    host.call_module("pear", "poll", json!({})).expect("poll A");
    assert!(rx.try_recv().is_err(), "the first poll must not fire the trigger");

    // Same song again: still nothing.
    host.call_module("pear", "poll", json!({})).expect("poll A again");
    assert!(rx.try_recv().is_err(), "no trigger when the song is unchanged");

    // Now the song actually changes → the trigger fires with the new title.
    *song.lock().unwrap() = r#"{"title":"Song B"}"#.to_string();
    host.call_module("pear", "poll", json!({})).expect("poll B");
    let ev = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("trigger within 1s")
        .expect("bus open");
    assert_eq!(ev.id, "pear.song-changed");
    assert_eq!(ev.context.get("title").and_then(|v| v.as_str()), Some("Song B"));
}

#[tokio::test]
async fn song_change_exposes_reactive_variables() {
    let (host, state, song, _bus) = setup();
    *song.lock().unwrap() = json!({
        "title": "Nightcall",
        "artist": "Kavinsky & Lovefoxxx",
        "imageSrc": "http://img/cover.jpg",
        "url": "http://music/watch?v=abc",
        "album": "OutRun",
        "videoId": "abc",
        "isPaused": false
    })
    .to_string();

    host.call_module("pear", "poll", json!({})).expect("poll");

    // The plugin mirrored the song's fields into `pear.*` variables.
    assert_eq!(state.get("pear.title"), Some(json!("Nightcall")));
    assert_eq!(state.get("pear.artist"), Some(json!("Kavinsky & Lovefoxxx")));
    assert_eq!(state.get("pear.artists"), Some(json!(["Kavinsky", "Lovefoxxx"])));
    assert_eq!(state.get("pear.cover"), Some(json!("http://img/cover.jpg")));
    assert_eq!(state.get("pear.url"), Some(json!("http://music/watch?v=abc")));
    assert_eq!(state.get("pear.album"), Some(json!("OutRun")));
    assert_eq!(state.get("pear.video-id"), Some(json!("abc")));
    // No `isPaused` in the payload → playing.
    assert_eq!(state.get("pear.playing"), Some(json!(true)));
    // Enrichment endpoints 404 in the stand-in → those vars are left unset
    // (rather than wiped), so a later pause-sync keeps any earlier values.
    assert_eq!(state.get("pear.lyrics"), None);
    assert_eq!(state.get("pear.playlist"), None);
}
