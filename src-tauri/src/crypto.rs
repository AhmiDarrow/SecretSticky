//! Argon2id KDF + XChaCha20-Poly1305 AEAD for vault secrets.

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Algorithm, Argon2, Params, Version,
};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::{AppError, AppResult};

/// Tuned for ~0.5–1s on typical desktop CPUs. Stored in vault header.
pub const ARGON2_M_KIB: u32 = 64 * 1024; // 64 MiB
pub const ARGON2_T: u32 = 3;
pub const ARGON2_P: u32 = 1;
pub const SALT_LEN: usize = 16;
pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey(pub [u8; KEY_LEN]);

impl MasterKey {
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> AppResult<Self> {
        if bytes.len() != KEY_LEN {
            return Err(AppError::Crypto(format!(
                "unexpected key length {}",
                bytes.len()
            )));
        }
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }
}

impl AsRef<[u8]> for MasterKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

pub fn random_array<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

fn argon2_instance(m_kib: u32, t: u32, p: u32) -> AppResult<Argon2<'static>> {
    let params = Params::new(m_kib, t, p, Some(KEY_LEN))
        .map_err(|e| AppError::Crypto(format!("argon2 params: {e}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Derive a 32-byte master key from password + salt.
pub fn derive_key(password: &str, salt: &[u8], m_kib: u32, t: u32, p: u32) -> AppResult<MasterKey> {
    let argon2 = argon2_instance(m_kib, t, p)?;
    let salt =
        SaltString::encode_b64(salt).map_err(|e| AppError::Crypto(format!("salt encode: {e}")))?;
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Crypto(format!("argon2: {e}")))?;
    let hash_bytes = hash
        .hash
        .ok_or_else(|| AppError::Crypto("missing hash".into()))?;
    let bytes = hash_bytes.as_bytes();
    if bytes.len() != KEY_LEN {
        return Err(AppError::Crypto(format!(
            "unexpected key length {}",
            bytes.len()
        )));
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(bytes);
    Ok(MasterKey(key))
}

/// Encrypt plaintext with associated data (AAD). Returns nonce || ciphertext+tag.
pub fn encrypt(key: &MasterKey, plaintext: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce_bytes = random_array::<NONCE_LEN>();
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AppError::Crypto("encrypt failed".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt nonce || ciphertext+tag with AAD.
pub fn decrypt(key: &MasterKey, blob: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
    if blob.len() < NONCE_LEN + 16 {
        return Err(AppError::Crypto("ciphertext too short".into()));
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce = XNonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, Payload { msg: ct, aad })
        .map_err(|_| AppError::BadPassword)
}

/// Generate a human-friendly recovery key (32 bytes → base32-ish hex groups).
pub fn generate_recovery_key() -> String {
    let bytes = random_array::<32>();
    let hex = hex::encode(bytes);
    hex.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or("????"))
        .collect::<Vec<_>>()
        .join("-")
        .to_uppercase()
}

/// Normalize recovery key input (strip spaces/dashes, lowercase).
pub fn normalize_recovery_key(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let salt = random_array::<SALT_LEN>();
        let key = derive_key("test-password-hunter2", &salt, 8 * 1024, 1, 1).unwrap();
        let pt = b"api_key=sk-secret-123";
        let aad = b"note:uuid";
        let blob = encrypt(&key, pt, aad).unwrap();
        let out = decrypt(&key, &blob, aad).unwrap();
        assert_eq!(out, pt);
    }

    #[test]
    fn wrong_password_fails() {
        let salt = random_array::<SALT_LEN>();
        let key = derive_key("correct", &salt, 8 * 1024, 1, 1).unwrap();
        let wrong = derive_key("wrong", &salt, 8 * 1024, 1, 1).unwrap();
        let blob = encrypt(&key, b"secret", b"aad").unwrap();
        assert!(decrypt(&wrong, &blob, b"aad").is_err());
    }

    #[test]
    fn wrong_aad_fails() {
        let salt = random_array::<SALT_LEN>();
        let key = derive_key("correct", &salt, 8 * 1024, 1, 1).unwrap();
        let blob = encrypt(&key, b"secret", b"aad-a").unwrap();
        assert!(decrypt(&key, &blob, b"aad-b").is_err());
    }

    #[test]
    fn recovery_key_normalize() {
        let k = generate_recovery_key();
        assert!(k.contains('-'));
        let n = normalize_recovery_key(&k);
        assert_eq!(n.len(), 64);
        assert!(!n.contains('-'));
    }

    #[test]
    fn ciphertext_too_short_fails() {
        let salt = random_array::<SALT_LEN>();
        let key = derive_key("pw", &salt, 8 * 1024, 1, 1).unwrap();
        assert!(decrypt(&key, &[0u8; 10], b"aad").is_err());
    }

    #[test]
    fn master_key_from_slice_rejects_bad_len() {
        assert!(MasterKey::from_slice(&[1, 2, 3]).is_err());
        let ok = MasterKey::from_slice(&[0u8; KEY_LEN]).unwrap();
        assert_eq!(ok.as_bytes().len(), KEY_LEN);
    }

    #[test]
    fn different_nonces_each_encrypt() {
        let salt = random_array::<SALT_LEN>();
        let key = derive_key("pw", &salt, 8 * 1024, 1, 1).unwrap();
        let a = encrypt(&key, b"same", b"aad").unwrap();
        let b = encrypt(&key, b"same", b"aad").unwrap();
        assert_ne!(a, b, "nonces must be random per encrypt");
        assert_eq!(decrypt(&key, &a, b"aad").unwrap(), b"same");
        assert_eq!(decrypt(&key, &b, b"aad").unwrap(), b"same");
    }

    #[test]
    fn normalize_strips_spaces_and_mixed_case() {
        let n = normalize_recovery_key(" Ab Cd-Ef 12 ");
        assert_eq!(n, "abcdef12");
    }
}
