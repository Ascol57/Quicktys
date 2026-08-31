//! A minimal WebSocket client (`ws://` and `wss://`) backing the module
//! `net.ws-connect` capability. Enough to consume a push stream (song/state
//! updates, OBS, Twitch chat): HTTP Upgrade handshake, RFC 6455 frame decode
//! with control-frame handling (ping→pong), client-masked sends, and text
//! message reassembly across continuation frames.
//!
//! **TLS split-use.** A `WsConn` is read by a blocking reader thread while the
//! host writes from another (`net.send`). For plain TCP that is free — clone the
//! socket. A TLS session cannot be cloned: both directions share one
//! `ClientConnection`. So the reader does its *blocking* socket read **outside**
//! the session lock and only locks to feed the bytes into rustls; the writer
//! locks, encrypts, and flushes to its own socket clone. Neither can starve the
//! other on a blocked read.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// The socket under a WebSocket: plain TCP, or TLS over a shared session.
enum Transport {
    Plain(TcpStream),
    Tls {
        sock: TcpStream,
        conn: Arc<Mutex<rustls::ClientConnection>>,
    },
}

impl Transport {
    /// Read some plaintext. Blocks on the socket outside any lock (see the
    /// module docs) so a concurrent send is never blocked behind a slow feed.
    fn read_some(&mut self, out: &mut [u8]) -> Result<usize, String> {
        match self {
            Transport::Plain(s) => s.read(out).map_err(|e| format!("read: {e}")),
            Transport::Tls { sock, conn } => loop {
                // Anything already decrypted?
                {
                    let mut c = conn.lock().unwrap();
                    match c.reader().read(out) {
                        Ok(n) if n > 0 => return Ok(n),
                        Err(e) if e.kind() != std::io::ErrorKind::WouldBlock => {
                            return Err(format!("tls read: {e}"))
                        }
                        _ => {}
                    }
                }
                // Nothing buffered: block on the raw socket WITHOUT the lock.
                let mut raw = [0u8; 8192];
                let n = sock.read(&mut raw).map_err(|e| format!("read: {e}"))?;
                if n == 0 {
                    return Ok(0);
                }
                let mut c = conn.lock().unwrap();
                let mut cursor = &raw[..n];
                while !cursor.is_empty() {
                    let used = c.read_tls(&mut cursor).map_err(|e| format!("tls feed: {e}"))?;
                    if used == 0 {
                        break;
                    }
                    c.process_new_packets().map_err(|e| format!("tls: {e}"))?;
                }
            },
        }
    }

    fn write_all_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self {
            Transport::Plain(s) => s.write_all(bytes).map_err(|e| format!("write: {e}")),
            Transport::Tls { sock, conn } => {
                let mut c = conn.lock().unwrap();
                c.writer()
                    .write_all(bytes)
                    .map_err(|e| format!("tls write: {e}"))?;
                while c.wants_write() {
                    c.write_tls(sock).map_err(|e| format!("tls flush: {e}"))?;
                }
                sock.flush().map_err(|e| format!("flush: {e}"))
            }
        }
    }

    /// A second handle onto the same connection, for the write half.
    fn split(&self) -> Result<Transport, String> {
        Ok(match self {
            Transport::Plain(s) => {
                Transport::Plain(s.try_clone().map_err(|e| format!("clone socket: {e}"))?)
            }
            // The socket is cloned, but the TLS session is *shared*: one session
            // encrypts both directions.
            Transport::Tls { sock, conn } => Transport::Tls {
                sock: sock.try_clone().map_err(|e| format!("clone socket: {e}"))?,
                conn: conn.clone(),
            },
        })
    }
}

/// The shared rustls client config (bundled roots), built once.
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(
            rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .expect("rustls protocol versions")
            .with_root_certificates(roots)
            .with_no_client_auth(),
        )
    })
    .clone()
}

/// An open WebSocket connection.
pub struct WsConn {
    stream: Transport,
    buf: Vec<u8>,
    pos: usize,
}

/// Connect to `ws://` or `wss://` `host:port/path?query` and perform the
/// Upgrade handshake.
pub fn connect(url: &str) -> Result<WsConn, String> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        return Err("only ws:// and wss:// URLs are supported".to_string());
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "bad port".to_string())?),
        None => (authority, if tls { 443 } else { 80 }),
    };

    let tcp = TcpStream::connect((host, port)).map_err(|e| format!("connect: {e}"))?;
    let mut stream = if tls {
        let name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| "bad server name".to_string())?;
        let mut conn = rustls::ClientConnection::new(tls_config(), name)
            .map_err(|e| format!("tls: {e}"))?;
        // Finish the handshake before splitting, while we still own both halves.
        let mut sock = tcp;
        conn.complete_io(&mut sock)
            .map_err(|e| format!("tls handshake: {e}"))?;
        Transport::Tls {
            sock,
            conn: Arc::new(Mutex::new(conn)),
        }
    } else {
        Transport::Plain(tcp)
    };
    let key = handshake_key();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all_bytes(req.as_bytes())?;

    // Read response headers; keep any frame bytes that arrived in the same read.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        let n = stream.read_some(&mut tmp)?;
        if n == 0 {
            return Err("connection closed during handshake".to_string());
        }
        buf.extend_from_slice(&tmp[..n]);
    };
    let status_line = std::str::from_utf8(&buf[..header_end])
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");
    if !status_line.contains(" 101") {
        return Err(format!("handshake rejected: {status_line:?}"));
    }
    let leftover = buf[header_end + 4..].to_vec();
    Ok(WsConn {
        stream,
        buf: leftover,
        pos: 0,
    })
}

