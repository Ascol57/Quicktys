//! `quicktys-install` — the runtime install engine (marketplace M6.2).
//!
//! It materializes a `.qtpkg` (see `quicktys-pkg`) into `<data>/installed/<kind>s/
//! <id>/` — the exact directory shape the boot loader already scans — updates the
//! `<data>/installed.json` **lockfile** (resolved versions, content hashes,
//! **reference counts**), and reports what to hot-load. It is transport/host-free:
//! verify + extract + bookkeeping only. The caller (the app) does the actual WASM
//! hot-load and locale/widget refresh (it owns the running host).
//!
//! Trust today is **hash-only** (`certified: false`): signature verification is
//! M6.5, so a sideloaded module that declares system capabilities is written with
//! `certified: false` and the host refuses it at load — the intended safe default.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod consent;
pub mod entitlement;
pub mod payload;
pub use consent::{ConsentRecord, ConsentStore};
pub use entitlement::EntitlementStore;
pub use payload::{decrypt_payload, encrypt_payload};
pub use quicktys_pkg::{Kind, LicenseModel, Manifest};

/// Where a package came from (drives UI + orphan policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Downloaded from a registry.
    Registry,
    /// Installed from a local `.qtpkg` file.
    Sideload,
    /// A dev bundle (built in-tree) — recorded, never written by this engine.
    Dev,
}

/// One installed package, as recorded in the lockfile (and surfaced to the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub id: String,
    /// `plugin` | `module` | `widget` | `content`.
    pub kind: String,
    pub version: String,
    pub content_hash: String,
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Module ids this package requires.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Slugs of installed packages that depend on this one (**ref count**).
    #[serde(default)]
    pub required_by: Vec<String>,
    /// `true` if the user asked for this package directly; `false` if it was
    /// pulled in as a dependency. A dependency with no dependents left is an
    /// **orphan** (safe to purge); an explicit install never is (apt/cargo model).
    #[serde(default = "default_true")]
    pub explicit: bool,
}

fn default_true() -> bool {
    true
}

impl InstalledRecord {
    /// `"<kind>/<id>"` — the lockfile key.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.kind, self.id)
    }
}

/// The install lockfile (`installed.json`): the source of truth for what is
/// installed, at which version, and who depends on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub schema: u32,
    /// slug (`<kind>/<id>`) → record.
    #[serde(default)]
    pub packages: BTreeMap<String, InstalledRecord>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Lockfile {
            schema: 1,
            packages: BTreeMap::new(),
        }
    }
}

