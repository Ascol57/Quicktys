//! `quicktys-pkg` — the `.qtpkg` distributable package format (marketplace M6.1).
//!
//! A `.qtpkg` is a **deterministic** tar (entries sorted, metadata zeroed) wrapped
//! in zstd, of a bundle directory (`manifest.json`, the `.wasm`, `lang/`, …). Its
//! identity is `content_hash` = `sha256:` of the produced archive — the same bytes
//! the host re-hashes on receipt (it never re-packs to verify), so determinism is
//! for reproducible *builds*, not for verification.
//!
//! This crate is deliberately transport/crypto-free: it packs, unpacks, hashes and
//! reads the manifest. Signing (M6.5) and payload encryption (M6.9) layer on top —
//! provenance signs `content_hash`, so it is computed over the (possibly encrypted)
//! bytes and verifiable without any content key (see docs/marketplace.md §3,
//! docs/drm.md Annexe B).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

/// The manifest file name inside a package (a superset of the legacy `bundle.json`;
/// see [`Manifest::to_bundle_json`] for the descriptor the boot loader still reads).
pub const MANIFEST_NAME: &str = "manifest.json";

/// zstd level used for packing. Fixed so packing is reproducible for a given zstd
/// version; verification never depends on it (it re-hashes received bytes).
const ZSTD_LEVEL: i32 = 19;

/// The kind of extension a package carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Plugin,
    Module,
    Widget,
    Content,
}

impl Kind {
    /// The `kind` string as written in `bundle.json`/`manifest.json`.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Plugin => "plugin",
            Kind::Module => "module",
            Kind::Widget => "widget",
            Kind::Content => "content",
        }
    }
}

/// A required module (mirrors `quicktys-core`'s `ModuleRequirement`): `{ module,
/// version }` where `version` is a semver requirement (`*`, `^1`, `>=1.2, <2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRequirement {
    pub module: String,
    #[serde(default = "any_version")]
    pub version: String,
}

fn any_version() -> String {
    "*".to_string()
}

/// The commercial model of an object (marketplace §11.1 / drm.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseModel {
    #[default]
    Free,
    Paid,
    Subscription,
}

/// Licensing metadata. Defaults to free so unpriced packages need no `license` key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct License {
    #[serde(default)]
    pub model: LicenseModel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
}

/// Payload-protection descriptor (drm.md Annexe B). Absent = tier 0 (clear).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Protection {
    /// `"encrypted"` for tier 1; other tiers reserved.
    pub tier: String,
    /// AEAD suite, e.g. `"xchacha20-poly1305"` (default) or `"aes-256-gcm"`.
    pub cipher: String,
    /// Identifier of the content key (per object+version).
    pub content_key_id: String,
    /// The files that are shipped encrypted (`<name>` → `<name>.enc`).
    #[serde(default)]
    pub enc: Vec<String>,
}

/// The package manifest — a superset of the legacy `bundle.json`. Unknown fields
/// the boot loader ignores; the marketplace/DRM layers read the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub kind: Kind,
    /// Semver version of the package.
    pub version: String,
    /// The component file (absent for widgets/content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm: Option<String>,
    /// Modules this package requires (plugins).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<ModuleRequirement>,
    /// System capabilities requested (modules only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// Minimum host version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_host: Option<String>,
    /// Publisher id (certified) or pseudonym.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Commercial model.
    #[serde(default)]
    pub license: License,
    /// Per-file sha256 of the **plaintext** payload (integrity, incl. after decrypt).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hashes: BTreeMap<String, String>,
    /// Payload protection (tier 1 = encrypted). Absent = clear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<Protection>,
    /// Forensic watermarking (M6.10): weave a per-buyer opaque id into the
    /// package's text assets at install, to trace leaks.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub watermark: bool,
}

impl Manifest {
    /// The minimal descriptor the boot loader (`main.rs::BundleManifest`) reads:
    /// `{ id, kind, wasm, certified }`. `certified` is a load-time verdict, not a
    /// manifest field — the installer computes it (M6.5) and writes it here.
    pub fn to_bundle_json(&self, certified: bool) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "kind": self.kind.as_str(),
            "wasm": self.wasm.clone().unwrap_or_default(),
            "certified": certified,
        })
    }

    /// `"<kind>/<id>"` — the key used by the lockfile / ref-counting (M6.2).
    pub fn slug(&self) -> String {
        format!("{}/{}", self.kind.as_str(), self.id)
    }
}

/// Errors from packing/unpacking/reading a package.
#[derive(Debug, thiserror::Error)]
pub enum PkgError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("`{MANIFEST_NAME}` missing from package")]
    MissingManifest,
    #[error("unsafe path in archive: {0}")]
    UnsafePath(String),
}

/// Pack a bundle directory into a `.qtpkg` (deterministic tar + zstd). The dir must
/// contain a `manifest.json`. Returns the archive bytes.
pub fn pack_dir(dir: &Path) -> Result<Vec<u8>, PkgError> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    collect(dir, dir, &mut files)?;
    // Deterministic order: sort by the raw relative-path bytes.
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    let mut tar_buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_buf);
        for (rel, data) in &files {
            let mut header = tar::Header::new_gnu();
            header.set_path(rel)?;
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder.append(&header, data.as_slice())?;
        }
        builder.finish()?;
    }
    Ok(zstd::encode_all(tar_buf.as_slice(), ZSTD_LEVEL)?)
}

/// Unpack a `.qtpkg` into `dest` (created if needed). Rejects path traversal.
pub fn unpack(archive: &[u8], dest: &Path) -> Result<(), PkgError> {
    let decompressed = zstd::decode_all(archive)?;
    let mut ar = tar::Archive::new(decompressed.as_slice());
    for entry in ar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        ensure_safe(&path)?;
        let out = dest.join(&path);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}

