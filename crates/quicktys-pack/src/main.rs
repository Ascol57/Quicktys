//! `quicktys-pack` — turn built extension **bundles** into pkg **sources** that
//! `quicktys-regserve` (or a real registry) can pack and serve.
//!
//! For each `<bundles>/{plugins,modules}/<id>/`, it instantiates the `.wasm` and
//! reads its real `init()` manifest (so `requires` / `capabilities` are exact,
//! not guessed), writes a pkg `manifest.json`, and copies the payload (wasm +
//! `lang/`). Widgets (no wasm) are copied with a synthesized manifest. The old
//! `bundle.json` is dropped for plugins/modules so the installer regenerates it
//! with a crypto-derived `certified` (M6.5); widgets keep their authored one.
//!
//! ```text
//! quicktys-pack <bundles_dir> [--out <dir>]     # default out: <bundles_dir>/../pkgs
//! ```

use std::path::{Path, PathBuf};

use quicktys_cert::{encode_license, issue_license, to_hex, Entitlement, PublisherCert, Release, SignatureFile};
use quicktys_core::{StateStore, TriggerBus};
use quicktys_crypto::{SigningKey, SUITE_ID};
use quicktys_host::WasmHost;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// All system caps — the dev CA's ceiling is permissive.
const ALL_CAPS: &[&str] = &["net", "input", "audio", "hid", "files", "process", "timers"];
/// Cert/release expiry for dev packages (year ~2100).
const FAR_FUTURE: u64 = 4_102_444_800;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut bundles = None;
    let mut out = None;
    let mut sign = false;
    let mut sign_only = false;
    let mut ca_path = None;
    let mut store_path = None;
    let mut paid: HashMap<String, String> = HashMap::new(); // slug -> sku
    let mut encrypt: std::collections::HashSet<String> = std::collections::HashSet::new(); // slugs
    let mut watermark: std::collections::HashSet<String> = std::collections::HashSet::new(); // slugs
    let mut issue: Option<(String, String, String)> = None; // (kind, id, sku)
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(it.next().ok_or_else(|| anyhow::anyhow!("--out needs a value"))?)),
            "--sign" => sign = true,
            // Sign already-packed pkg sources in place (no wasm re-inspection).
            "--sign-only" => {
                sign = true;
                sign_only = true;
            }
            "--ca" => ca_path = Some(PathBuf::from(it.next().ok_or_else(|| anyhow::anyhow!("--ca needs a value"))?)),
            "--store" => store_path = Some(PathBuf::from(it.next().ok_or_else(|| anyhow::anyhow!("--store needs a value"))?)),
            // Mark a package paid (license.model=paid) at packing time.
            "--paid" => {
                let (kind, id, sku) = parse_spec(it.next().ok_or_else(|| anyhow::anyhow!("--paid needs <kind/id[:sku]>"))?);
                paid.insert(format!("{kind}/{id}"), sku);
            }
            // Encrypt a package's wasm payload (T1, M6.9). Implies paid.
            "--encrypt" => {
                let (kind, id, sku) = parse_spec(it.next().ok_or_else(|| anyhow::anyhow!("--encrypt needs <kind/id[:sku]>"))?);
                let slug = format!("{kind}/{id}");
                paid.entry(slug.clone()).or_insert(sku);
                encrypt.insert(slug);
            }
            // Enable forensic watermarking of a package's text assets (M6.10).
            "--watermark" => {
                let (kind, id, _) = parse_spec(it.next().ok_or_else(|| anyhow::anyhow!("--watermark needs <kind/id>"))?);
                watermark.insert(format!("{kind}/{id}"));
            }
            // Issue a license key for a package (uses the dev store key).
            "--issue-license" => {
                issue = Some(parse_spec(it.next().ok_or_else(|| anyhow::anyhow!("--issue-license needs <kind/id[:sku]>"))?));
            }
            "-h" | "--help" => {
                println!("Usage: quicktys-pack <bundles_dir> [--out <dir>] [--sign] [--ca <keyfile>] [--paid <kind/id[:sku]>]\n       quicktys-pack <pkgs_dir> --sign-only [--ca <keyfile>]\n       quicktys-pack --issue-license <kind/id[:sku]> [--store <keyfile>]");
                return Ok(());
            }
            other => bundles = Some(PathBuf::from(other)),
        }
    }

    // `--issue-license`: sign an entitlement with the dev store key and print it.
    if let Some((kind, id, sku)) = issue {
        let store_path = store_path.unwrap_or_else(|| PathBuf::from("dist/store.key"));
        let store = load_or_create_ca(&store_path)?;
        std::fs::write(store_path.with_extension("pub"), to_hex(&store.verifying_key().to_bytes()))?;
        // For an encrypted package, include the content key so the app can decrypt.
        let secrets = out
            .clone()
            .unwrap_or_else(|| PathBuf::from("dist/pkgs"))
            .join("../secrets")
            .join(format!("{kind}-{id}.ck"));
        let key_material = std::fs::read_to_string(&secrets).ok().map(|s| s.trim().to_string());
        if key_material.is_some() {
            println!("(including content key from {})", secrets.display());
        }
        let lic = issue_license(
            &store,
            Entitlement { sku, kind, id, model: "paid".into(), not_after: None, account: None, key_material },
        )
        .map_err(|e| anyhow::anyhow!(e))?;
        println!(
            "pin the store key in the app:\n  QUICKTYS_STORE=$(cat {})\n\nlicense key (paste it in the app):\n{}",
            store_path.with_extension("pub").display(),
            encode_license(&lic)
        );
        return Ok(());
    }
    let bundles = bundles.ok_or_else(|| anyhow::anyhow!("missing <bundles_dir>\nUsage: quicktys-pack <bundles_dir> [--out <dir>] [--sign] [--ca <keyfile>]"))?;
    let out = out.unwrap_or_else(|| bundles.join("../pkgs"));

    // With `--sign`, build a dev CA→publisher signer so installed modules certify.
    let signer = if sign {
        let ca_path = ca_path.unwrap_or_else(|| out.join("../ca.key"));
        let ca = load_or_create_ca(&ca_path)?;
        // Persist the CA public key next to the signing key, so the app can pin it
        // conveniently: `QUICKTYS_CA=$(cat <ca>.pub)`.
        let pub_path = ca_path.with_extension("pub");
        std::fs::write(&pub_path, to_hex(&ca.verifying_key().to_bytes()))?;
        let s = Signer::new(&ca)?;
        println!(
            "signing with dev CA — pin it in the app:\n  QUICKTYS_CA=$(cat {})\n",
            pub_path.display()
        );
        Some(s)
    } else {
        None
    };

    // `--sign-only`: just add signature.json to already-packed pkg sources.
    if sign_only {
        let n = sign_existing(&bundles, signer.as_ref().unwrap())?;
        println!("signed {n} package(s) in {}", bundles.display());
        return Ok(());
    }

    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new()))?;
    let secrets_dir = out.join("../secrets");
    let mut count = 0;

    for kind in ["plugins", "modules"] {
        let dir = bundles.join(kind);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            match pack_wasm_bundle(&host, &entry.path(), &out, signer.as_ref(), &paid, &encrypt, &watermark, &secrets_dir) {
                Ok(slug) => {
                    println!("  ✓ {slug}");
                    count += 1;
                }
                Err(e) => eprintln!("  ✗ {}: {e}", entry.path().display()),
            }
        }
    }

    // Widgets are frontend-only (no wasm) — copy them with a synthesized manifest.
    if let Ok(entries) = std::fs::read_dir(bundles.join("widgets")) {
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            match pack_widget(&entry.path(), &out, &watermark) {
                Ok(slug) => {
                    println!("  ✓ {slug}");
                    count += 1;
                }
                Err(e) => eprintln!("  ✗ {}: {e}", entry.path().display()),
            }
        }
    }

    println!("packed {count} extension(s) into {}", out.display());
    Ok(())
}