impl Lockfile {
    /// Load from `<data>/installed.json`, or a fresh empty lockfile if absent.
    pub fn load(data_root: &Path) -> Result<Lockfile, InstallError> {
        let path = data_root.join(LOCKFILE_NAME);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Lockfile::default()),
            Err(e) => Err(InstallError::Io(e)),
        }
    }

    /// Persist to `<data>/installed.json` (atomically: tmp + rename).
    pub fn save(&self, data_root: &Path) -> Result<(), InstallError> {
        let path = data_root.join(LOCKFILE_NAME);
        let tmp = data_root.join(format!("{LOCKFILE_NAME}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// What to hot-load after a successful install.
#[derive(Debug, Clone)]
pub struct InstallOutcome {
    pub record: InstalledRecord,
    /// The installed bundle dir (`<data>/installed/<kind>s/<id>`).
    pub dir: PathBuf,
    pub kind: Kind,
    /// The component file name (plugins/modules), for the host loader.
    pub wasm: Option<String>,
    /// `certified` verdict written to `bundle.json` (hash-only ⇒ false for now).
    pub certified: bool,
    /// The catalogued widget manifest (kind == widget), ready for `list_widgets`.
    pub widget_manifest: Option<serde_json::Value>,
}

const LOCKFILE_NAME: &str = "installed.json";
const BUNDLE_DESCRIPTOR: &str = "bundle.json";
const SIGNATURE_FILE: &str = "signature.json";

/// Errors from the install engine.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("package error: {0}")]
    Pkg(#[from] quicktys_pkg::PkgError),
    #[error("content hash mismatch: expected {expected}, got {got}")]
    HashMismatch { expected: String, got: String },
    #[error("refusing to overwrite installed `{0}` published by a different party")]
    PublisherConflict(String),
    #[error("signature rejected: {0}")]
    SignatureRejected(String),
    #[error("signature decode: {0}")]
    Cert(#[from] quicktys_cert::CertError),
    #[error("`{0}` is a paid package — no valid license entitlement")]
    NotEntitled(String),
    #[error("payload decryption failed: {0}")]
    Payload(String),
}

/// Install a `.qtpkg` under `data_root` and record it in the lockfile.
///
/// `expected` (when `Some`) is checked against the archive's `content_hash` — the
/// registry path passes the index-advertised hash; sideload passes `None`. The
/// bundle is extracted **atomically** (`.tmp/… → rename`) into
/// `<data>/installed/<kind>s/<id>/`.
#[allow(clippy::too_many_arguments)]
pub fn install(
    archive: &[u8],
    data_root: &Path,
    source: Source,
    expected: Option<&str>,
    explicit: bool,
    trust: &quicktys_cert::TrustStore,
    now: u64,
) -> Result<InstallOutcome, InstallError> {
    let content_hash = quicktys_pkg::content_hash(archive);
    if let Some(expected) = expected {
        if expected != content_hash {
            return Err(InstallError::HashMismatch {
                expected: expected.to_string(),
                got: content_hash,
            });
        }
    }

    let manifest = quicktys_pkg::read_manifest(archive)?;
    let installed_root = data_root.join("installed");
    let kind_dir = installed_root.join(format!("{}s", manifest.kind.as_str()));
    let dest = kind_dir.join(&manifest.id);

    // Guard against id hijack: don't silently overwrite another publisher's dir.
    let lock = Lockfile::load(data_root)?;
    let slug = manifest.slug();
    if let Some(prev) = lock.packages.get(&slug) {
        if prev.publisher != manifest.publisher {
            return Err(InstallError::PublisherConflict(slug));
        }
    }

    // Entitlement gate (M6.8): a paid/subscription package needs a covering,
    // store-signed license verified against the pinned store keys.
    if manifest.license.model != LicenseModel::Free {
        let entitlements = EntitlementStore::load(data_root)?;
        if !entitlements.is_entitled(manifest.kind.as_str(), &manifest.id, &trust.store_keys, now) {
            return Err(InstallError::NotEntitled(slug));
        }
    }

    // Atomic extraction: unpack into a private temp dir, then swap it into place.
    std::fs::create_dir_all(&kind_dir)?;
    let staging = installed_root.join(".tmp");
    std::fs::create_dir_all(&staging)?;
    let tmp = staging.join(format!("{}-{}", manifest.id, unique()));
    let _ = std::fs::remove_dir_all(&tmp);
    quicktys_pkg::unpack(archive, &tmp)?;

    // Provenance (M6.5): if the package ships a `signature.json`, verify the
    // CA→publisher→release chain against the pinned trust store and compute
    // `certified` from crypto (never a plain flag). A present-but-invalid
    // signature is tampering → refuse. Unsigned → uncertified (fine for a
    // sandboxed plugin; a module then can't be granted system caps at load).
    let certified = match std::fs::read(tmp.join(SIGNATURE_FILE)) {
        Ok(sig_bytes) => {
            let sig = quicktys_cert::parse_signature(&sig_bytes)?;
            let manifest_bytes = std::fs::read(tmp.join(quicktys_pkg::MANIFEST_NAME))?;
            let manifest_hash = quicktys_pkg::hash_bytes(&manifest_bytes);
            match quicktys_cert::verify(&sig, &manifest, Some(&manifest_hash), trust, now) {
                quicktys_cert::Verdict::Certified { .. } => true,
                quicktys_cert::Verdict::Rejected(reason) => {
                    let _ = std::fs::remove_dir_all(&tmp);
                    return Err(InstallError::SignatureRejected(reason));
                }
                quicktys_cert::Verdict::Unsigned => false,
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(InstallError::Io(e)),
    };

    // Boot reads `bundle.json` ({id,kind,wasm,certified}); write it from the pkg
    // manifest unless the package already ships one (widgets author their own,
    // richer descriptor — leave it intact).
    if !tmp.join(BUNDLE_DESCRIPTOR).exists() {
        std::fs::write(
            tmp.join(BUNDLE_DESCRIPTOR),
            serde_json::to_vec_pretty(&manifest.to_bundle_json(certified))?,
        )?;
    }

    // Forensic watermark (M6.10): weave a per-buyer opaque id into the text
    // assets *after* provenance is verified (the mark is per-delivery, so it can't
    // be part of the signed bytes). The seed is the covering entitlement's
    // account/sku (or "anonymous" for a free watermarked package).
    if manifest.watermark {
        let seed = EntitlementStore::load(data_root)?
            .seed_for(manifest.kind.as_str(), &manifest.id, &trust.store_keys, now)
            .unwrap_or_else(|| "anonymous".to_string());
        let id = quicktys_crypto::watermark::derive_id(&seed, &slug);
        let _ = watermark_dir(&tmp, &id);
    }

    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::rename(&tmp, &dest)?;

    // Update the lockfile: upsert this package, then recompute all ref counts.
    let mut lock = lock;
    let requires: Vec<String> = manifest.requires.iter().map(|r| r.module.clone()).collect();
    // An explicit install is sticky: a package already wanted directly stays
    // explicit even if a later install is a dependency pull.
    let explicit = explicit || lock.packages.get(&slug).is_some_and(|p| p.explicit);
    lock.packages.insert(
        slug.clone(),
        InstalledRecord {
            id: manifest.id.clone(),
            kind: manifest.kind.as_str().to_string(),
            version: manifest.version.clone(),
            content_hash,
            source,
            publisher: manifest.publisher.clone(),
            requires,
            required_by: Vec::new(),
            explicit,
        },
    );
    recompute_refs(&mut lock.packages);
    lock.save(data_root)?;

    let widget_manifest = if manifest.kind == Kind::Widget {
        Some(build_widget_manifest(&dest)?)
    } else {
        None
    };

    Ok(InstallOutcome {
        record: lock.packages[&slug].clone(),
        dir: dest,
        kind: manifest.kind,
        wasm: manifest.wasm.clone(),
        certified,
        widget_manifest,
    })
}

/// Remove an installed package: delete its dir and lockfile entry, recompute ref
/// counts, and return the slugs of modules that became **orphans** (nothing
/// depends on them anymore) — candidates the caller may offer to purge.
pub fn uninstall(slug: &str, data_root: &Path) -> Result<Vec<String>, InstallError> {
    let mut lock = Lockfile::load(data_root)?;
    let Some(record) = lock.packages.remove(slug) else {
        return Ok(Vec::new());
    };
    let dest = data_root
        .join("installed")
        .join(format!("{}s", record.kind))
        .join(&record.id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    recompute_refs(&mut lock.packages);
    let orphans = orphans(&lock.packages);
    lock.save(data_root)?;
    Ok(orphans)
}

/// The installed packages, from the lockfile.
pub fn list(data_root: &Path) -> Result<Vec<InstalledRecord>, InstallError> {
    Ok(Lockfile::load(data_root)?.packages.into_values().collect())
}

/// Remove every orphaned module (a dependency-installed module nothing depends on
/// anymore). Returns the slugs actually removed. Iterates to a fixed point, since
/// removing one module can orphan another it pulled in.
pub fn purge_orphans(data_root: &Path) -> Result<Vec<String>, InstallError> {
    let mut removed = Vec::new();
    loop {
        let lock = Lockfile::load(data_root)?;
        let next = orphans(&lock.packages);
        if next.is_empty() {
            break;
        }
        for slug in next {
            uninstall(&slug, data_root)?;
            removed.push(slug);
        }
    }
    Ok(removed)
}

/// Modules that are dependency-installed (`!explicit`) with no remaining
/// dependents — the packages `purge_orphans` may safely remove.
fn orphans(pkgs: &BTreeMap<String, InstalledRecord>) -> Vec<String> {
    pkgs.values()
        .filter(|r| r.kind == "module" && !r.explicit && r.required_by.is_empty())
        .map(InstalledRecord::slug)
        .collect()
}

/// Text asset extensions eligible for watermarking (never `*.json` — that would
/// break signature.json / manifest.json).
const WATERMARK_EXTS: &[&str] = &["html", "htm", "js", "mjs", "css", "svg"];

/// Weave a per-buyer watermark `id` into every text asset under `dir` (M6.10).
/// Returns how many files were marked.
pub fn watermark_dir(dir: &Path, id: &str) -> Result<usize, InstallError> {
    let mut count = 0;
    watermark_walk(dir, id, &mut count)?;
    Ok(count)
}

fn watermark_walk(dir: &Path, id: &str, count: &mut usize) -> Result<(), InstallError> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            watermark_walk(&path, id, count)?;
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !WATERMARK_EXTS.contains(&ext) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            std::fs::write(&path, quicktys_crypto::watermark::embed(&text, id))?;
            *count += 1;
        }
    }
    Ok(())
}

/// Rebuild every `required_by` from the plugins' `requires` (a plugin requiring
/// module `m` adds its own slug to `module/m`'s `required_by`).
fn recompute_refs(pkgs: &mut BTreeMap<String, InstalledRecord>) {
    for r in pkgs.values_mut() {
        r.required_by.clear();
    }
    let edges: Vec<(String, String)> = pkgs
        .values()
        .filter(|r| r.kind == "plugin")
        .flat_map(|p| {
            let plugin_slug = p.slug();
            p.requires
                .iter()
                .map(move |m| (format!("module/{m}"), plugin_slug.clone()))
        })
        .collect();
    for (module_slug, plugin_slug) in edges {
        if let Some(m) = pkgs.get_mut(&module_slug) {
            if !m.required_by.contains(&plugin_slug) {
                m.required_by.push(plugin_slug);
            }
        }
    }
}

/// Build a widget's catalogue manifest for `list_widgets`: its authored
/// `bundle.json` plus any embedded `index.html` / preview page (the host does not
/// run widgets; the frontend renders them in a sandboxed iframe).
pub fn build_widget_manifest(dir: &Path) -> Result<serde_json::Value, InstallError> {
    let text = std::fs::read_to_string(dir.join(BUNDLE_DESCRIPTOR))?;
    let mut manifest: serde_json::Value = serde_json::from_str(&text)?;
    if let Some(obj) = manifest.as_object_mut() {
        if let Ok(html) = std::fs::read_to_string(dir.join("index.html")) {
            obj.insert("html".into(), serde_json::Value::String(html));
        }
        if let Some(file) = obj.get("preview").and_then(|v| v.as_str()).map(str::to_string) {
            if let Ok(html) = std::fs::read_to_string(dir.join(&file)) {
                obj.insert("previewHtml".into(), serde_json::Value::String(html));
            }
        }
    }
    Ok(manifest)
}

/// A filesystem-unique suffix for staging dirs, without pulling in rand/time.
fn unique() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!("{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    use quicktys_cert::TrustStore;

    // An empty trust store (no pinned CA): packages verify as uncertified — the
    // default that keeps hash-only sideload safe (a capped module stays gated).
    fn no_trust() -> TrustStore {
        TrustStore::default()
    }

    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("fminstall-{}", unique()));
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Pack a minimal bundle dir into a `.qtpkg` for the given manifest JSON.
    fn pkg(manifest_json: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
        let src = TmpDir::new();
        std::fs::write(src.0.join("manifest.json"), manifest_json).unwrap();
        for (rel, bytes) in extra {
            let p = src.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
        quicktys_pkg::pack_dir(&src.0).unwrap()
    }

    fn plugin_pkg(id: &str, requires: &str) -> Vec<u8> {
        pkg(
            &format!(
                r#"{{"id":"{id}","kind":"plugin","version":"1.0.0","wasm":"{id}.wasm","requires":[{requires}]}}"#
            ),
            &[(&format!("{id}.wasm"), b"\0asm")],
        )
    }

    /// Build a **signed** package: `manifest.json` + wasm + a `signature.json`
    /// carrying a full CA→publisher→release chain over the manifest's hash.
    fn signed_pkg(
        ca: &quicktys_crypto::SigningKey,
        manifest_json: &str,
        caps_ceiling: &[&str],
    ) -> Vec<u8> {
        use quicktys_cert::{to_hex, PublisherCert, Release, SignatureFile};
        let suite = quicktys_crypto::SUITE_ID.to_string();
        let publisher = quicktys_crypto::SigningKey::generate().unwrap();
        let m: Manifest = serde_json::from_str(manifest_json).unwrap();
        let manifest_hash = quicktys_pkg::hash_bytes(manifest_json.as_bytes());

        let cert = PublisherCert {
            publisher_id: "acme".to_string(),
            publisher_key: to_hex(&publisher.verifying_key().to_bytes()),
            caps_ceiling: caps_ceiling.iter().map(|s| s.to_string()).collect(),
            not_after: 2_000_000_000,
            suite: suite.clone(),
        };
        let ca_sig = to_hex(
            &ca.sign(&serde_json::to_vec(&cert).unwrap())
                .unwrap()
                .to_bytes(),
        );
        let release = Release {
            id: m.id.clone(),
            version: m.version.clone(),
            kind: m.kind.as_str().to_string(),
            capabilities: m.capabilities.clone(),
            manifest_hash,
            not_after: None,
            suite: suite.clone(),
        };
        let pub_sig = to_hex(
            &publisher
                .sign(&serde_json::to_vec(&release).unwrap())
                .unwrap()
                .to_bytes(),
        );
        let sig = SignatureFile {
            suite,
            release,
            pub_sig,
            cert,
            ca_sig,
        };
        pkg(
            manifest_json,
            &[
                (&format!("{}.wasm", m.id), b"\0asm"),
                ("signature.json", serde_json::to_string(&sig).unwrap().as_bytes()),
            ],
        )
    }

    fn trust_for(ca: &quicktys_crypto::SigningKey) -> TrustStore {
        TrustStore {
            ca_keys: vec![ca.verifying_key()],
            registry_keys: Vec::new(),
            store_keys: Vec::new(),
            crl: quicktys_cert::Crl::default(),
        }
    }

    const NOW: u64 = 1_754_000_000;

    #[test]
    fn signed_package_verifies_as_certified() {
        let data = TmpDir::new();
        let ca = quicktys_crypto::SigningKey::generate().unwrap();
        let m = r#"{"id":"obs","kind":"module","version":"1.0.0","wasm":"obs.wasm","capabilities":["net"],"publisher":"acme"}"#;
        let archive = signed_pkg(&ca, m, &["net", "audio"]);
        let out = install(&archive, &data.0, Source::Registry, None, true, &trust_for(&ca), NOW).unwrap();
        assert!(out.certified);
        let bundle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(data.0.join("installed/modules/obs/bundle.json")).unwrap())
                .unwrap();
        assert_eq!(bundle["certified"], true);
    }

    #[test]
    fn signed_package_untrusted_ca_is_refused() {
        let data = TmpDir::new();
        let ca = quicktys_crypto::SigningKey::generate().unwrap();
        let attacker = quicktys_crypto::SigningKey::generate().unwrap();
        let m = r#"{"id":"obs","kind":"module","version":"1.0.0","wasm":"obs.wasm","capabilities":["net"],"publisher":"acme"}"#;
        let archive = signed_pkg(&ca, m, &["net"]);
        // Pin a different CA → the chain does not verify → refuse, nothing written.
        let err = install(&archive, &data.0, Source::Registry, None, true, &trust_for(&attacker), NOW)
            .unwrap_err();
        assert!(matches!(err, InstallError::SignatureRejected(_)));
        assert!(!data.0.join("installed/modules/obs").exists());
    }

    #[test]
    fn watermark_woven_into_installed_assets() {
        let data = TmpDir::new();
        let widget = pkg(
            r#"{"id":"clock","kind":"widget","version":"1.0.0","watermark":true}"#,
            &[
                ("bundle.json", br#"{"id":"clock","kind":"widget","name":"Clock","template":"info"}"#),
                ("index.html", b"<!doctype html><body><h1>Clock</h1><p>a widget body long enough to carry a forensic mark across the document</p></body>"),
            ],
        );
        install(&widget, &data.0, Source::Sideload, None, true, &no_trust(), NOW).unwrap();
        let html = std::fs::read_to_string(data.0.join("installed/widgets/clock/index.html")).unwrap();
        // No entitlement → the seed is "anonymous"; the woven id is recoverable.
        let id = quicktys_crypto::watermark::derive_id("anonymous", "widget/clock");
        assert_eq!(quicktys_crypto::watermark::extract(&html).as_deref(), Some(id.as_str()));
    }

    #[test]
    fn unsigned_package_is_uncertified() {
        let data = TmpDir::new();
        let ca = quicktys_crypto::SigningKey::generate().unwrap();
        // Even with a pinned CA, a package with no signature.json stays uncertified.
        let out = install(&plugin_pkg("time", ""), &data.0, Source::Sideload, None, true, &trust_for(&ca), NOW)
            .unwrap();
        assert!(!out.certified);
    }

    #[test]
    fn install_places_bundle_and_writes_lockfile() {
        let data = TmpDir::new();
        let out = install(&plugin_pkg("time", ""), &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap();
        assert_eq!(out.kind, Kind::Plugin);
        assert_eq!(out.wasm.as_deref(), Some("time.wasm"));
        // Extracted to the boot-scanned shape, with a compat bundle.json.
        let dir = data.0.join("installed/plugins/time");
        assert!(dir.join("time.wasm").exists());
        let bundle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("bundle.json")).unwrap()).unwrap();
        assert_eq!(bundle["id"], "time");
        assert_eq!(bundle["kind"], "plugin");
        assert_eq!(bundle["certified"], false);
        // Lockfile records it.
        let recs = list(&data.0).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].slug(), "plugin/time");
        assert_eq!(recs[0].source, Source::Sideload);
    }

    #[test]
    fn ref_counting_tracks_dependents() {
        let data = TmpDir::new();
        // A module + two plugins that require it.
        let module = pkg(
            r#"{"id":"obs","kind":"module","version":"1.0.0","wasm":"obs.wasm"}"#,
            &[("obs.wasm", b"\0asm")],
        );
        // The module is pulled as a dependency (not explicit) → can become orphan.
        install(&module, &data.0, Source::Registry, None, false, &no_trust(), 0).unwrap();
        install(
            &plugin_pkg("obs-ctl", r#"{"module":"obs","version":"^1"}"#),
            &data.0,
            Source::Sideload,
            None,
            true,
            &no_trust(),
            0,
        )
        .unwrap();
        install(
            &plugin_pkg("obs-scenes", r#"{"module":"obs","version":"^1"}"#),
            &data.0,
            Source::Sideload,
            None,
            true,
            &no_trust(),
            0,
        )
        .unwrap();

        let lock = Lockfile::load(&data.0).unwrap();
        let obs = &lock.packages["module/obs"];
        let mut deps = obs.required_by.clone();
        deps.sort();
        assert_eq!(deps, ["plugin/obs-ctl", "plugin/obs-scenes"]);

        // Uninstall one dependent: module still required, not orphaned.
        let orphans = uninstall("plugin/obs-ctl", &data.0).unwrap();
        assert!(orphans.is_empty());
        // Uninstall the last dependent: module becomes an orphan.
        let orphans = uninstall("plugin/obs-scenes", &data.0).unwrap();
        assert_eq!(orphans, ["module/obs"]);
    }

    #[test]
    fn hash_mismatch_is_rejected_and_nothing_written() {
        let data = TmpDir::new();
        let err = install(
            &plugin_pkg("time", ""),
            &data.0,
            Source::Registry,
            Some("sha256:deadbeef"),
            true,
            &no_trust(),
            0,
        )
        .unwrap_err();
        assert!(matches!(err, InstallError::HashMismatch { .. }));
        assert!(!data.0.join("installed/plugins/time").exists());
        assert!(list(&data.0).unwrap().is_empty());
    }

    #[test]
    fn reinstall_overwrites_same_publisher_and_leaves_no_staging() {
        let data = TmpDir::new();
        install(&plugin_pkg("time", ""), &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap();
        // A newer version, same (absent) publisher.
        let v2 = pkg(
            r#"{"id":"time","kind":"plugin","version":"2.0.0","wasm":"time.wasm"}"#,
            &[("time.wasm", b"\0asm2")],
        );
        install(&v2, &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap();
        assert_eq!(list(&data.0).unwrap()[0].version, "2.0.0");
        assert_eq!(
            std::fs::read(data.0.join("installed/plugins/time/time.wasm")).unwrap(),
            b"\0asm2"
        );
        // No leftover staging dirs.
        let staging = data.0.join("installed/.tmp");
        let leftover = std::fs::read_dir(&staging)
            .map(|d| d.count())
            .unwrap_or(0);
        assert_eq!(leftover, 0);
    }

    #[test]
    fn widget_keeps_its_own_bundle_json_and_builds_manifest() {
        let data = TmpDir::new();
        // Widget package: a pkg manifest.json AND an authored widget bundle.json.
        let widget = pkg(
            r#"{"id":"clock","kind":"widget","version":"1.0.0"}"#,
            &[
                (
                    "bundle.json",
                    br#"{"id":"clock","kind":"widget","name":"Clock","template":"info","preview":"preview.html"}"#,
                ),
                ("index.html", b"<b>live</b>"),
                ("preview.html", b"<b>demo</b>"),
            ],
        );
        let out = install(&widget, &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap();
        let wm = out.widget_manifest.unwrap();
        assert_eq!(wm["name"], "Clock"); // authored bundle.json preserved
        assert_eq!(wm["html"], "<b>live</b>"); // index.html embedded
        assert_eq!(wm["previewHtml"], "<b>demo</b>"); // preview embedded
    }

    #[test]
    fn publisher_conflict_is_refused() {
        let data = TmpDir::new();
        let a = pkg(
            r#"{"id":"obs","kind":"module","version":"1.0.0","wasm":"obs.wasm","publisher":"acme"}"#,
            &[("obs.wasm", b"\0asm")],
        );
        install(&a, &data.0, Source::Registry, None, true, &no_trust(), 0).unwrap();
        let b = pkg(
            r#"{"id":"obs","kind":"module","version":"1.0.0","wasm":"obs.wasm","publisher":"evil"}"#,
            &[("obs.wasm", b"\0asm")],
        );
        let err = install(&b, &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap_err();
        assert!(matches!(err, InstallError::PublisherConflict(_)));
    }

    #[test]
    fn purge_removes_orphan_deps_but_keeps_explicit() {
        let data = TmpDir::new();
        // A dependency-installed module + an explicitly-installed module.
        let dep = pkg(
            r#"{"id":"http","kind":"module","version":"1.0.0","wasm":"http.wasm"}"#,
            &[("http.wasm", b"\0asm")],
        );
        let explicit_mod = pkg(
            r#"{"id":"audio","kind":"module","version":"1.0.0","wasm":"audio.wasm"}"#,
            &[("audio.wasm", b"\0asm")],
        );
        install(&dep, &data.0, Source::Registry, None, false, &no_trust(), 0).unwrap();
        install(&explicit_mod, &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap();
        install(
            &plugin_pkg("discord", r#"{"module":"http","version":"^1"}"#),
            &data.0,
            Source::Sideload,
            None,
            true,
            &no_trust(),
            0,
        )
        .unwrap();

        // With the plugin present, nothing is purgeable.
        assert!(purge_orphans(&data.0).unwrap().is_empty());
        // Remove the plugin → `http` is now an orphan dep; `audio` is explicit.
        uninstall("plugin/discord", &data.0).unwrap();
        let removed = purge_orphans(&data.0).unwrap();
        assert_eq!(removed, ["module/http"]);
        assert!(!data.0.join("installed/modules/http").exists());
        // The explicitly-installed module survives.
        let ids: Vec<String> = list(&data.0).unwrap().into_iter().map(|r| r.slug()).collect();
        assert_eq!(ids, ["module/audio"]);
    }

    #[test]
    fn explicit_install_promotes_a_dependency() {
        let data = TmpDir::new();
        let m = pkg(
            r#"{"id":"http","kind":"module","version":"1.0.0","wasm":"http.wasm"}"#,
            &[("http.wasm", b"\0asm")],
        );
        install(&m, &data.0, Source::Registry, None, false, &no_trust(), 0).unwrap(); // as dep
        install(&m, &data.0, Source::Sideload, None, true, &no_trust(), 0).unwrap(); // user asks for it
        // Now explicit → never an orphan even with no dependents.
        assert!(purge_orphans(&data.0).unwrap().is_empty());
        assert!(Lockfile::load(&data.0).unwrap().packages["module/http"].explicit);
    }
}
