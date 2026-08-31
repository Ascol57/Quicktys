//! `quicktys-crypto` — the hybrid post-quantum crypto suite (marketplace M6.4).
//!
//! Suite **`hyb-mlkem1024-mldsa87-v1`** (NIST level 5), the foundation for package
//! signing (M6.5) and payload encryption / DRM activation (M6.9). Everything
//! asymmetric is **hybrid classical + PQ** — secure if *either* half holds
//! (drm.md §2.1):
//!
//! - **Signatures**: Ed25519 **+** ML-DSA-87 (both must verify).
//! - **KEM**: X25519 **+** ML-KEM-1024, combined by HKDF-SHA-384 — the priority,
//!   since confidentiality must resist "harvest now, decrypt later".
//! - **AEAD**: XChaCha20-Poly1305 (already PQ-safe at 256-bit keys).
//!
//! Symmetric parts (AEAD, SHA-2) are quantum-safe as-is (Grover only halves the
//! effective key strength). Every serialized artifact is tagged with
//! [`SUITE_ID`] for crypto-agility.

pub mod watermark;

use ed25519_dalek::{Signer, Verifier};
use fips203::ml_kem_1024;
use fips203::traits::{Decaps, Encaps, KeyGen as KemKeyGen, SerDes as KemSerDes};
use fips204::ml_dsa_87;
use fips204::traits::{SerDes as SigSerDes, Signer as MlSigner, Verifier as MlVerifier};
use hkdf::Hkdf;
use sha2::Sha384;

/// The suite identifier stamped on every signature/seal for crypto-agility.
pub const SUITE_ID: &str = "hyb-mlkem1024-mldsa87-v1";

// Classical component sizes.
const ED25519_PUB: usize = 32;
const ED25519_SIG: usize = 64;
const X25519_LEN: usize = 32;
// PQ component sizes (ML-KEM-1024 / ML-DSA-87).
const MLKEM_EK: usize = ml_kem_1024::EK_LEN;
const MLKEM_DK: usize = ml_kem_1024::DK_LEN;
const MLKEM_CT: usize = ml_kem_1024::CT_LEN;
const MLDSA_PK: usize = ml_dsa_87::PK_LEN;
const MLDSA_SK: usize = ml_dsa_87::SK_LEN;
const MLDSA_SIG: usize = ml_dsa_87::SIG_LEN;

/// AEAD key + nonce sizes (XChaCha20-Poly1305).
const AEAD_KEY: usize = 32;
const AEAD_NONCE: usize = 24;

/// Errors from the crypto suite.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid key/signature/ciphertext encoding")]
    BadEncoding,
    #[error("signature verification failed")]
    BadSignature,
    #[error("AEAD open failed (wrong key/nonce/aad or tampered ciphertext)")]
    AeadOpen,
    #[error("post-quantum backend error: {0}")]
    Backend(String),
}

fn os_random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("OS entropy unavailable");
    b
}

/// Fresh cryptographically-random bytes (e.g. a content key or nonce, M6.9).
pub fn random<const N: usize>() -> [u8; N] {
    os_random::<N>()
}

fn pq(e: &str) -> CryptoError {
    CryptoError::Backend(e.to_string())
}

// ===========================================================================
// Signatures — Ed25519 + ML-DSA-87 (both must verify)
// ===========================================================================

/// A hybrid signing key (private). Serialize with [`SigningKey::to_bytes`] to
/// keep it in the OS keystore; never store it inside `installed/`.
pub struct SigningKey {
    ed: ed25519_dalek::SigningKey,
    ml: ml_dsa_87::PrivateKey,
}

/// A hybrid verifying key (public), pinned in the host to authenticate the CA /
/// store / activation service.
#[derive(Clone)]
pub struct VerifyingKey {
    ed: ed25519_dalek::VerifyingKey,
    ml: ml_dsa_87::PublicKey,
}

/// A hybrid signature: `ed25519 (64) ‖ ml-dsa-87`.
#[derive(Clone)]
pub struct Signature {
    ed: [u8; ED25519_SIG],
    ml: [u8; MLDSA_SIG],
}

impl SigningKey {
    /// Generate a fresh hybrid signing key.
    pub fn generate() -> Result<Self, CryptoError> {
        let ed = ed25519_dalek::SigningKey::from_bytes(&os_random::<32>());
        let (_pk, ml) = ml_dsa_87::try_keygen().map_err(pq)?;
        Ok(SigningKey { ed, ml })
    }

