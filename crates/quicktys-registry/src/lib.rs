//! `quicktys-registry` — the static marketplace registry client (M6.6).
//!
//! A registry is **static-first** (marketplace.md §4): a signed `index.json`
//! catalog + a `crl.json` + content-addressed `.qtpkg` blobs, servable from any
//! bucket/CDN. This crate fetches and caches the index, searches it, **resolves
//! dependencies** (a plugin pulls the certified modules it requires), and
//! downloads + integrity-checks blobs — leaving the actual materialization to
//! `quicktys-install` (which verifies each package's own signature, M6.5).
//!
//! Transport-agnostic: HTTP is injected as a [`Fetcher`], so the engine is fully
//! testable offline and `quicktys-registry` never depends on the WASM host.

use std::path::Path;

use quicktys_crypto::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// A dependency requirement in an index entry (`{module, version}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Req {
    pub module: String,
    #[serde(default = "star")]
    pub version: String,
}

fn star() -> String {
    "*".to_string()
}

/// One catalog entry (one published version of one package).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub kind: String,
    pub version: String,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub certified: bool,
    /// `sha256:` of the `.qtpkg` archive (download integrity).
    pub content_hash: String,
    #[serde(default)]
    pub size: u64,
    /// Blob location, relative to the registry base URL.
    pub url: String,
    #[serde(default)]
    pub requires: Vec<Req>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub min_host: Option<String>,
    /// Marketing metadata (localized later).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub license: serde_json::Value,
}

impl IndexEntry {
    pub fn slug(&self) -> String {
        format!("{}/{}", self.kind, self.id)
    }
}

/// The catalog (`index.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub schema: u32,
    #[serde(default)]
    pub packages: Vec<IndexEntry>,
}

/// One package to install, resolved from the index (with its dependency flag).
#[derive(Debug, Clone)]
pub struct PlanItem {
    pub entry: IndexEntry,
    /// `true` for the user's target; `false` for a pulled dependency.
    pub explicit: bool,
}

/// A downloaded, integrity-checked package ready for `quicktys-install`.
#[derive(Debug, Clone)]
pub struct FetchedPackage {
    pub archive: Vec<u8>,
    pub content_hash: String,
    pub explicit: bool,
    pub slug: String,
}

/// Injected HTTP transport (the app backs it with the host's client).
pub trait Fetcher: Send + Sync {
    /// GET `url`, returning the body, or `Err` for network/HTTP errors.
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
    /// GET that tolerates a missing resource (returns `Ok(None)` on 404).
    fn get_optional(&self, url: &str) -> Result<Option<Vec<u8>>, String> {
        match self.get(url) {
            Ok(b) => Ok(Some(b)),
            Err(_) => Ok(None),
        }
    }
}

const INDEX_FILE: &str = "index.json";
const INDEX_SIG: &str = "index.sig";
const CRL_FILE: &str = "crl.json";

/// Errors from the registry client.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("network error: {0}")]
    Net(String),
    #[error("index signature invalid")]
    BadIndexSignature,
    #[error("no candidate for `{0}` satisfying `{1}`")]
    Unresolved(String, String),
    #[error("dependency cycle at `{0}`")]
    Cycle(String),
    #[error("downloaded `{slug}` content hash mismatch (want {want}, got {got})")]
    HashMismatch {
        slug: String,
        want: String,
        got: String,
    },
}

/// Fetch a registry's `index.json` (+ optional `crl.json`), verify the index
/// signature against pinned `registry_keys` (if any), and cache both to
/// `cache_dir`. Returns the parsed index.
pub fn refresh(
    base_url: &str,
    fetcher: &dyn Fetcher,
    cache_dir: &Path,
    registry_keys: &[VerifyingKey],
) -> Result<RegistryIndex, RegistryError> {
    let base = base_url.trim_end_matches('/');
    let index_bytes = fetcher
        .get(&format!("{base}/{INDEX_FILE}"))
        .map_err(RegistryError::Net)?;

    // Verify the index signature when the registry key is pinned. A pinned key
    // with a missing/invalid signature is a hard failure.
    if !registry_keys.is_empty() {
        let sig_bytes = fetcher
            .get_optional(&format!("{base}/{INDEX_SIG}"))
            .map_err(RegistryError::Net)?
            .ok_or(RegistryError::BadIndexSignature)?;
        if !index_signature_ok(&index_bytes, &sig_bytes, registry_keys) {
            return Err(RegistryError::BadIndexSignature);
        }
    }

    let index: RegistryIndex = serde_json::from_slice(&index_bytes)?;
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(cache_dir.join(INDEX_FILE), &index_bytes)?;

    // The CRL is optional; cache it verbatim for the install path (M6.5) to read.
    if let Some(crl) = fetcher
        .get_optional(&format!("{base}/{CRL_FILE}"))
        .map_err(RegistryError::Net)?
    {
        std::fs::write(cache_dir.join(CRL_FILE), crl)?;
    }
    Ok(index)
}