/// A dev signing authority: a publisher key certified by the CA, ready to sign
/// each package's manifest hash (CA→publisher→release chain, M6.5).
struct Signer {
    publisher: SigningKey,
    cert: PublisherCert,
    ca_sig: String,
}

impl Signer {
    fn new(ca: &SigningKey) -> anyhow::Result<Signer> {
        let publisher = SigningKey::generate().map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let cert = PublisherCert {
            publisher_id: "quicktys-dev".to_string(),
            publisher_key: to_hex(&publisher.verifying_key().to_bytes()),
            caps_ceiling: ALL_CAPS.iter().map(|s| s.to_string()).collect(),
            not_after: FAR_FUTURE,
            suite: SUITE_ID.to_string(),
        };
        let ca_sig = to_hex(
            &ca.sign(&serde_json::to_vec(&cert)?)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .to_bytes(),
        );
        Ok(Signer { publisher, cert, ca_sig })
    }

    /// Build the `signature.json` for a package whose `manifest.json` is `bytes`.
    fn sign(&self, bytes: &[u8], id: &str, version: &str, kind: &str, caps: Vec<String>) -> anyhow::Result<SignatureFile> {
        let release = Release {
            id: id.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
            capabilities: caps,
            manifest_hash: quicktys_pkg::hash_bytes(bytes),
            not_after: None,
            suite: SUITE_ID.to_string(),
        };
        let pub_sig = to_hex(
            &self
                .publisher
                .sign(&serde_json::to_vec(&release)?)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .to_bytes(),
        );
        Ok(SignatureFile {
            suite: SUITE_ID.to_string(),
            release,
            pub_sig,
            cert: self.cert.clone(),
            ca_sig: self.ca_sig.clone(),
        })
    }
}