    /// The matching public verifying key.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey {
            ed: self.ed.verifying_key(),
            ml: self.ml.get_public_key(),
        }
    }

    /// Sign a message with both algorithms.
    pub fn sign(&self, msg: &[u8]) -> Result<Signature, CryptoError> {
        let ed = self.ed.sign(msg).to_bytes();
        let ml = self.ml.try_sign(msg, &[]).map_err(pq)?;
        Ok(Signature { ed, ml })
    }

    /// `ed25519 seed (32) ‖ ml-dsa-87 secret` — for the OS keystore.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + MLDSA_SK);
        out.extend_from_slice(&self.ed.to_bytes());
        out.extend_from_slice(&self.ml.clone().into_bytes());
        out
    }

    /// Reconstruct from [`to_bytes`](Self::to_bytes).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != 32 + MLDSA_SK {
            return Err(CryptoError::BadEncoding);
        }
        let ed_seed: [u8; 32] = bytes[..32].try_into().unwrap();
        let ml_bytes: [u8; MLDSA_SK] = bytes[32..].try_into().unwrap();
        Ok(SigningKey {
            ed: ed25519_dalek::SigningKey::from_bytes(&ed_seed),
            ml: ml_dsa_87::PrivateKey::try_from_bytes(ml_bytes).map_err(pq)?,
        })
    }
}

impl VerifyingKey {
    /// Verify a hybrid signature — **both** components must be valid.
    pub fn verify(&self, msg: &[u8], sig: &Signature) -> Result<(), CryptoError> {
        let ed_sig = ed25519_dalek::Signature::from_bytes(&sig.ed);
        self.ed
            .verify(msg, &ed_sig)
            .map_err(|_| CryptoError::BadSignature)?;
        if !self.ml.verify(msg, &sig.ml, &[]) {
            return Err(CryptoError::BadSignature);
        }
        Ok(())
    }

    /// `ed25519 pub (32) ‖ ml-dsa-87 pub`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ED25519_PUB + MLDSA_PK);
        out.extend_from_slice(self.ed.as_bytes());
        out.extend_from_slice(&self.ml.clone().into_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != ED25519_PUB + MLDSA_PK {
            return Err(CryptoError::BadEncoding);
        }
        let ed_bytes: [u8; ED25519_PUB] = bytes[..ED25519_PUB].try_into().unwrap();
        let ml_bytes: [u8; MLDSA_PK] = bytes[ED25519_PUB..].try_into().unwrap();
        Ok(VerifyingKey {
            ed: ed25519_dalek::VerifyingKey::from_bytes(&ed_bytes)
                .map_err(|_| CryptoError::BadEncoding)?,
            ml: ml_dsa_87::PublicKey::try_from_bytes(ml_bytes).map_err(pq)?,
        })
    }
}

impl Signature {
    /// `ed25519 (64) ‖ ml-dsa-87`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ED25519_SIG + MLDSA_SIG);
        out.extend_from_slice(&self.ed);
        out.extend_from_slice(&self.ml);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != ED25519_SIG + MLDSA_SIG {
            return Err(CryptoError::BadEncoding);
        }
        Ok(Signature {
            ed: bytes[..ED25519_SIG].try_into().unwrap(),
            ml: bytes[ED25519_SIG..].try_into().unwrap(),
        })
    }
}

// ===========================================================================
// KEM — X25519 + ML-KEM-1024, HKDF-SHA-384 combiner
// ===========================================================================

/// A hybrid KEM private key (the "device key" in DRM activation). Keep in the OS
/// keystore, never in `installed/`.
pub struct KemPrivateKey {
    x: x25519_dalek::StaticSecret,
    ml: ml_kem_1024::DecapsKey,
}

/// A hybrid KEM public key, sent to the activation service to seal a content key.
#[derive(Clone)]
pub struct KemPublicKey {
    x: x25519_dalek::PublicKey,
    ml: ml_kem_1024::EncapsKey,
}

/// The KEM encapsulation: `x25519 ephemeral pub (32) ‖ ml-kem-1024 ciphertext`.
#[derive(Clone)]
pub struct Encapsulation {
    x_eph: [u8; X25519_LEN],
    ml_ct: [u8; MLKEM_CT],
}

/// A 32-byte shared secret from the hybrid KEM.
pub struct SharedSecret([u8; 32]);

