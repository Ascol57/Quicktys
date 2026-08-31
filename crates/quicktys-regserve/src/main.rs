//! `quicktys-regserve` — a tiny static **marketplace registry** server for
//! development (exercises M6.6 against real HTTP).
//!
//! It takes a directory of **package sources** (each a folder with a pkg
//! `manifest.json` + its files), packs each into a content-addressed `.qtpkg`,
//! generates `index.json`, optionally signs it, and serves everything over HTTP —
//! so `QUICKTYS_REGISTRY=http://<addr>:<port>` just works.
//!
//! ```text
//! quicktys-regserve <packages_dir> [--port 8000] [--addr 127.0.0.1]
//!                                  [--out <dir>] [--sign] [--static]
//! ```
//! Package sources live flat (`<dir>/<pkg>/`) or nested
//! (`<dir>/{plugins,modules,widgets}/<pkg>/`); each `<pkg>/` has a `manifest.json`
//! (a `signature.json` marks it certified). `--out` sets where blobs + index are
//! written (default: a temp dir); `--sign` signs `index.json` with a fresh
//! registry key and prints the public key to pin; `--static` skips packing and
//! serves `<packages_dir>` as a prebuilt registry.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use quicktys_registry::{IndexEntry, RegistryIndex, Req};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match Config::parse(&args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let root = if cfg.static_mode {
        cfg.packages.clone()
    } else {
        match build_registry(&cfg) {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("error building registry: {e}");
                return ExitCode::FAILURE;
            }
        }
    };

    let bind = format!("{}:{}", cfg.addr, cfg.port);
    let listener = match TcpListener::bind(&bind) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: cannot bind {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("registry root: {}", root.display());
    println!("serving on    http://{bind}");
    println!("point Quicktys at it:\n  QUICKTYS_REGISTRY=http://{bind} cargo run -p quicktys-app");

    for stream in listener.incoming().flatten() {
        let root = root.clone();
        std::thread::spawn(move || {
            let _ = handle(stream, &root);
        });
    }
    ExitCode::SUCCESS
}

const USAGE: &str = "Usage: quicktys-regserve <packages_dir> [--port N] [--addr IP] [--out DIR] [--sign] [--static]";

struct Config {
    packages: PathBuf,
    out: Option<PathBuf>,
    addr: String,
    port: u16,
    sign: bool,
    static_mode: bool,
}

impl Config {
    fn parse(args: &[String]) -> Result<Config, String> {
        let mut packages = None;
        let mut out = None;
        let mut addr = "127.0.0.1".to_string();
        let mut port = 8000u16;
        let mut sign = false;
        let mut static_mode = false;
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--port" => {
                    port = it.next().ok_or("--port needs a value")?.parse().map_err(|_| "bad --port")?
                }
                "--addr" => addr = it.next().ok_or("--addr needs a value")?.clone(),
                "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
                "--sign" => sign = true,
                "--static" => static_mode = true,
                "-h" | "--help" => return Err("help".to_string()),
                other if other.starts_with('-') => return Err(format!("unknown flag {other}")),
                other => packages = Some(PathBuf::from(other)),
            }
        }
        Ok(Config {
            packages: packages.ok_or("missing <packages_dir>")?,
            out,
            addr,
            port,
            sign,
            static_mode,
        })
    }
}

/// Pack every package source under `cfg.packages`, write blobs + `index.json`
/// (optionally signed) into the output dir, and return that dir.
fn build_registry(cfg: &Config) -> Result<PathBuf, String> {
    let out = cfg
        .out
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join(format!("quicktys-registry-{}", std::process::id())));
    let blobs = out.join("blobs");
    std::fs::create_dir_all(&blobs).map_err(|e| e.to_string())?;

    let sources = collect_packages(&cfg.packages)?;
    if sources.is_empty() {
        return Err(format!("no package sources (dirs with manifest.json) under {}", cfg.packages.display()));
    }

    let mut packages = Vec::new();
    for src in &sources {
        let entry = pack_one(src, &blobs)?;
        println!(
            "packed {}/{} @ {}  ({} o, {})",
            entry.kind,
            entry.id,
            entry.version,
            entry.size,
            if entry.certified { "signed" } else { "unsigned" }
        );
        packages.push(entry);
    }

    let index = RegistryIndex { schema: 1, packages };
    let index_bytes = serde_json::to_vec_pretty(&index).map_err(|e| e.to_string())?;
    std::fs::write(out.join("index.json"), &index_bytes).map_err(|e| e.to_string())?;

    if cfg.sign {
        let key = quicktys_crypto::SigningKey::generate().map_err(|e| format!("{e:?}"))?;
        let sig = key.sign(&index_bytes).map_err(|e| format!("{e:?}"))?;
        std::fs::write(out.join("index.sig"), to_hex(&sig.to_bytes())).map_err(|e| e.to_string())?;
        println!(
            "signed index.json — pin this registry key:\n  {}",
            to_hex(&key.verifying_key().to_bytes())
        );
    }
    println!("indexed {} package(s)", index.packages.len());
    Ok(out)
}