impl WsConn {
    /// Read the next full text/binary message, transparently answering pings and
    /// reassembling continuation frames. Errors on close or I/O failure.
    pub fn read_message(&mut self) -> Result<String, String> {
        let mut msg = Vec::new();
        loop {
            self.ensure(2)?;
            let b0 = self.buf[self.pos];
            let b1 = self.buf[self.pos + 1];
            self.pos += 2;
            let fin = b0 & 0x80 != 0;
            let opcode = b0 & 0x0f;
            let masked = b1 & 0x80 != 0;
            let mut len = (b1 & 0x7f) as usize;
            if len == 126 {
                self.ensure(2)?;
                let b = self.take(2);
                len = u16::from_be_bytes([b[0], b[1]]) as usize;
            } else if len == 127 {
                self.ensure(8)?;
                let b = self.take(8);
                len = u64::from_be_bytes(b.try_into().unwrap()) as usize;
            }
            let mask = if masked {
                self.ensure(4)?;
                Some(self.take(4))
            } else {
                None
            };
            self.ensure(len)?;
            let mut payload = self.take(len);
            if let Some(m) = mask {
                for (i, byte) in payload.iter_mut().enumerate() {
                    *byte ^= m[i % 4];
                }
            }
            match opcode {
                0x0..=0x2 => {
                    msg.extend_from_slice(&payload);
                    if fin {
                        return Ok(String::from_utf8_lossy(&msg).into_owned());
                    }
                }
                0x8 => return Err("closed by peer".to_string()), // close
                0x9 => send_frame(&mut self.stream, 0xA, &payload)?, // ping → pong
                0xA => {}                                         // pong
                _ => {}
            }
        }
    }

    /// A write-only handle to the same socket, for the `net.send` capability
    /// (module → server, e.g. the obs-websocket `Identify`/requests). Cloning the
    /// `TcpStream` lets the host write while the reader thread blocks on reads.
    pub fn sender(&self) -> Result<WsSender, String> {
        Ok(WsSender {
            stream: self.stream.split()?,
        })
    }

    /// Ensure at least `n` unconsumed bytes are buffered.
    fn ensure(&mut self, n: usize) -> Result<(), String> {
        // Compact occasionally so the buffer doesn't grow unbounded.
        if self.pos > 1 << 16 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
        let mut tmp = [0u8; 8192];
        while self.buf.len() - self.pos < n {
            let r = self.stream.read_some(&mut tmp)?;
            if r == 0 {
                return Err("connection closed".to_string());
            }
            self.buf.extend_from_slice(&tmp[..r]);
        }
        Ok(())
    }

    fn take(&mut self, n: usize) -> Vec<u8> {
        let out = self.buf[self.pos..self.pos + n].to_vec();
        self.pos += n;
        out
    }
}

/// The write half of a [`WsConn`] (a cloned socket). Sends client-masked text
/// frames — used by `net.send` so a driver can talk back to the server.
pub struct WsSender {
    stream: Transport,
}

impl WsSender {
    pub fn send_text(&mut self, text: &str) -> Result<(), String> {
        send_frame(&mut self.stream, 0x1, text.as_bytes())
    }
}

/// Write one masked WebSocket frame (client frames must be masked, RFC 6455).
fn send_frame(stream: &mut Transport, opcode: u8, payload: &[u8]) -> Result<(), String> {
    let mut frame = vec![0x80 | opcode]; // FIN + opcode
    let len = payload.len();
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len < 65536 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mask = mask_key();
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    stream.write_all_bytes(&frame)
}

/// A 16-byte handshake nonce, base64-encoded. Not security-sensitive (the server
/// only echoes it hashed), so a time+counter seed suffices.
fn handshake_key() -> String {
    base64(&seed_bytes())
}

fn mask_key() -> [u8; 4] {
    let s = seed_bytes();
    [s[0], s[1], s[2], s[3]]
}

fn seed_bytes() -> [u8; 16] {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = CTR.fetch_add(1, Ordering::Relaxed);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&nanos.to_le_bytes());
    out[8..].copy_from_slice(&c.to_le_bytes());
    out
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn text_frame(s: &str) -> Vec<u8> {
        let mut f = vec![0x81]; // FIN + text, server frames are unmasked
        f.push(s.len() as u8);
        f.extend_from_slice(s.as_bytes());
        f
    }

    #[test]
    fn handshakes_and_reads_text_frames() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
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
            assert!(String::from_utf8_lossy(&req).contains("Upgrade: websocket"));
            s.write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
            )
            .unwrap();
            // Two messages, including one sent right after the handshake bytes.
            s.write_all(&text_frame(r#"{"type":"PLAYER_INFO"}"#)).unwrap();
            s.write_all(&text_frame("hello")).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(200));
        });
        let mut conn = connect(&format!("ws://{addr}/api/v1/ws?token=x")).unwrap();
        assert_eq!(conn.read_message().unwrap(), r#"{"type":"PLAYER_INFO"}"#);
        assert_eq!(conn.read_message().unwrap(), "hello");
    }

    #[test]
    fn rejects_non_ws_scheme() {
        assert!(connect("http://x/y").is_err());
    }
}