impl SharedSecret {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl KemPrivateKey {
    pub fn generate() -> Result<(KemPrivateKey, KemPublicKey), CryptoError> {
        let x_sk = x25519_dalek::StaticSecret::from(os_random::<32>());
        let x_pk = x25519_dalek::PublicKey::from(&x_sk);
        let (ml_ek, ml_dk) = ml_kem_1024::KG::try_keygen().map_err(pq)?;
        Ok((
            KemPrivateKey { x: x_sk, ml: ml_dk },
            KemPublicKey { x: x_pk, ml: ml_ek },
        ))
    }

    /// `x25519 secret (32) ‖ ml-kem-1024 decaps key`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(X25519_LEN + MLKEM_DK);
        out.extend_from_slice(&self.x.to_bytes());
        out.extend_from_slice(&self.ml.clone().into_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != X25519_LEN + MLKEM_DK {
            return Err(CryptoError::BadEncoding);
        }
        let x_bytes: [u8; X25519_LEN] = bytes[..X25519_LEN].try_into().unwrap();
        let ml_bytes: [u8; MLKEM_DK] = bytes[X25519_LEN..].try_into().unwrap();
        Ok(KemPrivateKey {
            x: x25519_dalek::StaticSecret::from(x_bytes),
            ml: ml_kem_1024::DecapsKey::try_from_bytes(ml_bytes).map_err(pq)?,
        })
    }

    /// Recover the shared secret from an encapsulation (the recipient side).
    pub fn decapsulate(&self, enc: &Encapsulation) -> Result<SharedSecret, CryptoError> {
        let x_eph = x25519_dalek::PublicKey::from(enc.x_eph);
        let x_ss = self.x.diffie_hellman(&x_eph);
        let ct = ml_kem_1024::CipherText::try_from_bytes(enc.ml_ct).map_err(pq)?;
        let ml_ss = self.ml.try_decaps(&ct).map_err(pq)?;
        let x_recip = x25519_dalek::PublicKey::from(&self.x);
        Ok(combine(
            x_ss.as_bytes(),
            &ml_ss.into_bytes(),
            &enc.ml_ct,
            &enc.x_eph,
            x_recip.as_bytes(),
        ))
    }
}

impl KemPublicKey {
    /// Encapsulate to this key: returns the transport encapsulation and the shared
    /// secret (the sender side).
    pub fn encapsulate(&self) -> Result<(Encapsulation, SharedSecret), CryptoError> {
        let x_eph_sk = x25519_dalek::StaticSecret::from(os_random::<32>());
        let x_eph_pk = x25519_dalek::PublicKey::from(&x_eph_sk);
        let x_ss = x_eph_sk.diffie_hellman(&self.x);
        let (ml_ss, ml_ct) = self.ml.try_encaps().map_err(pq)?;
        let ml_ct_bytes = ml_ct.into_bytes();
        let ss = combine(
            x_ss.as_bytes(),
            &ml_ss.into_bytes(),
            &ml_ct_bytes,
            x_eph_pk.as_bytes(),
            self.x.as_bytes(),
        );
        Ok((
            Encapsulation {
                x_eph: *x_eph_pk.as_bytes(),
                ml_ct: ml_ct_bytes,
            },
            ss,
        ))
    }

    /// `x25519 pub (32) ‖ ml-kem-1024 encaps key`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(X25519_LEN + MLKEM_EK);
        out.extend_from_slice(self.x.as_bytes());
        out.extend_from_slice(&self.ml.clone().into_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != X25519_LEN + MLKEM_EK {
            return Err(CryptoError::BadEncoding);
        }
        let x_bytes: [u8; X25519_LEN] = bytes[..X25519_LEN].try_into().unwrap();
        let ml_bytes: [u8; MLKEM_EK] = bytes[X25519_LEN..].try_into().unwrap();
        Ok(KemPublicKey {
            x: x25519_dalek::PublicKey::from(x_bytes),
            ml: ml_kem_1024::EncapsKey::try_from_bytes(ml_bytes).map_err(pq)?,
        })
    }
}

impl Encapsulation {
    /// `x25519 ephemeral pub (32) ‖ ml-kem-1024 ciphertext`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(X25519_LEN + MLKEM_CT);
        out.extend_from_slice(&self.x_eph);
        out.extend_from_slice(&self.ml_ct);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != X25519_LEN + MLKEM_CT {
            return Err(CryptoError::BadEncoding);
        }
        Ok(Encapsulation {
            x_eph: bytes[..X25519_LEN].try_into().unwrap(),
            ml_ct: bytes[X25519_LEN..].try_into().unwrap(),
        })
    }
}