/// Pack a single package source dir into `blobs/<id>-<version>.qtpkg` and return
/// its index entry.
fn pack_one(src: &Path, blobs: &Path) -> Result<IndexEntry, String> {
    let manifest_bytes = std::fs::read(src.join("manifest.json"))
        .map_err(|e| format!("{}: {e}", src.display()))?;
    let manifest: quicktys_pkg::Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("{}: {e}", src.display()))?;

    let archive = quicktys_pkg::pack_dir(src).map_err(|e| e.to_string())?;
    let content_hash = quicktys_pkg::content_hash(&archive);
    // Include the kind so a plugin and a module sharing an id don't collide.
    let file = format!("{}-{}-{}.qtpkg", manifest.kind.as_str(), manifest.id, manifest.version);
    std::fs::write(blobs.join(&file), &archive).map_err(|e| e.to_string())?;

    Ok(IndexEntry {
        id: manifest.id.clone(),
        kind: manifest.kind.as_str().to_string(),
        version: manifest.version.clone(),
        publisher: manifest.publisher.clone(),
        certified: src.join("signature.json").exists(),
        content_hash,
        size: archive.len() as u64,
        url: format!("blobs/{file}"),
        requires: manifest
            .requires
            .iter()
            .map(|r| Req {
                module: r.module.clone(),
                version: r.version.clone(),
            })
            .collect(),
        capabilities: manifest.capabilities.clone(),
        min_host: manifest.min_host.clone(),
        name: None,
        summary: None,
        license: serde_json::to_value(&manifest.license).unwrap_or(serde_json::Value::Null),
    })
}

/// Find package sources: directories containing a `manifest.json`, at depth 1
/// (`<root>/<pkg>/`) or depth 2 (`<root>/<kind>/<pkg>/`).
fn collect_packages(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("manifest.json").is_file() {
            found.push(path);
        } else if let Ok(inner) = std::fs::read_dir(&path) {
            for c in inner.flatten() {
                let cp = c.path();
                if cp.is_dir() && cp.join("manifest.json").is_file() {
                    found.push(cp);
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Serve one request: `GET /<path>` → the file under `root`, else 404.
fn handle(mut stream: std::net::TcpStream, root: &Path) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > 16 * 1024 {
            break; // header too large
        }
    }
    let request = String::from_utf8_lossy(&buf);
    let path = request
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");
    let rel = path.trim_start_matches('/').split('?').next().unwrap_or("");

    match resolve(root, rel) {
        Some((bytes, ctype)) => {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            stream.write_all(head.as_bytes())?;
            stream.write_all(&bytes)?;
        }
        None => {
            let body = b"not found";
            let head = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes())?;
            stream.write_all(body)?;
        }
    }
    stream.flush()
}

/// Map a request path to a file under `root`, rejecting traversal. Returns the
/// bytes + a content type.
fn resolve(root: &Path, rel: &str) -> Option<(Vec<u8>, &'static str)> {
    if rel.split('/').any(|c| c == "..") {
        return None;
    }
    let target = if rel.is_empty() { root.join("index.json") } else { root.join(rel) };
    let bytes = std::fs::read(&target).ok()?;
    let ctype = match target.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("sig") => "text/plain",
        Some("qtpkg") => "application/octet-stream",
        _ => "application/octet-stream",
    };
    Some((bytes, ctype))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
