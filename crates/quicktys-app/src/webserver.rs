//! Local HTTP server for **browser-source overlays** (OBS) and remote interfaces.
//!
//! Serves each `webview` overlay resource as a self-contained page whose runtime
//! widgets are laid out by percentage on a transparent canvas, and streams live
//! reactive state to them over **Server-Sent Events** (`/events`) — the same
//! `postMessage` bridge the in-app `WidgetFrame` uses, so a widget renders
//! identically in the app and in OBS. One-directional by design (host → browser);
//! widget actions post back to `/action`.
//!
//! Dependency-free beyond tokio: a tiny hand-rolled HTTP/1.1 responder. Plain
//! `http://` on localhost (a browser source is local and trusted).

use crate::widget_asset;
use quicktys_core::StateStore;
use quicktys_ipc::AppBackend;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::error::RecvError;

/// Default port for the overlay server (the OBS browser-source URL host).
pub const DEFAULT_PORT: u16 = 8787;

pub struct OverlayServer {
    pub state: Arc<StateStore>,
    pub backend: Arc<AppBackend>,
    pub port: u16,
}

/// Accept loop. Binds `127.0.0.1:<port>`; logs and returns if the port is taken.
pub async fn serve(server: Arc<OverlayServer>) {
    // Bind to all interfaces so OBS (and other LAN devices) can reach the
    // browser-source overlays. LAN-open by design for a streamer tool.
    let listener = match TcpListener::bind(("0.0.0.0", server.port)).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("overlay server: bind 127.0.0.1:{} failed: {e}", server.port);
            return;
        }
    };
    tracing::info!("overlay server on http://127.0.0.1:{}", server.port);
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let s = server.clone();
                tokio::spawn(async move { handle(s, stream).await });
            }
            Err(_) => continue,
        }
    }
}

async fn handle(server: Arc<OverlayServer>, mut stream: TcpStream) {
    // Read request head (+ any buffered body bytes).
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        if let Some(p) = find(&buf, b"\r\n\r\n") {
            break p;
        }
        match stream.read(&mut tmp).await {
            Ok(0) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => return,
        }
        if buf.len() > 64 * 1024 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.lines();
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");

    match (method, path) {
        ("GET", "/") => respond(stream, "200 OK", "text/html; charset=utf-8", index_page(&server).into_bytes()).await,
        ("GET", "/events") => sse(server, stream).await,
        ("GET", "/state") => {
            let body = json!(server.state.snapshot()).to_string().into_bytes();
            respond(stream, "200 OK", "application/json", body).await
        }
        ("POST", "/action") => {
            let len = content_length(&head);
            let mut body = buf[header_end + 4..].to_vec();
            while body.len() < len {
                match stream.read(&mut tmp).await {
                    Ok(0) => break,
                    Ok(n) => body.extend_from_slice(&tmp[..n]),
                    Err(_) => break,
                }
            }
            // Answer with the outcome so a self-driving widget can show why it
            // is empty instead of just sitting there.
            match run_action(&server, &body).await {
                Ok(()) => respond(stream, "200 OK", "application/json", b"{\"ok\":true}".to_vec()).await,
                Err(message) => {
                    let body = json!({ "ok": false, "error": message }).to_string();
                    respond(stream, "200 OK", "application/json", body.into_bytes()).await
                }
            }
        }
        ("GET", p) if p.starts_with("/overlay/") => {
            let id = p.trim_start_matches("/overlay/").trim_end_matches('/');
            // The `webview` module owns overlay definitions.
            match server.backend.call_module("webview", "get-overlay", json!({ "id": id })) {
                Ok(overlay) if !overlay.is_null() => {
                    respond(stream, "200 OK", "text/html; charset=utf-8", overlay_page(&overlay).into_bytes()).await
                }
                _ => respond(stream, "404 Not Found", "text/plain", b"no such overlay".to_vec()).await,
            }
        }
        ("GET", p) if p.starts_with("/widget/") => {
            match widget_asset(p.trim_start_matches("/widget")) {
                Some((bytes, ct)) => respond(stream, "200 OK", ct, bytes).await,
                None => respond(stream, "404 Not Found", "text/plain", b"not found".to_vec()).await,
            }
        }
        _ => respond(stream, "404 Not Found", "text/plain", b"not found".to_vec()).await,
    }
}

/// Stream reactive state as Server-Sent Events: an initial snapshot, then one
/// event per changed key.
async fn sse(server: Arc<OverlayServer>, mut stream: TcpStream) {
    let mut rx = server.state.subscribe();
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                Cache-Control: no-cache\r\nConnection: keep-alive\r\n\
                Access-Control-Allow-Origin: *\r\n\r\n";
    if stream.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let snapshot = json!({ "type": "snapshot", "state": server.state.snapshot() });
    if stream.write_all(format!("data: {snapshot}\n\n").as_bytes()).await.is_err() {
        return;
    }
    loop {
        match rx.recv().await {
            Ok(key) => {
                let value = server.state.get(&key).unwrap_or(Value::Null);
                let msg = json!({ "type": "state", "key": key, "value": value });
                if stream.write_all(format!("data: {msg}\n\n").as_bytes()).await.is_err() {
                    return;
                }
            }
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return,
        }
    }
}

