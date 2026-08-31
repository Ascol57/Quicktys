//! A minimal HTTP/1.1 client. Plain `http://` over `std::net::TcpStream`, and
//! `https://` over a rustls-wrapped stream (bundled roots via webpki-roots).
//!
//! Backs the module `net.fetch` capability and the OAuth token exchange. Frames
//! the response by `Content-Length`/chunked (never relies on the connection
//! closing), and is testable against a local server with no external deps.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// A parsed HTTP response.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Read+Write stream — either a plain `TcpStream` or a TLS-wrapped one.
trait Io: Read + Write {}
impl<T: Read + Write> Io for T {}

/// The shared rustls client config (bundled roots), built once.
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("rustls protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        Arc::new(cfg)
    })
    .clone()
}

/// Perform an HTTP request and read the full response.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> Result<Response, String> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("only http:// and https:// URLs are supported".to_string());
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
    tcp.set_read_timeout(Some(Duration::from_secs(15))).map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(Duration::from_secs(15))).map_err(|e| e.to_string())?;

    // Wrap in TLS for https://.
    let mut stream: Box<dyn Io> = if tls {
        let name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| "bad server name".to_string())?;
        let conn = rustls::ClientConnection::new(tls_config(), name)
            .map_err(|e| format!("tls: {e}"))?;
        Box::new(rustls::StreamOwned::new(conn, tcp))
    } else {
        Box::new(tcp)
    };

    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        head.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    head.push_str("\r\n");

    stream
        .write_all(head.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    if let Some(b) = body {
        stream.write_all(b).map_err(|e| format!("write body: {e}"))?;
    }
    // NOTE: we do NOT half-close the write side. Some servers run async handlers
    // (e.g. Pear's `/queue`, which round-trips over IPC) and abandon the response
    // if the client sends FIN first. We instead frame the body by Content-Length
    // / chunked, so we never rely on the connection closing.

    // Read up to and including the header terminator.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        if let Some(p) = find_sub(&buf, b"\r\n\r\n") {
            break p;
        }
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("connection closed before response headers".to_string());
        }
        buf.extend_from_slice(&tmp[..n]);
    };

    let header_text =
        std::str::from_utf8(&buf[..header_end]).map_err(|_| "non-utf8 headers".to_string())?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().ok_or_else(|| "empty response".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| format!("bad status line: {status_line:?}"))?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect();

    let pre = buf[header_end + 4..].to_vec();
    let body = read_body(&mut *stream, &headers, pre)?;
    Ok(Response {
        status,
        headers,
        body,
    })
}

fn header_val<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read the response body, framed by `Content-Length`, `Transfer-Encoding:
/// chunked`, or (last resort) until the server closes. `pre` is the bytes read
/// past the headers already.
fn read_body(
    stream: &mut dyn Io,
    headers: &[(String, String)],
    mut pre: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if header_val(headers, "transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
    {
        return decode_chunked(stream, pre);
    }
    let mut tmp = [0u8; 8192];
    if let Some(len) = header_val(headers, "content-length").and_then(|v| v.trim().parse::<usize>().ok())
    {
        while pre.len() < len {
            let n = stream.read(&mut tmp).map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                break;
            }
            pre.extend_from_slice(&tmp[..n]);
        }
        pre.truncate(len);
        return Ok(pre);
    }
    // No framing info: read until the peer closes.
    loop {
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        pre.extend_from_slice(&tmp[..n]);
    }
    Ok(pre)
}