/// Add `signature.json` to every already-packed pkg source under `pkgs`
/// (`{plugins,modules}/<id>/manifest.json`), signing the exact on-disk manifest
/// bytes so the hash matches what the installer verifies.
fn sign_existing(pkgs: &Path, signer: &Signer) -> anyhow::Result<usize> {
    let mut n = 0;
    for kind in ["plugins", "modules"] {
        let Ok(entries) = std::fs::read_dir(pkgs.join(kind)) else {
            continue;
        };
        for e in entries.flatten().filter(|e| e.path().is_dir()) {
            let src = e.path();
            let Ok(bytes) = std::fs::read(src.join("manifest.json")) else {
                continue;
            };
            let m: Value = serde_json::from_slice(&bytes)?;
            let id = m.get("id").and_then(Value::as_str).unwrap_or_default();
            let version = m.get("version").and_then(Value::as_str).unwrap_or("0.0.0");
            let k = m.get("kind").and_then(Value::as_str).unwrap_or_default();
            let caps: Vec<String> = m
                .get("capabilities")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let sig = signer.sign(&bytes, id, version, k, caps)?;
            std::fs::write(src.join("signature.json"), serde_json::to_vec_pretty(&sig)?)?;
            println!("  ✓ signed {k}/{id}");
            n += 1;
        }
    }
    Ok(n)
}