/// The hybrid KEM combiner: derive a 32-byte shared secret from both component
/// secrets, bound to the transcript (ciphertext + ephemeral + recipient pub) so a
/// swap of either component is detected. IND-CCA2 if **either** KEM holds.
fn combine(
    x_ss: &[u8],
    ml_ss: &[u8],
    ml_ct: &[u8],
    x_eph: &[u8],
    x_recip: &[u8],
) -> SharedSecret {
    let mut ikm = Vec::new();
    ikm.extend_from_slice(x_ss);
    ikm.extend_from_slice(ml_ss);
    ikm.extend_from_slice(ml_ct);
    ikm.extend_from_slice(x_eph);
    ikm.extend_from_slice(x_recip);
    let hk = Hkdf::<Sha384>::new(None, &ikm);
    let mut okm = [0u8; 32];
    let info = [SUITE_ID.as_bytes(), b"/kem"].concat();
    hk.expand(&info, &mut okm).expect("hkdf expand");
    SharedSecret(okm)
}

// ===========================================================================
// Seal / open — HPKE-like: encapsulate → derive AEAD material → XChaCha20-Poly1305
// ===========================================================================

/// A message sealed to a [`KemPublicKey`]: the encapsulation plus AEAD ciphertext.
pub struct Sealed {
    pub enc: Encapsulation,
    pub ciphertext: Vec<u8>,
}

/// Seal `plaintext` to `pk`, authenticating `aad`. Only the holder of the matching
/// private key can [`open`](open_sealed) it — this is the DRM content-key delivery
/// (drm.md Annexe A).
pub fn seal(pk: &KemPublicKey, aad: &[u8], plaintext: &[u8]) -> Result<Sealed, CryptoError> {
    let (enc, ss) = pk.encapsulate()?;
    let (key, nonce) = seal_material(&ss);
    let ciphertext = aead_seal(&key, &nonce, aad, plaintext)?;
    Ok(Sealed { enc, ciphertext })
}

/// Open a [`Sealed`] message with the recipient private key.
pub fn open_sealed(sk: &KemPrivateKey, sealed: &Sealed, aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let ss = sk.decapsulate(&sealed.enc)?;
    let (key, nonce) = seal_material(&ss);
    aead_open(&key, &nonce, aad, &sealed.ciphertext)
}

impl Sealed {
    /// `encapsulation ‖ ciphertext` (the ml-kem ciphertext has a fixed length, so
    /// the split point is known).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.enc.to_bytes();
        out.extend_from_slice(&self.ciphertext);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let split = X25519_LEN + MLKEM_CT;
        if bytes.len() < split {
            return Err(CryptoError::BadEncoding);
        }
        Ok(Sealed {
            enc: Encapsulation::from_bytes(&bytes[..split])?,
            ciphertext: bytes[split..].to_vec(),
        })
    }
}

/// Derive the AEAD key + nonce from a KEM shared secret (domain-separated).
fn seal_material(ss: &SharedSecret) -> ([u8; AEAD_KEY], [u8; AEAD_NONCE]) {
    let hk = Hkdf::<Sha384>::new(None, ss.as_bytes());
    let mut okm = [0u8; AEAD_KEY + AEAD_NONCE];
    let info = [SUITE_ID.as_bytes(), b"/seal"].concat();
    hk.expand(&info, &mut okm).expect("hkdf expand");
    let mut key = [0u8; AEAD_KEY];
    let mut nonce = [0u8; AEAD_NONCE];
    key.copy_from_slice(&okm[..AEAD_KEY]);
    nonce.copy_from_slice(&okm[AEAD_KEY..]);
    (key, nonce)
}

// ===========================================================================
// AEAD — XChaCha20-Poly1305 (payload content keys, drm.md Annexe B)
// ===========================================================================

/// AEAD-encrypt with an explicit key + 24-byte nonce, authenticating `aad`.
/// Returns `ciphertext ‖ tag`.
pub fn aead_seal(
    key: &[u8; AEAD_KEY],
    nonce: &[u8; AEAD_NONCE],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::XChaCha20Poly1305;
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(nonce.into(), Payload { msg: plaintext, aad })
        .map_err(|_| CryptoError::AeadOpen)
}

