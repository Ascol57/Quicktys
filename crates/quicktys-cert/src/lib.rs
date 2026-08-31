//! `quicktys-cert` — package provenance: the CA → publisher → release signature
//! chain, and the verification that produces the **`certified`** verdict
//! (marketplace M6.5, drm.md §2). Built on `quicktys-crypto`'s hybrid PQ
//! signatures.
//!
//! Two independent layers (drm.md §4): **provenance** (this crate — *who*
//! published, tamper-proof) and **confidentiality** (payload encryption, M6.9).
//! Provenance is verifiable without any content key.
//!
//! ```text
//! CA root key ──signs──► PublisherCert{publisher_key, caps_ceiling, not_after}
//!                              │
//!            publisher_key ──signs──► Release{id,version,kind,caps,manifest_hash}
//! ```
//!
//! Verification: `ca_sig` valid under a **pinned** CA key → cert unexpired &
//! publisher not revoked → `pub_sig` valid under `publisher_key` → hashes/identity
//! match the package → `caps ⊆ caps_ceiling` → **Certified**. A missing signature
//! is **Unsigned** (fine for sandboxed plugins; a module stays uncertified). A
//! present-but-broken signature is **Rejected** (tampering → refuse the install).

use quicktys_crypto::{Signature, SUITE_ID};
use quicktys_pkg::Manifest;
use serde::{Deserialize, Serialize};

// Re-exported so callers can pin CA keys without depending on `quicktys-crypto`.
pub use quicktys_crypto::VerifyingKey;

/// A publisher certificate: the CA vouches for a publisher key and caps ceiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherCert {
    pub publisher_id: String,
    /// Hex of the publisher's hybrid verifying key.
    pub publisher_key: String,
    /// The maximum system capabilities this publisher may request.
    pub caps_ceiling: Vec<String>,
    /// Expiry as a Unix timestamp (seconds). Kept numeric so verification needs
    /// no date-formatting dependency — the caller passes `now` the same way.
    pub not_after: u64,
    pub suite: String,
}

/// A signed release statement: the publisher commits to one exact package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub version: String,
    pub kind: String,
    pub capabilities: Vec<String>,
    /// `sha256:` of the package's `manifest.json` bytes. This is what the
    /// signature binds (the full-archive `content_hash` can't be signed — the
    /// signature lives *inside* the archive). The manifest in turn commits to the
    /// payload via its per-file `hashes`, and it re-verifies at load without the
    /// archive. Download integrity of the archive is a separate, registry-level
    /// `content_hash` check (drm.md Annexe B).
    pub manifest_hash: String,
    /// Optional release expiry (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<u64>,
    pub suite: String,
}

/// `signature.json`: the detached signature block shipped (in clear) in a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureFile {
    pub suite: String,
    pub release: Release,
    /// Hex of the publisher's signature over `release`.
    pub pub_sig: String,
    pub cert: PublisherCert,
    /// Hex of the CA's signature over `cert`.
    pub ca_sig: String,
}

/// A signed-content revocation list (the signature check is a follow-up; for now
/// the host supplies it as trusted data alongside the pinned keys).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Crl {
    /// Revoked by signed `manifest_hash` (the release identity).
    #[serde(default)]
    pub revoked_manifest_hashes: Vec<String>,
    #[serde(default)]
    pub revoked_publishers: Vec<String>,
}

/// The host's pinned trust anchors: the CA verifying keys, the registry keys (that
/// sign a registry's `index.json`/`crl.json`, M6.6), and the CRL. **Empty by
/// default** — with no pinned CA, nothing verifies as certified (safe default).
#[derive(Default)]
pub struct TrustStore {
    pub ca_keys: Vec<VerifyingKey>,
    pub registry_keys: Vec<VerifyingKey>,
    /// Keys that sign entitlements / license keys (the store service, M6.8).
    pub store_keys: Vec<VerifyingKey>,
    pub crl: Crl,
}

/// The outcome of verifying a package's provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Chain valid: the package is certified (system caps may be granted).
    Certified {
        publisher: String,
        capabilities: Vec<String>,
    },
    /// No `signature.json` — fine for plugins/widgets; a module stays uncertified.
    Unsigned,
    /// A signature is present but invalid — refuse the install (tampering).
    Rejected(String),
}

impl Verdict {
    pub fn is_certified(&self) -> bool {
        matches!(self, Verdict::Certified { .. })
    }
}

/// Errors decoding a signature file.
#[derive(Debug, thiserror::Error)]
pub enum CertError {
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bad license encoding")]
    BadLicense,
}

/// Parse a `signature.json`.
pub fn parse_signature(bytes: &[u8]) -> Result<SignatureFile, CertError> {
    Ok(serde_json::from_slice(bytes)?)
}