/// Load the dev CA signing key from `path` (hex), or generate + save one.
fn load_or_create_ca(path: &Path) -> anyhow::Result<SigningKey> {
    if let Ok(hex) = std::fs::read_to_string(path) {
        let bytes = quicktys_cert::from_hex(hex.trim()).map_err(|e| anyhow::anyhow!(e))?;
        return SigningKey::from_bytes(&bytes).map_err(|e| anyhow::anyhow!("{e:?}"));
    }
    let ca = SigningKey::generate().map_err(|e| anyhow::anyhow!("{e:?}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, to_hex(&ca.to_bytes()))?;
    Ok(ca)
}

/// Inspect a plugin/module bundle, write pkg `manifest.json`, copy the payload.
#[allow(clippy::too_many_arguments)]
fn pack_wasm_bundle(
    host: &WasmHost,
    src: &Path,
    out: &Path,
    signer: Option<&Signer>,
    paid: &HashMap<String, String>,
    encrypt: &std::collections::HashSet<String>,
    watermark: &std::collections::HashSet<String>,
    secrets_dir: &Path,
) -> anyhow::Result<String> {
    // Find the component file (from bundle.json `wasm`, else `<dirname>.wasm`).
    let bundle: Value = std::fs::read(src.join("bundle.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let wasm_name = bundle
        .get("wasm")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}.wasm", dir_name(src)));
    let wasm_path = src.join(&wasm_name);
    anyhow::ensure!(wasm_path.exists(), "no wasm at {}", wasm_path.display());

    let (kind, m) = host.inspect(&wasm_path)?;
    let id = m.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
    let version = m.get("version").and_then(Value::as_str).unwrap_or("0.0.0");

    let mut manifest = json!({
        "id": id,
        "kind": kind,
        "version": version,
        "wasm": wasm_name,
    });
    // Plugins declare `requires`; modules declare `capabilities`.
    if let Some(reqs) = m.get("requires").filter(|v| v.as_array().is_some_and(|a| !a.is_empty())) {
        manifest["requires"] = reqs.clone();
    }
    if let Some(caps) = m.get("capabilities").filter(|v| v.as_array().is_some_and(|a| !a.is_empty())) {
        manifest["capabilities"] = caps.clone();
    }
    // Carry the settings schema through (informational; the host also reads it
    // from the component's `init()` at load).
    if let Some(settings) = m.get("settings").filter(|v| !v.is_null()) {
        manifest["settings"] = settings.clone();
    }
    // Mark the package paid if requested (the install gate then needs a license).
    if let Some(sku) = paid.get(&format!("{kind}/{id}")) {
        manifest["license"] = json!({ "model": "paid", "sku": sku });
    }
    if watermark.contains(&format!("{kind}/{id}")) {
        manifest["watermark"] = json!(true);
    }

    let dest = out.join(format!("{kind}s")).join(&id);
    copy_tree(src, &dest)?;
    // Drop the old descriptor so the installer regenerates `certified` from crypto.
    let _ = std::fs::remove_file(dest.join("bundle.json"));

    // Encrypt the payload (T1, M6.9): replace `<wasm>` with `<wasm>.enc`, declare
    // `protection`, and stash the content key for license issuance.
    if encrypt.contains(&format!("{kind}/{id}")) {
        let wasm_path = dest.join(&wasm_name);
        let plain = std::fs::read(&wasm_path)?;
        // Reuse the stashed content key if one exists, so re-packing doesn't
        // invalidate a license already issued/entered (stable key across builds).
        let ck_file = secrets_dir.join(format!("{kind}-{id}.ck"));
        let ck: [u8; 32] = std::fs::read_to_string(&ck_file)
            .ok()
            .and_then(|h| quicktys_cert::from_hex(h.trim()).ok())
            .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
            .unwrap_or_else(quicktys_crypto::random);
        let content_key_id = to_hex(&quicktys_crypto::random::<16>());
        let enc = quicktys_install::encrypt_payload(&plain, &ck, &content_key_id);
        std::fs::write(dest.join(format!("{wasm_name}.enc")), enc)?;
        std::fs::remove_file(&wasm_path)?;
        manifest["protection"] = json!({
            "tier": "encrypted",
            "cipher": "xchacha20-poly1305",
            "content_key_id": content_key_id,
            "enc": [wasm_name],
        });
        std::fs::create_dir_all(secrets_dir)?;
        std::fs::write(secrets_dir.join(format!("{kind}-{id}.ck")), to_hex(&ck))?;
    }

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    std::fs::write(dest.join("manifest.json"), &manifest_bytes)?;

    // Sign it (CA→publisher→release over the manifest hash) so the installer can
    // certify it against the pinned CA — required for a module's system caps.
    if let Some(signer) = signer {
        let caps: Vec<String> = manifest["capabilities"]
            .as_array()
            .map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let sig = signer.sign(&manifest_bytes, &id, version, &kind, caps)?;
        std::fs::write(dest.join("signature.json"), serde_json::to_vec_pretty(&sig)?)?;
    }
    Ok(format!("{kind}/{id}"))
}

/// Copy a widget bundle and synthesize its pkg manifest (keeps its bundle.json).
fn pack_widget(src: &Path, out: &Path, watermark: &std::collections::HashSet<String>) -> anyhow::Result<String> {
    let bundle: Value = serde_json::from_slice(&std::fs::read(src.join("bundle.json"))?)?;
    let id = bundle.get("id").and_then(Value::as_str).unwrap_or_else(|| dir_name(src)).to_string();
    let version = bundle.get("version").and_then(Value::as_str).unwrap_or("0.1.0");
    let mut manifest = json!({ "id": id, "kind": "widget", "version": version });
    if watermark.contains(&format!("widget/{id}")) {
        manifest["watermark"] = json!(true);
    }

    let dest = out.join("widgets").join(&id);
    copy_tree(src, &dest)?;
    std::fs::write(dest.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(format!("widget/{id}"))
}

fn dir_name(p: &Path) -> &str {
    p.file_name().and_then(|s| s.to_str()).unwrap_or("unknown")
}

/// Parse `kind/id[:sku]` → `(kind, id, sku)` (sku defaults to the id).
fn parse_spec(s: &str) -> (String, String, String) {
    let (path, sku) = s.split_once(':').unwrap_or((s, ""));
    let (kind, id) = path.split_once('/').unwrap_or(("", path));
    let sku = if sku.is_empty() { id.to_string() } else { sku.to_string() };
    (kind.to_string(), id.to_string(), sku)
}

/// Recursively copy `src` into `dest` (created fresh).
fn copy_tree(src: &Path, dest: &Path) -> anyhow::Result<()> {
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