/// AEAD-decrypt the output of [`aead_seal`].
pub fn aead_open(
    key: &[u8; AEAD_KEY],
    nonce: &[u8; AEAD_NONCE],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::XChaCha20Poly1305;
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(nonce.into(), Payload { msg: ciphertext, aad })
        .map_err(|_| CryptoError::AeadOpen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_signature_round_trip() {
        let sk = SigningKey::generate().unwrap();
        let vk = sk.verifying_key();
        let msg = b"release: obs@1.4.0";
        let sig = sk.sign(msg).unwrap();
        assert!(vk.verify(msg, &sig).is_ok());
        // Tampered message rejected.
        assert!(vk.verify(b"release: obs@9.9.9", &sig).is_err());
    }

    #[test]
    fn tampering_either_signature_half_fails() {
        let sk = SigningKey::generate().unwrap();
        let vk = sk.verifying_key();
        let msg = b"hello";
        let sig = sk.sign(msg).unwrap();
        // Flip a bit in the ed25519 half.
        let mut a = sig.clone();
        a.ed[0] ^= 1;
        assert!(vk.verify(msg, &a).is_err());
        // Flip a bit in the ml-dsa half.
        let mut b = sig.clone();
        b.ml[0] ^= 1;
        assert!(vk.verify(msg, &b).is_err());
    }

    #[test]
    fn signature_key_serialization_round_trips() {
        let sk = SigningKey::generate().unwrap();
        let sk2 = SigningKey::from_bytes(&sk.to_bytes()).unwrap();
        let vk = VerifyingKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
        let msg = b"data";
        let sig = Signature::from_bytes(&sk2.sign(msg).unwrap().to_bytes()).unwrap();
        assert!(vk.verify(msg, &sig).is_ok());
    }

    #[test]
    fn kem_encaps_decaps_agree() {
        let (sk, pk) = KemPrivateKey::generate().unwrap();
        let (enc, ss_sender) = pk.encapsulate().unwrap();
        let ss_recipient = sk.decapsulate(&enc).unwrap();
        assert_eq!(ss_sender.as_bytes(), ss_recipient.as_bytes());
    }

    #[test]
    fn kem_wrong_key_yields_different_secret() {
        let (_sk, pk) = KemPrivateKey::generate().unwrap();
        let (other_sk, _other_pk) = KemPrivateKey::generate().unwrap();
        let (enc, ss_sender) = pk.encapsulate().unwrap();
        // A different private key must not recover the same secret.
        let ss_other = other_sk.decapsulate(&enc).unwrap();
        assert_ne!(ss_sender.as_bytes(), ss_other.as_bytes());
    }

    #[test]
    fn kem_key_and_encapsulation_serialization() {
        let (sk, pk) = KemPrivateKey::generate().unwrap();
        let pk2 = KemPublicKey::from_bytes(&pk.to_bytes()).unwrap();
        let sk2 = KemPrivateKey::from_bytes(&sk.to_bytes()).unwrap();
        let (enc, ss) = pk2.encapsulate().unwrap();
        let enc2 = Encapsulation::from_bytes(&enc.to_bytes()).unwrap();
        assert_eq!(sk2.decapsulate(&enc2).unwrap().as_bytes(), ss.as_bytes());
    }

    #[test]
    fn seal_open_round_trip_and_binding() {
        let (sk, pk) = KemPrivateKey::generate().unwrap();
        let aad = b"activation-record-hash";
        let pt = b"the 32-byte content key ........";
        let sealed = seal(&pk, aad, pt).unwrap();
        // Serialize the sealed blob (as stored in an activation record).
        let sealed = Sealed::from_bytes(&sealed.to_bytes()).unwrap();
        assert_eq!(open_sealed(&sk, &sealed, aad).unwrap(), pt);
        // Wrong aad is rejected.
        assert!(open_sealed(&sk, &sealed, b"other-aad").is_err());
        // Wrong key is rejected.
        let (other_sk, _) = KemPrivateKey::generate().unwrap();
        assert!(open_sealed(&other_sk, &sealed, aad).is_err());
    }

    #[test]
    fn aead_round_trip_and_tamper_detection() {
        let key = [7u8; AEAD_KEY];
        let nonce = [9u8; AEAD_NONCE];
        let aad = b"qtpkg/v1\0obs.wasm";
        let pt = b"plaintext wasm bytes";
        let mut ct = aead_seal(&key, &nonce, aad, pt).unwrap();
        assert_eq!(aead_open(&key, &nonce, aad, &ct).unwrap(), pt);
        // Flip a ciphertext bit → tag mismatch.
        ct[0] ^= 1;
        assert!(aead_open(&key, &nonce, aad, &ct).is_err());
    }

    #[test]
    fn suite_id_is_stable() {
        assert_eq!(SUITE_ID, "hyb-mlkem1024-mldsa87-v1");
    }
}
