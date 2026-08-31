//! Per-extension **capability consent** (marketplace M6.7).
//!
//! Certification (M6.5) decides whether a module *may* touch a system cap; consent
//! decides whether the **user** has allowed it. The two compose at every cap
//! dispatch: `granted = certified && declared && consented`.
//!
//! Model (drm.md §7): a module is either *un-reviewed* (no record → **allowed**,
//! a grace so nothing breaks before the user visits the consent screen) or
//! *reviewed* (a record → **strict**: only the caps in `granted`). Revoking a cap
//! takes effect immediately; a package **update that widens** its caps has the new
//! ones denied until re-reviewed ([`needs_review`](ConsentStore::needs_review)).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::InstallError;

const CONSENT_FILE: &str = "consent.json";

/// The caps a user reviewed for one module, and the version reviewed at (so a
/// later widening update can re-prompt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentRecord {
    pub granted: Vec<String>,
    pub version: String,
}

/// Persisted consent decisions, keyed by module id (`<data>/consent.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsentStore {
    #[serde(default)]
    records: BTreeMap<String, ConsentRecord>,
}

impl ConsentStore {
    /// Load from `<data>/consent.json`, or empty if absent.
    pub fn load(data_root: &Path) -> Result<ConsentStore, InstallError> {
        match std::fs::read(data_root.join(CONSENT_FILE)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ConsentStore::default()),
            Err(e) => Err(InstallError::Io(e)),
        }
    }

    /// Persist to `<data>/consent.json` (atomically).
    pub fn save(&self, data_root: &Path) -> Result<(), InstallError> {
        let path = data_root.join(CONSENT_FILE);
        let tmp = data_root.join(format!("{CONSENT_FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Enforcement: may `id` use `cap`? Un-reviewed modules are allowed (grace);
    /// reviewed modules are restricted to their granted set.
    pub fn allows(&self, id: &str, cap: &str) -> bool {
        self.records
            .get(id)
            .is_none_or(|r| r.granted.iter().any(|c| c == cap))
    }

    /// Record the user's decision: `id` is granted exactly `caps` at `version`.
    pub fn grant(&mut self, id: &str, caps: Vec<String>, version: &str) {
        self.records.insert(
            id.to_string(),
            ConsentRecord {
                granted: caps,
                version: version.to_string(),
            },
        );
    }

    /// Revoke one cap. Enrolls at `declared` first if the module was un-reviewed,
    /// so the result is "everything it declared, minus `cap`".
    pub fn revoke(&mut self, id: &str, cap: &str, declared: &[String], version: &str) {
        let rec = self.records.entry(id.to_string()).or_insert_with(|| ConsentRecord {
            granted: declared.to_vec(),
            version: version.to_string(),
        });
        rec.granted.retain(|c| c != cap);
        rec.version = version.to_string();
    }

    /// The declared caps that still need a user decision: everything for a never
    /// reviewed module, or the caps a widening update added that aren't granted.
    pub fn needs_review(&self, id: &str, declared: &[String], version: &str) -> Vec<String> {
        match self.records.get(id) {
            None => declared.to_vec(),
            Some(r) if r.version != version => declared
                .iter()
                .filter(|c| !r.granted.contains(c))
                .cloned()
                .collect(),
            Some(_) => Vec::new(),
        }
    }

    /// Whether the user has reviewed this module at all.
    pub fn is_reviewed(&self, id: &str) -> bool {
        self.records.contains_key(id)
    }

    /// The granted caps of a reviewed module (empty if un-reviewed).
    pub fn granted(&self, id: &str) -> Vec<String> {
        self.records
            .get(id)
            .map(|r| r.granted.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn unreviewed_module_is_allowed_but_flagged_for_review() {
        let s = ConsentStore::default();
        assert!(s.allows("obs", "net")); // grace before review
        assert!(!s.is_reviewed("obs"));
        assert_eq!(s.needs_review("obs", &caps(&["net"]), "1.0.0"), ["net"]);
    }

    #[test]
    fn grant_makes_enforcement_strict() {
        let mut s = ConsentStore::default();
        s.grant("obs", caps(&["net"]), "1.0.0");
        assert!(s.allows("obs", "net"));
        assert!(!s.allows("obs", "input")); // reviewed → only granted caps
        assert!(s.needs_review("obs", &caps(&["net"]), "1.0.0").is_empty());
    }

    #[test]
    fn revoke_denies_immediately() {
        let mut s = ConsentStore::default();
        s.revoke("obs", "net", &caps(&["net", "audio"]), "1.0.0");
        assert!(!s.allows("obs", "net"));
        assert!(s.allows("obs", "audio")); // the rest of what it declared
    }

    #[test]
    fn widening_update_reprompts_and_denies_new_caps() {
        let mut s = ConsentStore::default();
        s.grant("obs", caps(&["net"]), "1.0.0");
        // v2 now also wants `input`.
        assert_eq!(s.needs_review("obs", &caps(&["net", "input"]), "2.0.0"), ["input"]);
        assert!(!s.allows("obs", "input")); // denied until re-reviewed
        // User approves the wider set at v2.
        s.grant("obs", caps(&["net", "input"]), "2.0.0");
        assert!(s.allows("obs", "input"));
        assert!(s.needs_review("obs", &caps(&["net", "input"]), "2.0.0").is_empty());
    }

    #[test]
    fn round_trips_on_disk() {
        let dir = std::env::temp_dir().join(format!("fmconsent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = ConsentStore::default();
        s.grant("obs", caps(&["net"]), "1.0.0");
        s.save(&dir).unwrap();
        let loaded = ConsentStore::load(&dir).unwrap();
        assert!(loaded.allows("obs", "net"));
        assert!(!loaded.allows("obs", "input"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