/// Verify a package's provenance chain and produce a [`Verdict`].
///
/// `expected_manifest`, when `Some`, is checked against the release's signed
/// `manifest_hash` — both the install and boot paths pass the sha256 of the
/// on-disk `manifest.json`. `now` is the current time (Unix seconds), supplied by
/// the caller so this stays clock-free.
pub fn verify(
    sig: &SignatureFile,
    manifest: &Manifest,
    expected_manifest: Option<&str>,
    trust: &TrustStore,
    now: u64,
) -> Verdict {
    match verify_inner(sig, manifest, expected_manifest, trust, now) {
        Ok(()) => Verdict::Certified {
            publisher: sig.cert.publisher_id.clone(),
            capabilities: sig.release.capabilities.clone(),
        },
        Err(reason) => Verdict::Rejected(reason),
    }
}

fn verify_inner(
    sig: &SignatureFile,
    manifest: &Manifest,
    expected_manifest: Option<&str>,
    trust: &TrustStore,
    now: u64,
) -> Result<(), String> {
    if sig.suite != SUITE_ID || sig.cert.suite != SUITE_ID || sig.release.suite != SUITE_ID {
        return Err(format!("unknown crypto suite (want {SUITE_ID})"));
    }
    if trust.ca_keys.is_empty() {
        return Err("no pinned CA key".to_string());
    }

    // 1. The CA vouches for the publisher cert.
    let ca_sig = decode_sig(&sig.ca_sig)?;
    let cert_bytes = canonical(&sig.cert)?;
    if !trust
        .ca_keys
        .iter()
        .any(|k| k.verify(&cert_bytes, &ca_sig).is_ok())
    {
        return Err("publisher certificate not signed by a pinned CA".to_string());
    }
    if now >= sig.cert.not_after {
        return Err("publisher certificate expired".to_string());
    }
    if trust
        .crl
        .revoked_publishers
        .iter()
        .any(|p| p == &sig.cert.publisher_id)
    {
        return Err("publisher revoked".to_string());
    }

    // 2. The publisher signs the release.
    let publisher_key = VerifyingKey::from_bytes(&from_hex(&sig.cert.publisher_key)?)
        .map_err(|_| "bad publisher key encoding".to_string())?;
    let pub_sig = decode_sig(&sig.pub_sig)?;
    let release_bytes = canonical(&sig.release)?;
    publisher_key
        .verify(&release_bytes, &pub_sig)
        .map_err(|_| "release not signed by the certified publisher".to_string())?;

    // 3. The release must match this exact package.
    let r = &sig.release;
    if r.id != manifest.id || r.kind != manifest.kind.as_str() || r.version != manifest.version {
        return Err("release identity does not match the manifest".to_string());
    }
    if let Some(expected) = expected_manifest {
        if r.manifest_hash != expected {
            return Err("release manifest hash mismatch".to_string());
        }
    }
    if trust
        .crl
        .revoked_manifest_hashes
        .iter()
        .any(|h| h == &r.manifest_hash)
    {
        return Err("release revoked".to_string());
    }
    if let Some(exp) = &r.not_after {
        if now >= *exp {
            return Err("release expired".to_string());
        }
    }

    // 4. Capabilities: consistent with the manifest, and within the ceiling.
    if !same_set(&r.capabilities, &manifest.capabilities) {
        return Err("release capabilities differ from the manifest".to_string());
    }
    let ceiling = &sig.cert.caps_ceiling;
    if let Some(over) = r.capabilities.iter().find(|c| !ceiling.contains(c)) {
        return Err(format!("capability `{over}` exceeds the publisher ceiling"));
    }
    Ok(())
}

/// Deterministic bytes to sign/verify a struct (serde emits fields in declaration
/// order; no maps are used, so this is stable).
fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| format!("canonicalization failed: {e}"))
}

fn decode_sig(hex: &str) -> Result<Signature, String> {
    Signature::from_bytes(&from_hex(hex)?).map_err(|_| "bad signature encoding".to_string())
}

fn same_set(a: &[String], b: &[String]) -> bool {
    let mut a: Vec<&String> = a.iter().collect();
    let mut b: Vec<&String> = b.iter().collect();
    a.sort();
    b.sort();
    a == b
}

/// Lowercase hex encode.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Lowercase/uppercase hex decode.
pub fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("odd hex length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "bad hex digit".to_string()))
        .collect()
}

// ===========================================================================
// Licensing / entitlements (M6.8)
// ===========================================================================

