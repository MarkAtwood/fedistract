//! End-to-end encrypted direct messages for ActivityPub.
//!
//! Uses X25519 key exchange (derived from Ed25519 actor keys) +
//! XChaCha20-Poly1305 encryption. Backwards-compatible: encrypted content
//! goes in an `encryptedContent` field; the `content` field contains a
//! human-readable fallback for unaware servers.
//!
//! # Usage
//!
//! ```
//! use fedi_e2ee::{encrypt_dm, decrypt_dm};
//! use fedi_did::generate_recovery_keypair;
//!
//! let (sender_priv, sender_pub) = generate_recovery_keypair();
//! let (recip_priv, recip_pub) = generate_recovery_keypair();
//!
//! let encrypted = encrypt_dm("Secret message", &sender_priv, &recip_pub).unwrap();
//! let plaintext = decrypt_dm(&encrypted, &recip_priv, &sender_pub).unwrap();
//! assert_eq!(plaintext, "Secret message");
//! ```

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};
use zeroize::Zeroize;

/// The fallback content shown by servers that don't support E2EE.
pub const FALLBACK_CONTENT: &str =
    "This message is encrypted. Your server does not support encrypted messages.";

#[derive(Debug, thiserror::Error)]
pub enum E2eeError {
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("decryption failed (wrong key or tampered ciphertext)")]
    DecryptionFailed,
    #[error("invalid nonce encoding: {0}")]
    InvalidNonce(String),
    #[error("invalid ciphertext encoding: {0}")]
    InvalidCiphertext(String),
}

/// An encrypted direct message.
#[derive(Debug, Clone)]
pub struct EncryptedMessage {
    /// Multibase-encoded ciphertext (XChaCha20-Poly1305).
    pub ciphertext: String,
    /// Multibase-encoded 24-byte nonce.
    pub nonce: String,
    /// did:key of the sender (for key discovery on decrypt).
    pub sender_did: String,
}

/// Encrypt a direct message.
///
/// Uses the sender's Ed25519 private key and recipient's Ed25519 public key.
/// Keys are converted to X25519 for ECDH key agreement.
pub fn encrypt_dm(
    plaintext: &str,
    sender_private: &[u8; 32],
    recipient_public: &[u8; 32],
) -> Result<EncryptedMessage, E2eeError> {
    let mut shared_secret = derive_shared_secret(sender_private, recipient_public);
    let mut encryption_key = derive_encryption_key(&shared_secret);
    shared_secret.zeroize();

    // Generate random 24-byte nonce (XChaCha20 = safe with random nonces)
    let mut nonce_bytes = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce_bytes);

    let cipher = XChaCha20Poly1305::new_from_slice(&encryption_key)
        .map_err(|_| E2eeError::EncryptionFailed)?;
    encryption_key.zeroize();

    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext_bytes = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| E2eeError::EncryptionFailed)?;

    let sender_pub = fedi_did::ed25519_public_from_private(sender_private);
    let sender_did = fedi_did::ed25519_to_did_key(&sender_pub);

    Ok(EncryptedMessage {
        ciphertext: multibase::encode(multibase::Base::Base64Url, &ciphertext_bytes),
        nonce: multibase::encode(multibase::Base::Base64Url, &nonce_bytes),
        sender_did,
    })
}

/// Decrypt a direct message.
///
/// Uses the recipient's Ed25519 private key and sender's Ed25519 public key.
pub fn decrypt_dm(
    msg: &EncryptedMessage,
    recipient_private: &[u8; 32],
    sender_public: &[u8; 32],
) -> Result<String, E2eeError> {
    let (_, ciphertext_bytes) = multibase::decode(&msg.ciphertext)
        .map_err(|e| E2eeError::InvalidCiphertext(e.to_string()))?;

    let (_, nonce_bytes) = multibase::decode(&msg.nonce)
        .map_err(|e| E2eeError::InvalidNonce(e.to_string()))?;

    if nonce_bytes.len() != 24 {
        return Err(E2eeError::InvalidNonce(format!(
            "expected 24 bytes, got {}",
            nonce_bytes.len()
        )));
    }

    let mut shared_secret = derive_shared_secret(recipient_private, sender_public);
    let mut encryption_key = derive_encryption_key(&shared_secret);
    shared_secret.zeroize();

    let cipher = XChaCha20Poly1305::new_from_slice(&encryption_key)
        .map_err(|_| E2eeError::DecryptionFailed)?;
    encryption_key.zeroize();

    let nonce = XNonce::from_slice(&nonce_bytes);
    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext_bytes.as_ref())
        .map_err(|_| E2eeError::DecryptionFailed)?;

    String::from_utf8(plaintext_bytes).map_err(|_| E2eeError::DecryptionFailed)
}

