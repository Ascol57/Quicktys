//! Held entitlements / license keys (marketplace M6.8).
//!
//! A `free` package needs none. A `paid`/`subscription` package requires a
//! store-signed [`LicenseKey`](quicktys_cert::LicenseKey) covering it — verified
//! **offline** against the host's pinned store keys. The store persists the keys
//! the user has entered (`<data>/entitlements.json`).

use std::path::Path;

use quicktys_cert::{verify_license, Entitlement, LicenseKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::InstallError;

const ENTITLEMENTS_FILE: &str = "entitlements.json";

/// The license keys the user holds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntitlementStore {
    #[serde(default)]
    keys: Vec<LicenseKey>,
}

impl EntitlementStore {
    pub fn load(data_root: &Path) -> Result<EntitlementStore, InstallError> {
        match std::fs::read(data_root.join(ENTITLEMENTS_FILE)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EntitlementStore::default()),
            Err(e) => Err(InstallError::Io(e)),
        }
    }

    pub fn save(&self, data_root: &Path) -> Result<(), InstallError> {
        let path = data_root.join(ENTITLEMENTS_FILE);
        let tmp = data_root.join(format!("{ENTITLEMENTS_FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Add a license key (dedup by signature).
    pub fn add(&mut self, key: LicenseKey) {
        if !self.keys.iter().any(|k| k.sig == key.sig) {
            self.keys.push(key);
        }
    }

    /// The valid, unexpired entitlements (against the pinned store keys).
    pub fn valid(&self, store_keys: &[VerifyingKey], now: u64) -> Vec<Entitlement> {
        self.keys
            .iter()
            .filter_map(|k| verify_license(k, store_keys, now).ok())
            .collect()
    }

    /// Whether a valid entitlement covers `(kind, id)`.
    pub fn is_entitled(&self, kind: &str, id: &str, store_keys: &[VerifyingKey], now: u64) -> bool {
        self.valid(store_keys, now).iter().any(|e| e.covers(kind, id))
    }

    /// A per-buyer seed for watermarking (M6.10): the covering entitlement's
    /// account, or its sku. `None` if not entitled.
    pub fn seed_for(&self, kind: &str, id: &str, store_keys: &[VerifyingKey], now: u64) -> Option<String> {
        self.valid(store_keys, now)
            .into_iter()
            .find(|e| e.covers(kind, id))
            .map(|e| e.account.unwrap_or(e.sku))
    }

    /// The `key_material` of a covering entitlement (for encrypted payloads, M6.9).
    pub fn key_material_for(
        &self,
        kind: &str,
        id: &str,
        store_keys: &[VerifyingKey],
        now: u64,
    ) -> Option<String> {
        self.valid(store_keys, now)
            .into_iter()
            .find(|e| e.covers(kind, id))
            .and_then(|e| e.key_material)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quicktys_cert::{issue_license, Entitlement};
    use quicktys_crypto::SigningKey;

    fn ent(kind: &str, id: &str) -> Entitlement {
        Entitlement {
            sku: "x".into(),
            kind: kind.into(),
            id: id.into(),
            model: "paid".into(),
            not_after: None,
            account: None,
            key_material: None,
        }
    }

    #[test]
    fn entitlement_gate() {
        let store = SigningKey::generate().unwrap();
        let keys = [store.verifying_key()];
        let mut es = EntitlementStore::default();
        es.add(issue_license(&store, ent("plugin", "obs")).unwrap());

        assert!(es.is_entitled("plugin", "obs", &keys, 1_754_000_000));
        assert!(!es.is_entitled("plugin", "discord", &keys, 1_754_000_000));
        // No pinned store key → nothing is entitled.
        assert!(!es.is_entitled("plugin", "obs", &[], 1_754_000_000));
    }
}