/// A store-signed usage grant for a package (or a wildcard). A `free` package
/// needs none; `paid`/`subscription` packages require a covering entitlement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entitlement {
    pub sku: String,
    /// Package kind covered (`plugin`/`module`/`widget`), or `*`.
    pub kind: String,
    /// Package id covered, or `*`.
    pub id: String,
    /// `paid` | `subscription`.
    pub model: String,
    /// Expiry (Unix seconds) — subscriptions set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Optional secret (hex) that derives the payload content key of an encrypted
    /// package (M6.9, offline-license path). Kept out of any public index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_material: Option<String>,
}

impl Entitlement {
    /// Whether this entitlement covers `(kind, id)`.
    pub fn covers(&self, kind: &str, id: &str) -> bool {
        (self.kind == "*" || self.kind == kind) && (self.id == "*" || self.id == id)
    }
}

/// An entitlement plus the store's signature — the token the user holds and pastes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    pub entitlement: Entitlement,
    /// Hex of the store's hybrid signature over the canonical entitlement.
    pub sig: String,
}

const LICENSE_PREFIX: &str = "QTLIC-";

/// Encode a license key as a pasteable `QTLIC-<hex(json)>` string.
pub fn encode_license(key: &LicenseKey) -> String {
    let json = serde_json::to_vec(key).unwrap_or_default();
    format!("{LICENSE_PREFIX}{}", to_hex(&json))
}