/// Read only `manifest.json` from a package, without unpacking the rest.
pub fn read_manifest(archive: &[u8]) -> Result<Manifest, PkgError> {
    let decompressed = zstd::decode_all(archive)?;
    let mut ar = tar::Archive::new(decompressed.as_slice());
    for entry in ar.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_ref() == Path::new(MANIFEST_NAME) {
            let mut s = String::new();
            entry.read_to_string(&mut s)?;
            return Ok(serde_json::from_str(&s)?);
        }
    }
    Err(PkgError::MissingManifest)
}

/// `sha256:<hex>` of an archive — its content-addressed identity.
pub fn content_hash(archive: &[u8]) -> String {
    hash_bytes(archive)
}

/// `sha256:<hex>` of arbitrary bytes (also used for per-file plaintext `hashes`).
pub fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{:x}", h.finalize())
}

/// Recursively collect files under `dir` as `(relative-path, bytes)`, forward-slash
/// separated. Directories are implied by their files (recreated on unpack).
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), PkgError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let path = entry.path();
        if ft.is_dir() {
            collect(root, &path, out)?;
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(root)
                .expect("child of root")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, std::fs::read(&path)?));
        }
    }
    Ok(())
}

/// Reject absolute paths and `..` components (tar path-traversal defense).
fn ensure_safe(path: &Path) -> Result<(), PkgError> {
    for c in path.components() {
        if matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err(PkgError::UnsafePath(path.display().to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A throwaway dir under the system temp, cleaned on drop. No external deps.
    /// The suffix is a **process-global** atomic counter (not thread-local) so
    /// concurrent tests never collide on the same path.
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let pid = std::process::id();
            let p = std::env::temp_dir().join(format!("qtpkg-{tag}-{pid}-{n}"));
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> TmpDir {
        let d = TmpDir::new("src");
        d.write(
            MANIFEST_NAME,
            br#"{ "id":"obs", "kind":"module", "version":"1.4.0", "wasm":"obs.wasm",
                 "capabilities":["net"], "requires":[{"module":"http","version":"^1"}],
                 "license":{"model":"paid","sku":"obs-pro"} }"#,
        );
        d.write("obs.wasm", b"\0asm\x01\0\0\0fake");
        d.write("lang/en.json", br#"{"name":"OBS"}"#);
        d
    }

    #[test]
    fn read_manifest_parses_superset() {
        let d = sample();
        let archive = pack_dir(&d.0).unwrap();
        let m = read_manifest(&archive).unwrap();
        assert_eq!(m.id, "obs");
        assert_eq!(m.kind, Kind::Module);
        assert_eq!(m.wasm.as_deref(), Some("obs.wasm"));
        assert_eq!(m.capabilities, ["net"]);
        assert_eq!(m.requires[0].module, "http");
        assert_eq!(m.requires[0].version, "^1");
        assert_eq!(m.license.model, LicenseModel::Paid);
        assert_eq!(m.slug(), "module/obs");
    }

    #[test]
    fn pack_is_deterministic() {
        let d = sample();
        assert_eq!(pack_dir(&d.0).unwrap(), pack_dir(&d.0).unwrap());
    }

    #[test]
    fn content_hash_is_stable_and_prefixed() {
        let d = sample();
        let h = content_hash(&pack_dir(&d.0).unwrap());
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
        assert_eq!(h, content_hash(&pack_dir(&d.0).unwrap()));
    }

    #[test]
    fn round_trip_restores_files() {
        let d = sample();
        let archive = pack_dir(&d.0).unwrap();
        let out = TmpDir::new("out");
        unpack(&archive, &out.0).unwrap();
        assert_eq!(
            std::fs::read(out.0.join("obs.wasm")).unwrap(),
            b"\0asm\x01\0\0\0fake"
        );
        assert_eq!(
            std::fs::read_to_string(out.0.join("lang/en.json")).unwrap(),
            r#"{"name":"OBS"}"#
        );
        // The unpacked manifest re-reads identically.
        let a = read_manifest(&archive).unwrap();
        let b: Manifest =
            serde_json::from_slice(&std::fs::read(out.0.join(MANIFEST_NAME)).unwrap()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn to_bundle_json_is_boot_compatible() {
        let d = sample();
        let m = read_manifest(&pack_dir(&d.0).unwrap()).unwrap();
        let bj = m.to_bundle_json(true);
        assert_eq!(bj["id"], "obs");
        assert_eq!(bj["kind"], "module");
        assert_eq!(bj["wasm"], "obs.wasm");
        assert_eq!(bj["certified"], true);
    }

    #[test]
    fn missing_manifest_errors() {
        let d = TmpDir::new("nomani");
        d.write("obs.wasm", b"x");
        let archive = pack_dir(&d.0).unwrap();
        assert!(matches!(
            read_manifest(&archive),
            Err(PkgError::MissingManifest)
        ));
    }

    #[test]
    fn defaults_fill_optional_fields() {
        let m: Manifest =
            serde_json::from_str(r#"{"id":"x","kind":"widget","version":"0.1.0"}"#).unwrap();
        assert_eq!(m.license.model, LicenseModel::Free);
        assert!(m.wasm.is_none());
        assert!(m.requires.is_empty());
        assert!(m.protection.is_none());
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        assert!(matches!(
            ensure_safe(Path::new("../evil")),
            Err(PkgError::UnsafePath(_))
        ));
        assert!(matches!(
            ensure_safe(Path::new("/etc/passwd")),
            Err(PkgError::UnsafePath(_))
        ));
        assert!(ensure_safe(Path::new("lang/en.json")).is_ok());
    }
}