/// Run a widget-posted action (`{plugin, action, config}`) as a one-node sequence.
/// Run a widget-requested action. Returns the outcome so the overlay page can
/// surface it: a widget that drives itself (the Twitch chat asks the plugin to
/// join a channel) is dead in the water if the action fails, and swallowing the
/// error made that indistinguishable from "no messages yet".
async fn run_action(server: &OverlayServer, body: &[u8]) -> Result<(), String> {
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("bad request: {e}"))?;
    let (Some(plugin), Some(action)) = (
        v.get("plugin").and_then(Value::as_str),
        v.get("action").and_then(Value::as_str),
    ) else {
        return Err("plugin and action are required".to_string());
    };
    let root = json!({
        "kind": "action",
        "plugin": plugin,
        "action": action,
        "config": v.get("config").cloned().unwrap_or_else(|| json!({})),
    });
    server
        .backend
        .run_sequence(root)
        .await
        .map(|_| ())
        .map_err(|message| {
            tracing::warn!(%plugin, %action, "overlay action failed: {message}");
            message
        })
}

async fn respond(mut stream: TcpStream, status: &str, ct: &str, body: Vec<u8>) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(&body).await;
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|l| l.strip_prefix("Content-Length:").or_else(|| l.strip_prefix("content-length:")))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// A tiny index listing the available overlays (their browser-source URLs).
fn index_page(server: &OverlayServer) -> String {
    let overlays = server
        .backend
        .call_module("webview", "list-overlays", json!({}))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let items: String = overlays
        .iter()
        .map(|o| {
            let id = o.get("id").and_then(Value::as_str).unwrap_or("");
            let name = o.get("name").and_then(Value::as_str).unwrap_or(id);
            format!("<li><a href=\"/overlay/{id}\">{name}</a> <code>/overlay/{id}</code></li>")
        })
        .collect();
    format!(
        "<!doctype html><meta charset=utf-8><title>Quicktys overlays</title>\
         <body style=\"font-family:system-ui;background:#111;color:#eee;padding:2rem\">\
         <h1>Overlays</h1><ul>{items}</ul></body>"
    )
}

/// Generate the self-contained overlay page: it lays out each runtime widget as
/// a percentage-positioned iframe and bridges live state to it over SSE.
fn overlay_page(overlay: &Value) -> String {
    // Our own data, but guard against a stray `</script>` closing the block.
    let data = overlay.to_string().replace("</", "<\\/");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
<style>
  html,body{{margin:0;height:100%;background:transparent;overflow:hidden;font-family:system-ui,sans-serif}}
  #root{{position:relative;width:100vw;height:100vh}}
  .node{{position:absolute}}
  .node iframe{{width:100%;height:100%;border:0;display:block;background:transparent}}
</style></head><body><div id="root"></div>
<script>
const OVERLAY = {data};
let state = {{}};
const frames = [];
const root = document.getElementById('root');
for (const w of (OVERLAY.widgets || [])) {{
  const node = document.createElement('div');
  node.className = 'node';
  node.style.cssText = `left:${{w.x}}%;top:${{w.y}}%;width:${{w.w}}%;height:${{w.h}}%`;
  const f = document.createElement('iframe');
  f.setAttribute('scrolling', 'no');
  f.src = `/widget/${{encodeURIComponent(w.type)}}/index.html`;
  node.appendChild(f); root.appendChild(node);
  frames.push({{ f, config: w.config || {{}} }});
}}
function post(f, msg) {{ try {{ f.contentWindow && f.contentWindow.postMessage(msg, '*'); }} catch (e) {{}} }}
const pushState = (fr) => post(fr.f, {{ source: 'quicktys', type: 'state', state }});
const pushConfig = (fr) => post(fr.f, {{ source: 'quicktys', type: 'config', config: fr.config }});
window.addEventListener('message', (e) => {{
  const d = e.data; if (!d || d.source !== 'quicktys-widget') return;
  const fr = frames.find((x) => x.f.contentWindow === e.source);
  if (d.type === 'ready' && fr) {{ pushConfig(fr); pushState(fr); }}
  else if (d.type === 'action' && d.plugin && d.action) {{
    fetch('/action', {{ method: 'POST', headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ plugin: d.plugin, action: d.action, config: d.config || {{}} }}) }})
      .then((r) => r.json())
      .then((res) => {{
        if (res && res.ok === false) {{
          console.error('[quicktys] action ' + d.plugin + '.' + d.action + ' failed:', res.error);
          if (fr) post(fr.f, {{ source: 'quicktys', type: 'error', error: res.error }});
        }}
      }})
      .catch(() => {{}});
  }}
}});
const es = new EventSource('/events');
es.onmessage = (e) => {{
  let d; try {{ d = JSON.parse(e.data); }} catch (_) {{ return; }}
  if (d.type === 'snapshot') state = d.state || {{}};
  else if (d.type === 'state') state[d.key] = d.value;
  frames.forEach(pushState);
}};
</script></body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_header_terminator() {
        assert_eq!(find(b"GET / HTTP/1.1\r\n\r\nbody", b"\r\n\r\n"), Some(14));
        assert_eq!(find(b"no terminator", b"\r\n\r\n"), None);
    }

    #[test]
    fn parses_content_length_case_insensitively() {
        assert_eq!(content_length("POST /x\r\nContent-Length: 12\r\n"), 12);
        assert_eq!(content_length("POST /x\r\ncontent-length: 7\r\n"), 7);
        assert_eq!(content_length("GET /x\r\n"), 0);
    }

    #[test]
    fn overlay_page_embeds_widgets_and_escapes_script() {
        let overlay = json!({
            "id": "main",
            "widgets": [{ "type": "pear-music", "x": 5, "y": 5, "w": 40, "h": 25, "config": { "showTitle": true } }],
        });
        let html = overlay_page(&overlay);
        assert!(html.contains("const OVERLAY = "));
        assert!(html.contains("pear-music"));
        assert!(html.contains("new EventSource('/events')"));
        let tricky = json!({ "id": "x", "name": "</script><b>", "widgets": [] });
        assert!(!overlay_page(&tricky).contains("</script><b>"));
    }
}