/// Parse a `QTLIC-…` (or bare hex) license string.
pub fn parse_license(s: &str) -> Result<LicenseKey, CertError> {
    let s = s.trim();
    let hex = s.strip_prefix(LICENSE_PREFIX).unwrap_or(s);
    let bytes = from_hex(hex).map_err(|_| CertError::BadLicense)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Sign an entitlement with a store signing key, producing a [`LicenseKey`].
pub fn issue_license(
    store: &quicktys_crypto::SigningKey,
    entitlement: Entitlement,
) -> Result<LicenseKey, String> {
    let bytes = canonical(&entitlement)?;
    let sig = to_hex(&store.sign(&bytes).map_err(|e| format!("{e:?}"))?.to_bytes());
    Ok(LicenseKey { entitlement, sig })
}

/// Verify a license against pinned store keys; returns the entitlement if valid
/// and unexpired.
pub fn verify_license(
    key: &LicenseKey,
    store_keys: &[VerifyingKey],
    now: u64,
) -> Result<Entitlement, String> {
    if store_keys.is_empty() {
        return Err("no pinned store key".to_string());
    }
    let sig = Signature::from_bytes(&from_hex(&key.sig)?).map_err(|_| "bad signature".to_string())?;
    let bytes = canonical(&key.entitlement)?;
    if !store_keys.iter().any(|k| k.verify(&bytes, &sig).is_ok()) {
        return Err("license not signed by a pinned store key".to_string());
    }
    if let Some(exp) = key.entitlement.not_after {
        if now >= exp {
            return Err("license expired".to_string());
        }
    }
    Ok(key.entitlement.clone())
}

#[cfg(test)]
mod license_tests {
    use super::*;
    use quicktys_crypto::SigningKey;

    fn ent() -> Entitlement {
        Entitlement {
            sku: "obs-pro".into(),
            kind: "plugin".into(),
            id: "obs".into(),
            model: "paid".into(),
            not_after: None,
            account: None,
            key_material: None,
        }
    }

    #[test]
    fn license_round_trips_and_verifies() {
        let store = SigningKey::generate().unwrap();
        let lic = issue_license(&store, ent()).unwrap();
        let encoded = encode_license(&lic);
        let parsed = parse_license(&encoded).unwrap();
        let e = verify_license(&parsed, &[store.verifying_key()], 1_754_000_000).unwrap();
        assert_eq!(e.sku, "obs-pro");
        assert!(e.covers("plugin", "obs"));
        assert!(!e.covers("plugin", "discord"));
    }

    #[test]
    fn wrong_store_key_and_expiry_rejected() {
        let store = SigningKey::generate().unwrap();
        let attacker = SigningKey::generate().unwrap();
        let lic = issue_license(&store, ent()).unwrap();
        assert!(verify_license(&lic, &[attacker.verifying_key()], 1_754_000_000).is_err());

        let mut e = ent();
        e.not_after = Some(1_000_000_000);
        let expired = issue_license(&store, e).unwrap();
        assert!(verify_license(&expired, &[store.verifying_key()], 1_754_000_000).is_err());
    }

    #[test]
    fn wildcard_covers_all() {
        let e = Entitlement { kind: "*".into(), id: "*".into(), ..ent() };
        assert!(e.covers("module", "anything"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quicktys_crypto::SigningKey;

    const NOW: u64 = 1_754_000_000;
    const FUTURE: u64 = 2_000_000_000;
    const PAST: u64 = 1_000_000_000;

    fn manifest(caps: &[&str]) -> Manifest {
        let caps_json = serde_json::to_string(caps).unwrap();
        serde_json::from_str(&format!(
            r#"{{"id":"obs","kind":"module","version":"1.4.0","wasm":"obs.wasm","capabilities":{caps_json}}}"#
        ))
        .unwrap()
    }

    /// Build a full signature file for `manifest`, signed by `ca` → `publisher`.
    fn sign_package(
        ca: &SigningKey,
        publisher: &SigningKey,
        m: &Manifest,
        caps_ceiling: &[&str],
        cert_not_after: u64,
    ) -> SignatureFile {
        let cert = PublisherCert {
            publisher_id: "acme".to_string(),
            publisher_key: to_hex(&publisher.verifying_key().to_bytes()),
            caps_ceiling: caps_ceiling.iter().map(|s| s.to_string()).collect(),
            not_after: cert_not_after,
            suite: SUITE_ID.to_string(),
        };
        let ca_sig = to_hex(&ca.sign(&canonical(&cert).unwrap()).unwrap().to_bytes());
        let release = Release {
            id: m.id.clone(),
            version: m.version.clone(),
            kind: m.kind.as_str().to_string(),
            capabilities: m.capabilities.clone(),
            manifest_hash: "sha256:bb".to_string(),
            not_after: None,
            suite: SUITE_ID.to_string(),
        };
        let pub_sig = to_hex(&publisher.sign(&canonical(&release).unwrap()).unwrap().to_bytes());
        SignatureFile {
            suite: SUITE_ID.to_string(),
            release,
            pub_sig,
            cert,
            ca_sig,
        }
    }

    fn trust(ca: &SigningKey) -> TrustStore {
        TrustStore {
            ca_keys: vec![ca.verifying_key()],
            registry_keys: Vec::new(),
            store_keys: Vec::new(),
            crl: Crl::default(),
        }
    }

    #[test]
    fn valid_chain_certifies() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let sig = sign_package(&ca, &pubk, &m, &["net", "audio"], FUTURE);
        let v = verify(&sig, &m, Some("sha256:bb"), &trust(&ca), NOW);
        assert_eq!(
            v,
            Verdict::Certified {
                publisher: "acme".to_string(),
                capabilities: vec!["net".to_string()],
            }
        );
    }

    #[test]
    fn no_pinned_ca_never_certifies() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let sig = sign_package(&ca, &pubk, &m, &["net"], FUTURE);
        let empty = TrustStore::default();
        assert!(matches!(
            verify(&sig, &m, None, &empty, NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn wrong_ca_rejected() {
        let ca = SigningKey::generate().unwrap();
        let attacker_ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let sig = sign_package(&ca, &pubk, &m, &["net"], FUTURE);
        assert!(matches!(
            verify(&sig, &m, None, &trust(&attacker_ca), NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn tampered_release_rejected() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let mut sig = sign_package(&ca, &pubk, &m, &["net"], FUTURE);
        // Attacker escalates the signed capabilities after signing.
        sig.release.capabilities = vec!["net".to_string(), "input".to_string()];
        assert!(matches!(
            verify(&sig, &m, None, &trust(&ca), NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn caps_exceeding_ceiling_rejected() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["input"]);
        // Publisher only allowed `net`, but the package wants `input`.
        let sig = sign_package(&ca, &pubk, &m, &["net"], FUTURE);
        assert!(matches!(
            verify(&sig, &m, None, &trust(&ca), NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn expired_cert_rejected() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let sig = sign_package(&ca, &pubk, &m, &["net"], PAST);
        assert!(matches!(
            verify(&sig, &m, None, &trust(&ca), NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn revoked_publisher_and_hash_rejected() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let sig = sign_package(&ca, &pubk, &m, &["net"], FUTURE);

        let mut t = trust(&ca);
        t.crl.revoked_publishers.push("acme".to_string());
        assert!(matches!(
            verify(&sig, &m, None, &t, NOW),
            Verdict::Rejected(_)
        ));

        let mut t = trust(&ca);
        t.crl.revoked_manifest_hashes.push("sha256:bb".to_string());
        assert!(matches!(
            verify(&sig, &m, None, &t, NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn manifest_hash_mismatch_rejected() {
        let ca = SigningKey::generate().unwrap();
        let pubk = SigningKey::generate().unwrap();
        let m = manifest(&["net"]);
        let sig = sign_package(&ca, &pubk, &m, &["net"], FUTURE);
        // Signature is for manifest "sha256:bb", but the on-disk manifest differs.
        assert!(matches!(
            verify(&sig, &m, Some("sha256:ffff"), &trust(&ca), NOW),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn hex_round_trips() {
        let bytes = [0u8, 1, 15, 16, 255, 128];
        assert_eq!(from_hex(&to_hex(&bytes)).unwrap(), bytes);
        assert!(from_hex("xyz").is_err());
    }
}