/// Load a previously [`refresh`]ed index from cache.
pub fn load_index(cache_dir: &Path) -> Result<RegistryIndex, RegistryError> {
    Ok(serde_json::from_slice(&std::fs::read(
        cache_dir.join(INDEX_FILE),
    )?)?)
}

fn index_signature_ok(index_bytes: &[u8], sig_hex: &[u8], keys: &[VerifyingKey]) -> bool {
    let Ok(sig_hex) = std::str::from_utf8(sig_hex) else {
        return false;
    };
    let Ok(raw) = from_hex(sig_hex.trim()) else {
        return false;
    };
    let Ok(sig) = Signature::from_bytes(&raw) else {
        return false;
    };
    keys.iter().any(|k| k.verify(index_bytes, &sig).is_ok())
}

/// Case-insensitive search over the cached index (id / name / summary), optionally
/// filtered by kind. Returns the highest version of each matching id.
pub fn search<'a>(
    index: &'a RegistryIndex,
    query: &str,
    kind: Option<&str>,
) -> Vec<&'a IndexEntry> {
    let q = query.trim().to_lowercase();
    let mut best: std::collections::BTreeMap<String, &IndexEntry> = Default::default();
    for e in &index.packages {
        if let Some(k) = kind {
            if e.kind != k {
                continue;
            }
        }
        let hay = format!(
            "{} {} {}",
            e.id,
            e.name.as_deref().unwrap_or(""),
            e.summary.as_deref().unwrap_or("")
        )
        .to_lowercase();
        if !q.is_empty() && !hay.contains(&q) {
            continue;
        }
        best.entry(e.slug())
            .and_modify(|cur| {
                if version_gt(&e.version, &cur.version) {
                    *cur = e;
                }
            })
            .or_insert(e);
    }
    best.into_values().collect()
}

/// Resolve the install set for a target (`id`, `kind`): the target plus every
/// required module not already satisfied by `installed` (`(slug, version)`),
/// ordered **modules before plugins** (the order the host loads them).
pub fn plan(
    index: &RegistryIndex,
    id: &str,
    kind: &str,
    installed: &[(String, String)],
) -> Result<Vec<PlanItem>, RegistryError> {
    let mut chosen: Vec<PlanItem> = Vec::new();
    let mut path: Vec<String> = Vec::new();
    resolve_into(index, id, kind, "*", true, installed, &mut chosen, &mut path)?;
    Ok(chosen)
}

/// Post-order resolution: a package is appended **after** its dependencies, so the
/// result is already in load order (deps first). Dedup merges the explicit flag;
/// the `path` detects cycles.
#[allow(clippy::too_many_arguments)]
fn resolve_into(
    index: &RegistryIndex,
    id: &str,
    kind: &str,
    req: &str,
    explicit: bool,
    installed: &[(String, String)],
    chosen: &mut Vec<PlanItem>,
    path: &mut Vec<String>,
) -> Result<(), RegistryError> {
    let entry = best(index, id, kind, req)?;
    let slug = entry.slug();
    if let Some(existing) = chosen.iter_mut().find(|p| p.entry.slug() == slug) {
        existing.explicit |= explicit;
        return Ok(());
    }
    if path.contains(&slug) {
        return Err(RegistryError::Cycle(slug));
    }
    path.push(slug);

    for dep in entry.requires.clone() {
        let dep_slug = format!("module/{}", dep.module);
        if installed
            .iter()
            .any(|(s, v)| s == &dep_slug && satisfies(&dep.version, v))
        {
            continue;
        }
        resolve_into(
            index, &dep.module, "module", &dep.version, false, installed, chosen, path,
        )?;
    }

    path.pop();
    chosen.push(PlanItem { entry, explicit });
    Ok(())
}

/// Download and integrity-check every package in a [`plan`], ready to install.
pub fn fetch_all(
    base_url: &str,
    items: &[PlanItem],
    fetcher: &dyn Fetcher,
) -> Result<Vec<FetchedPackage>, RegistryError> {
    let base = base_url.trim_end_matches('/');
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let url = format!("{base}/{}", item.entry.url.trim_start_matches('/'));
        let archive = fetcher.get(&url).map_err(RegistryError::Net)?;
        let got = quicktys_pkg::content_hash(&archive);
        if got != item.entry.content_hash {
            return Err(RegistryError::HashMismatch {
                slug: item.entry.slug(),
                want: item.entry.content_hash.clone(),
                got,
            });
        }
        out.push(FetchedPackage {
            archive,
            content_hash: got,
            explicit: item.explicit,
            slug: item.entry.slug(),
        });
    }
    Ok(out)
}