/// Add encrypted content to an ActivityPub Note.
///
/// Sets `encryptedContent` with the ciphertext and replaces `content`
/// with a human-readable fallback for unaware servers.
pub fn prepare_dm_note(note: &mut serde_json::Value, encrypted: &EncryptedMessage) {
    if let Some(obj) = note.as_object_mut() {
        obj.insert(
            "encryptedContent".into(),
            serde_json::json!({
                "type": "X25519-XChaCha20-Poly1305",
                "ciphertext": encrypted.ciphertext,
                "nonce": encrypted.nonce,
                "senderDid": encrypted.sender_did
            }),
        );
        obj.insert("content".into(), serde_json::Value::String(FALLBACK_CONTENT.into()));
    }
}

/// Extract encrypted content from a received Note, if present.
pub fn extract_encrypted_content(note: &serde_json::Value) -> Option<EncryptedMessage> {
    let enc = note.get("encryptedContent")?;
    Some(EncryptedMessage {
        ciphertext: enc.get("ciphertext")?.as_str()?.to_string(),
        nonce: enc.get("nonce")?.as_str()?.to_string(),
        sender_did: enc.get("senderDid")?.as_str()?.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Internal crypto
// ---------------------------------------------------------------------------

/// Convert Ed25519 private key to X25519 and perform ECDH.
fn derive_shared_secret(our_private: &[u8; 32], their_public_ed: &[u8; 32]) -> [u8; 32] {
    // Ed25519 private → X25519 private: clamp and use as scalar
    let ed_signing = ed25519_dalek::SigningKey::from_bytes(our_private);
    let x_secret = ed25519_to_x25519_private(&ed_signing);

    // Ed25519 public → X25519 public: use Montgomery form
    let x_public = ed25519_to_x25519_public(their_public_ed);

    x_secret.diffie_hellman(&x_public).to_bytes()
}

/// Derive a 32-byte encryption key from the shared secret using HKDF.
fn derive_encryption_key(shared_secret: &[u8; 32]) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(None, shared_secret);
    let mut key = [0u8; 32];
    hkdf.expand(b"fedi-e2ee-xchacha20-v1", &mut key)
        .expect("32 bytes is a valid HKDF output length");
    key
}

/// Convert Ed25519 signing key to X25519 static secret.
fn ed25519_to_x25519_private(signing_key: &ed25519_dalek::SigningKey) -> X25519Secret {
    // Hash the seed with SHA-512, take first 32 bytes, clamp
    use sha2::{Digest, Sha512};
    let hash = Sha512::digest(signing_key.as_bytes());
    let mut x_bytes = [0u8; 32];
    x_bytes.copy_from_slice(&hash[..32]);
    X25519Secret::from(x_bytes)
}

/// Convert Ed25519 public key bytes to X25519 public key.
fn ed25519_to_x25519_public(ed_public: &[u8; 32]) -> X25519Public {
    // Convert Edwards point to Montgomery point
    let ed_point = curve25519_dalek::edwards::CompressedEdwardsY(*ed_public);
    let montgomery = ed_point
        .decompress()
        .expect("valid Ed25519 public key")
        .to_montgomery();
    X25519Public::from(montgomery.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fedi_did::generate_recovery_keypair;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let (sender_priv, sender_pub) = generate_recovery_keypair();
        let (recip_priv, recip_pub) = generate_recovery_keypair();

        let encrypted = encrypt_dm("Hello, secret world!", &sender_priv, &recip_pub).unwrap();
        let plaintext = decrypt_dm(&encrypted, &recip_priv, &sender_pub).unwrap();
        assert_eq!(plaintext, "Hello, secret world!");
    }

    #[test]
    fn wrong_recipient_key_fails() {
        let (sender_priv, _sender_pub) = generate_recovery_keypair();
        let (_recip_priv, recip_pub) = generate_recovery_keypair();
        let (wrong_priv, _) = generate_recovery_keypair();

        let encrypted = encrypt_dm("secret", &sender_priv, &recip_pub).unwrap();
        let result = decrypt_dm(&encrypted, &wrong_priv, &_sender_pub);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_sender_key_fails() {
        let (sender_priv, _sender_pub) = generate_recovery_keypair();
        let (recip_priv, recip_pub) = generate_recovery_keypair();
        let (_, wrong_pub) = generate_recovery_keypair();

        let encrypted = encrypt_dm("secret", &sender_priv, &recip_pub).unwrap();
        let result = decrypt_dm(&encrypted, &recip_priv, &wrong_pub);
        assert!(result.is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let (sender_priv, sender_pub) = generate_recovery_keypair();
        let (recip_priv, recip_pub) = generate_recovery_keypair();

        let mut encrypted = encrypt_dm("secret", &sender_priv, &recip_pub).unwrap();
        // Tamper with ciphertext
        encrypted.ciphertext = multibase::encode(multibase::Base::Base64Url, b"tampered data here!!");

        let result = decrypt_dm(&encrypted, &recip_priv, &sender_pub);
        assert!(result.is_err());
    }

    #[test]
    fn empty_message_works() {
        let (sender_priv, sender_pub) = generate_recovery_keypair();
        let (recip_priv, recip_pub) = generate_recovery_keypair();

        let encrypted = encrypt_dm("", &sender_priv, &recip_pub).unwrap();
        let plaintext = decrypt_dm(&encrypted, &recip_priv, &sender_pub).unwrap();
        assert_eq!(plaintext, "");
    }

    #[test]
    fn unicode_message_works() {
        let (sender_priv, sender_pub) = generate_recovery_keypair();
        let (recip_priv, recip_pub) = generate_recovery_keypair();

        let msg = "Hello 🌍! Привет мир! こんにちは世界!";
        let encrypted = encrypt_dm(msg, &sender_priv, &recip_pub).unwrap();
        let plaintext = decrypt_dm(&encrypted, &recip_priv, &sender_pub).unwrap();
        assert_eq!(plaintext, msg);
    }

    #[test]
    fn sender_did_is_set() {
        let (sender_priv, sender_pub) = generate_recovery_keypair();
        let (_, recip_pub) = generate_recovery_keypair();

        let encrypted = encrypt_dm("test", &sender_priv, &recip_pub).unwrap();
        let expected_did = fedi_did::ed25519_to_did_key(&sender_pub);
        assert_eq!(encrypted.sender_did, expected_did);
    }

    #[test]
    fn prepare_dm_note_adds_encrypted_content() {
        let (sender_priv, _sender_pub) = generate_recovery_keypair();
        let (_, recip_pub) = generate_recovery_keypair();

        let encrypted = encrypt_dm("secret", &sender_priv, &recip_pub).unwrap();

        let mut note = serde_json::json!({
            "type": "Note",
            "content": "original content",
            "to": ["https://example.com/users/bob"]
        });

        prepare_dm_note(&mut note, &encrypted);

        assert_eq!(note["content"], FALLBACK_CONTENT);
        assert!(note.get("encryptedContent").is_some());
        assert_eq!(note["encryptedContent"]["type"], "X25519-XChaCha20-Poly1305");
        assert!(note["encryptedContent"]["ciphertext"].as_str().is_some());
        assert!(note["encryptedContent"]["nonce"].as_str().is_some());
        assert!(note["encryptedContent"]["senderDid"].as_str().is_some());
    }

    #[test]
    fn extract_encrypted_content_round_trip() {
        let (sender_priv, sender_pub) = generate_recovery_keypair();
        let (recip_priv, recip_pub) = generate_recovery_keypair();

        let encrypted = encrypt_dm("hello from note", &sender_priv, &recip_pub).unwrap();

        let mut note = serde_json::json!({"type": "Note", "content": "original"});
        prepare_dm_note(&mut note, &encrypted);

        let extracted = extract_encrypted_content(&note).expect("should have encrypted content");
        let plaintext = decrypt_dm(&extracted, &recip_priv, &sender_pub).unwrap();
        assert_eq!(plaintext, "hello from note");
    }

    #[test]
    fn extract_from_unencrypted_note_returns_none() {
        let note = serde_json::json!({"type": "Note", "content": "plain text"});
        assert!(extract_encrypted_content(&note).is_none());
    }

    #[test]
    fn different_encryptions_produce_different_ciphertext() {
        let (sender_priv, _) = generate_recovery_keypair();
        let (_, recip_pub) = generate_recovery_keypair();

        let e1 = encrypt_dm("same message", &sender_priv, &recip_pub).unwrap();
        let e2 = encrypt_dm("same message", &sender_priv, &recip_pub).unwrap();

        // Different nonces → different ciphertext (probabilistic encryption)
        assert_ne!(e1.ciphertext, e2.ciphertext);
        assert_ne!(e1.nonce, e2.nonce);
    }
}