/// Decode a `Transfer-Encoding: chunked` body. `pre` holds bytes already read.
fn decode_chunked(stream: &mut dyn Io, pre: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut src = pre;
    let mut pos = 0usize;
    let mut out = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        // Read a chunk-size line.
        let line_end = loop {
            if let Some(i) = find_sub(&src[pos..], b"\r\n") {
                break pos + i;
            }
            let n = stream.read(&mut tmp).map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                return Err("eof in chunk size".to_string());
            }
            src.extend_from_slice(&tmp[..n]);
        };
        let size_str = std::str::from_utf8(&src[pos..line_end]).map_err(|_| "bad chunk size")?;
        let size = usize::from_str_radix(size_str.trim().split(';').next().unwrap_or(""), 16)
            .map_err(|_| "bad chunk size".to_string())?;
        pos = line_end + 2;
        if size == 0 {
            break;
        }
        while src.len() < pos + size + 2 {
            let n = stream.read(&mut tmp).map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                return Err("eof in chunk data".to_string());
            }
            src.extend_from_slice(&tmp[..n]);
        }
        out.extend_from_slice(&src[pos..pos + size]);
        pos += size + 2; // skip data + trailing CRLF
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod test_server {
    //! A throwaway one-shot HTTP server for tests.
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;

    /// Read a full HTTP request: headers up to `\r\n\r\n`, then any
    /// Content-Length body (the client no longer signals EOF via a half-close).
    pub(crate) fn read_request(stream: &mut TcpStream) -> Vec<u8> {
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
        buf
    }

    /// Spawn a server that answers `count` requests with `response_body`,
    /// returning its base URL and a receiver of the raw request bytes.
    pub fn spawn(count: usize, response_body: &'static str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for _ in 0..count {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                // The client no longer half-closes, so read the request headers
                // (up to \r\n\r\n) + any Content-Length body, rather than to EOF.
                let buf = read_request(&mut stream);
                let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (format!("http://{addr}"), rx)
    }

    /// Like [`spawn`], but answers a **sequence** of `(status, body)` — needed by
    /// anything that polls, e.g. the OAuth device flow (device code, then
    /// `authorization_pending`, then the token).
    pub fn spawn_sequence(
        responses: Vec<(u16, String)>,
    ) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let buf = read_request(&mut stream);
                let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (format!("http://{addr}"), rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_round_trips_against_a_local_server() {
        let (base, requests) = test_server::spawn(1, r#"{"ok":true}"#);
        let resp = request("GET", &format!("{base}/hello"), &[], None).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, br#"{"ok":true}"#);
        let req = requests.recv().unwrap();
        assert!(req.starts_with("GET /hello HTTP/1.1"), "got: {req}");
    }

    #[test]
    fn post_sends_body() {
        let (base, requests) = test_server::spawn(1, "received");
        let resp = request(
            "POST",
            &format!("{base}/submit"),
            &[("Content-Type".into(), "application/json".into())],
            Some(b"{\"x\":1}"),
        )
        .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"received");
        let req = requests.recv().unwrap();
        assert!(req.contains("POST /submit"), "got: {req}");
        assert!(req.contains("{\"x\":1}"), "body missing: {req}");
    }

    #[test]
    fn rejects_unsupported_scheme() {
        assert!(request("GET", "ftp://example.com", &[], None).is_err());
        assert!(request("GET", "example.com", &[], None).is_err());
    }

    /// `https://` is now supported (rustls). Networked, so ignored by default.
    #[test]
    #[ignore = "needs network"]
    fn https_roundtrips() {
        let resp = request("GET", "https://example.com", &[], None).unwrap();
        assert_eq!(resp.status, 200);
        assert!(!resp.body.is_empty());
    }

    /// The bug that made `/queue` come back empty: a keep-alive server that never
    /// closes. The client must frame the body by Content-Length and return, not
    /// block waiting for EOF.
    #[test]
    fn reads_full_body_from_a_keep_alive_server() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = "x".repeat(100_000);
        let body2 = body.clone();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = test_server::read_request(&mut s);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                    body2.len(),
                    body2
                );
                let _ = s.write_all(resp.as_bytes());
                std::thread::sleep(Duration::from_secs(3)); // stay open, like Pear
            }
        });
        let resp = request("GET", &format!("http://{addr}/big"), &[], None).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body.len(), 100_000);
    }

    #[test]
    fn decodes_a_chunked_response() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = test_server::read_request(&mut s);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
                );
            }
        });
        let resp = request("GET", &format!("http://{addr}/c"), &[], None).unwrap();
        assert_eq!(resp.body, b"hello world");
    }
}