/// Installed packages (`(slug, version)`) that the index offers a newer version
/// of — the highest available above what's installed.
pub fn updates<'a>(
    index: &'a RegistryIndex,
    installed: &[(String, String)],
) -> Vec<&'a IndexEntry> {
    let mut out = Vec::new();
    for (slug, ver) in installed {
        let mut best: Option<&IndexEntry> = None;
        for e in &index.packages {
            if &e.slug() == slug
                && version_gt(&e.version, ver)
                && best.is_none_or(|b| version_gt(&e.version, &b.version))
            {
                best = Some(e);
            }
        }
        if let Some(b) = best {
            out.push(b);
        }
    }
    out
}

/// The highest-version entry for `id`/`kind` satisfying `req`.
fn best(index: &RegistryIndex, id: &str, kind: &str, req: &str) -> Result<IndexEntry, RegistryError> {
    index
        .packages
        .iter()
        .filter(|e| e.id == id && e.kind == kind && satisfies(req, &e.version))
        .max_by(|a, b| cmp_version(&a.version, &b.version))
        .cloned()
        .ok_or_else(|| RegistryError::Unresolved(format!("{kind}/{id}"), req.to_string()))
}

fn parse(v: &str) -> Option<semver::Version> {
    let v = v.trim().trim_start_matches('v');
    semver::Version::parse(v).ok().or_else(|| {
        let mut parts: Vec<&str> = v.split('.').take(3).collect();
        while parts.len() < 3 {
            parts.push("0");
        }
        semver::Version::parse(&parts.join(".")).ok()
    })
}

fn satisfies(req: &str, version: &str) -> bool {
    let req = req.trim();
    if req.is_empty() || req == "*" {
        return true;
    }
    match (semver::VersionReq::parse(req), parse(version)) {
        (Ok(r), Some(v)) => r.matches(&v),
        _ => false,
    }
}

