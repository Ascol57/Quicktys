//! Encrypted payload (T1, marketplace M6.9): a package's `.wasm` shipped as
//! `<name>.enc` and decrypted at load with a content key delivered by the user's
//! license (drm.md Annexe B, offline path).
//!
//! Envelope: `nonce(24) ‖ XChaCha20-Poly1305(content_key, aad = content_key_id)`.
//! The content key is symmetric, so this is quantum-safe as-is; delivering it via
//! a store-signed license (its `key_material`) is the offline, account-free path.

use crate::InstallError;

const NONCE_LEN: usize = 24;

/// Encrypt a plaintext payload for packaging.
pub fn encrypt_payload(plain: &[u8], content_key: &[u8; 32], content_key_id: &str) -> Vec<u8> {
    let nonce: [u8; NONCE_LEN] = quicktys_crypto::random();
    let ct = quicktys_crypto::aead_seal(content_key, &nonce, content_key_id.as_bytes(), plain)
        .expect("aead seal cannot fail");
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

/// Decrypt an `.enc` payload with the content key (hex) from the entitlement,
/// binding the AAD to `content_key_id`.
pub fn decrypt_payload(
    enc: &[u8],
    content_key_hex: &str,
    content_key_id: &str,
) -> Result<Vec<u8>, InstallError> {
    if enc.len() < NONCE_LEN {
        return Err(InstallError::Payload("truncated payload".to_string()));
    }
    let key_bytes = quicktys_cert::from_hex(content_key_hex).map_err(InstallError::Payload)?;
    let key: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| InstallError::Payload("content key must be 32 bytes".to_string()))?;
    let nonce: [u8; NONCE_LEN] = enc[..NONCE_LEN].try_into().unwrap();
    quicktys_crypto::aead_open(&key, &nonce, content_key_id.as_bytes(), &enc[NONCE_LEN..])
        .map_err(|e| InstallError::Payload(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_wrong_key() {
        let ck: [u8; 32] = quicktys_crypto::random();
        let id = "ck-123";
        let wasm = b"\0asm the secret component bytes";
        let enc = encrypt_payload(wasm, &ck, id);
        let key_hex = quicktys_cert::to_hex(&ck);
        assert_eq!(decrypt_payload(&enc, &key_hex, id).unwrap(), wasm);

        // Wrong key / wrong id → fail.
        let other = quicktys_cert::to_hex(&quicktys_crypto::random::<32>());
        assert!(decrypt_payload(&enc, &other, id).is_err());
        assert!(decrypt_payload(&enc, &key_hex, "other-id").is_err());
    }
}