fn cmp_version(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse(a), parse(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

fn version_gt(a: &str, b: &str) -> bool {
    cmp_version(a, b) == std::cmp::Ordering::Greater
}

#[cfg(test)]
fn to_hex_test(bytes: &[u8]) -> String {
    let mut s = String::new();
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Lowercase hex decode (for the index signature).
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("odd hex length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "bad hex".to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// An in-memory registry: url → bytes.
    #[derive(Default)]
    struct MockFetcher {
        files: Mutex<HashMap<String, Vec<u8>>>,
    }
    impl MockFetcher {
        fn put(&self, url: &str, bytes: Vec<u8>) {
            self.files.lock().unwrap().insert(url.to_string(), bytes);
        }
    }
    impl Fetcher for MockFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.files
                .lock()
                .unwrap()
                .get(url)
                .cloned()
                .ok_or_else(|| format!("404 {url}"))
        }
    }

    fn entry(id: &str, kind: &str, version: &str, requires: &[(&str, &str)]) -> IndexEntry {
        IndexEntry {
            id: id.to_string(),
            kind: kind.to_string(),
            version: version.to_string(),
            publisher: None,
            certified: kind == "module",
            content_hash: format!("sha256:{id}{version}"),
            size: 0,
            url: format!("blobs/{id}-{version}.qtpkg"),
            requires: requires
                .iter()
                .map(|(m, v)| Req {
                    module: m.to_string(),
                    version: v.to_string(),
                })
                .collect(),
            capabilities: vec![],
            min_host: None,
            name: Some(format!("The {id}")),
            summary: None,
            license: serde_json::Value::Null,
        }
    }

    fn index(entries: Vec<IndexEntry>) -> RegistryIndex {
        RegistryIndex {
            schema: 1,
            packages: entries,
        }
    }

    #[test]
    fn plan_pulls_required_modules_first() {
        let idx = index(vec![
            entry("obs", "plugin", "2.0.0", &[("obs", "^1")]),
            entry("obs", "module", "1.4.0", &[("http", "^1")]),
            entry("http", "module", "1.2.0", &[]),
        ]);
        let plan = plan(&idx, "obs", "plugin", &[]).unwrap();
        let order: Vec<String> = plan.iter().map(|p| p.entry.slug()).collect();
        // Modules before the plugin; both deps resolved.
        assert_eq!(
            order,
            ["module/http", "module/obs", "plugin/obs"]
        );
        // The target is explicit; pulled modules are not.
        let explicit: HashMap<String, bool> =
            plan.iter().map(|p| (p.entry.slug(), p.explicit)).collect();
        assert!(explicit["plugin/obs"]);
        assert!(!explicit["module/obs"]);
        assert!(!explicit["module/http"]);
    }

    #[test]
    fn plan_skips_already_satisfied_dependency() {
        let idx = index(vec![
            entry("obs", "plugin", "2.0.0", &[("obs", "^1")]),
            entry("obs", "module", "1.4.0", &[]),
        ]);
        // obs module already installed at a compatible version.
        let installed = vec![("module/obs".to_string(), "1.3.0".to_string())];
        let plan = plan(&idx, "obs", "plugin", &installed).unwrap();
        let order: Vec<String> = plan.iter().map(|p| p.entry.slug()).collect();
        assert_eq!(order, ["plugin/obs"]);
    }

    #[test]
    fn plan_picks_highest_compatible_version() {
        let idx = index(vec![
            entry("http", "module", "1.2.0", &[]),
            entry("http", "module", "1.5.0", &[]),
            entry("http", "module", "2.0.0", &[]),
        ]);
        let plan = plan(&idx, "http", "module", &[]).unwrap();
        assert_eq!(plan[0].entry.version, "2.0.0");
    }

    #[test]
    fn plan_unresolvable_dependency_errors() {
        let idx = index(vec![entry("obs", "plugin", "2.0.0", &[("obs", "^1")])]);
        assert!(matches!(
            plan(&idx, "obs", "plugin", &[]),
            Err(RegistryError::Unresolved(_, _))
        ));
    }

    #[test]
    fn fetch_all_verifies_content_hash() {
        // Build a real package and advertise its true hash.
        let src = std::env::temp_dir().join(format!("fmreg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&src);
        std::fs::write(
            src.join("manifest.json"),
            br#"{"id":"http","kind":"module","version":"1.0.0","wasm":"http.wasm"}"#,
        )
        .unwrap();
        std::fs::write(src.join("http.wasm"), b"\0asm").unwrap();
        let archive = quicktys_pkg::pack_dir(&src).unwrap();
        let _ = std::fs::remove_dir_all(&src);
        let hash = quicktys_pkg::content_hash(&archive);

        let mut e = entry("http", "module", "1.0.0", &[]);
        e.content_hash = hash.clone();
        e.url = "blobs/http.qtpkg".to_string();
        let idx = index(vec![e]);

        let fetcher = MockFetcher::default();
        fetcher.put("https://reg.test/blobs/http.qtpkg", archive.clone());
        let plan = plan(&idx, "http", "module", &[]).unwrap();
        let fetched = fetch_all("https://reg.test", &plan, &fetcher).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].content_hash, hash);

        // Corrupt the blob → hash mismatch.
        fetcher.put("https://reg.test/blobs/http.qtpkg", b"tampered".to_vec());
        assert!(matches!(
            fetch_all("https://reg.test", &plan, &fetcher),
            Err(RegistryError::HashMismatch { .. })
        ));
    }

    #[test]
    fn refresh_caches_and_verifies_signature() {
        let idx = index(vec![entry("http", "module", "1.0.0", &[])]);
        let index_bytes = serde_json::to_vec(&idx).unwrap();
        let key = quicktys_crypto::SigningKey::generate().unwrap();
        let sig = key.sign(&index_bytes).unwrap();

        let fetcher = MockFetcher::default();
        fetcher.put("https://reg.test/index.json", index_bytes.clone());
        fetcher.put(
            "https://reg.test/index.sig",
            crate::to_hex_test(&sig.to_bytes()).into_bytes(),
        );

        let cache = std::env::temp_dir().join(format!("fmreg-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);

        // Correct pinned key → verifies + caches.
        let got = refresh("https://reg.test", &fetcher, &cache, &[key.verifying_key()]).unwrap();
        assert_eq!(got.packages.len(), 1);
        assert!(cache.join("index.json").exists());

        // Wrong pinned key → rejected.
        let other = quicktys_crypto::SigningKey::generate().unwrap();
        assert!(matches!(
            refresh("https://reg.test", &fetcher, &cache, &[other.verifying_key()]),
            Err(RegistryError::BadIndexSignature)
        ));
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn search_and_updates() {
        let idx = index(vec![
            entry("obs", "plugin", "1.0.0", &[]),
            entry("obs", "plugin", "1.2.0", &[]),
            entry("discord", "plugin", "1.0.0", &[]),
        ]);
        // Search returns the highest version per id.
        let hits = search(&idx, "obs", None);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].version, "1.2.0");
        // Updates: obs installed at 1.0.0 → 1.2.0 available.
        let ups = updates(&idx, &[("plugin/obs".to_string(), "1.0.0".to_string())]);
        assert_eq!(ups.len(), 1);
        assert_eq!(ups[0].version, "1.2.0");
        // Already latest → no update.
        assert!(updates(&idx, &[("plugin/discord".to_string(), "1.0.0".to_string())]).is_empty());
    }
}
