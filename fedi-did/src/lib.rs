//! DID (Decentralized Identifier) utilities for fediverse servers.
//!
//! Provides `did:key`, `did:web`, and `did:plc` generation, BIP-39 mnemonic
//! recovery key management, and W3C DID document construction.
//!
//! # Usage
//!
//! ```
//! use fedi_did::{generate_recovery_keypair, ed25519_to_did_key, did_web};
//!
//! let (private_key, public_key) = generate_recovery_keypair();
//! let did_key = ed25519_to_did_key(&public_key);
//! assert!(did_key.starts_with("did:key:z6Mk"));
//!
//! let did = did_web("example.com", "alice");
//! assert_eq!(did, "did:web:example.com:users:alice");
//! ```

#[derive(Debug, thiserror::Error)]
pub enum DidError {
    #[error("invalid recovery phrase: {0}")]
    InvalidMnemonic(String),
    #[error("invalid hex: {0}")]
    InvalidHex(String),
}

/// Generate an Ed25519 recovery keypair. Returns (private_key_bytes, public_key_bytes).
pub fn generate_recovery_keypair() -> ([u8; 32], [u8; 32]) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (signing_key.to_bytes(), verifying_key.to_bytes())
}

/// Encode an Ed25519 public key as `did:key` (multicodec 0xed01 + multibase base58btc).
///
/// Returns a string like `did:key:z6Mk...`.
pub fn ed25519_to_did_key(public_key: &[u8; 32]) -> String {
    let mut buf = Vec::with_capacity(34);
    buf.push(0xed);
    buf.push(0x01);
    buf.extend_from_slice(public_key);

    let encoded = multibase::encode(multibase::Base::Base58Btc, &buf);
    format!("did:key:{encoded}")
}

/// Derive `did:web` from domain and username.
///
/// Returns `did:web:{domain}:users:{username}`. Dots in the domain are
/// preserved per the DID Web spec (colons separate path components).
pub fn did_web(domain: &str, username: &str) -> String {
    format!("did:web:{domain}:users:{username}")
}

/// Encode recovery private key as BIP-39 24-word mnemonic.
pub fn private_key_to_mnemonic(private_key: &[u8; 32]) -> String {
    let mnemonic =
        bip39::Mnemonic::from_entropy(private_key).expect("32 bytes is valid BIP-39 entropy");
    mnemonic.to_string()
}

/// Decode BIP-39 mnemonic back to private key bytes.
pub fn mnemonic_to_private_key(phrase: &str) -> Result<[u8; 32], DidError> {
    let mnemonic = bip39::Mnemonic::parse(phrase)
        .map_err(|e| DidError::InvalidMnemonic(e.to_string()))?;
    let entropy = mnemonic.to_entropy();
    if entropy.len() != 32 {
        return Err(DidError::InvalidMnemonic(
            "recovery phrase must encode exactly 32 bytes (24 words)".into(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&entropy);
    Ok(key)
}

/// Derive the Ed25519 public key from a private key.
pub fn ed25519_public_from_private(private_key: &[u8; 32]) -> [u8; 32] {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(private_key);
    signing_key.verifying_key().to_bytes()
}

/// Build a W3C DID document for `did:web`.
///
/// Includes the RSA public key as the primary verification method (for
/// ActivityPub HTTP signatures). When `recovery_pubkey` is provided, adds
/// an Ed25519 verification method for recovery.
pub fn did_web_document(
    domain: &str,
    username: &str,
    rsa_pub_pem: &str,
    recovery_pubkey: Option<&[u8; 32]>,
    also_known_as: &[String],
) -> serde_json::Value {
    let did = did_web(domain, username);

    let mut aka = serde_json::Value::Array(
        also_known_as
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
    );
    if aka.as_array().map_or(true, |a| a.is_empty()) {
        aka = serde_json::Value::Null;
    }

    let mut verification_methods = vec![serde_json::json!({
        "id": format!("{did}#main-key"),
        "type": "RsaVerificationKey2018",
        "controller": did,
        "publicKeyPem": rsa_pub_pem
    })];

    let mut contexts: Vec<serde_json::Value> =
        vec![serde_json::json!("https://www.w3.org/ns/did/v1")];

    if let Some(rpk) = recovery_pubkey {
        contexts.push(serde_json::json!(
            "https://w3id.org/security/suites/ed25519-2020/v1"
        ));
        let mut buf = Vec::with_capacity(34);
        buf.push(0xed);
        buf.push(0x01);
        buf.extend_from_slice(rpk);
        verification_methods.push(serde_json::json!({
            "id": format!("{did}#recovery-key"),
            "type": "Ed25519VerificationKey2020",
            "controller": did,
            "publicKeyMultibase": multibase::encode(multibase::Base::Base58Btc, buf)
        }));
    }

    serde_json::json!({
        "@context": contexts,
        "id": did,
        "alsoKnownAs": aka,
        "verificationMethod": verification_methods,
        "authentication": ["#main-key"],
        "assertionMethod": ["#main-key"],
    })
}

/// Generate a `did:plc` from recovery and signing public keys.
///
/// Computes SHA-256 of a canonical genesis operation, then base32-encodes
/// the first 15 bytes (lowercase, no padding) to produce the identifier.
pub fn generate_did_plc(
    recovery_pub: &[u8; 32],
    signing_pub: &[u8; 32],
) -> String {
    use sha2::{Digest, Sha256};

    let recovery_did = ed25519_to_did_key(recovery_pub);
    let signing_did = ed25519_to_did_key(signing_pub);

    let genesis = serde_json::json!({
        "rotationKeys": [recovery_did],
        "signingKey": signing_did,
        "type": "plc_operation",
        "verificationMethods": {}
    });

    let genesis_bytes = serde_json::to_vec(&genesis)
        .expect("JSON serialization cannot fail for static structure");

    let hash = Sha256::digest(&genesis_bytes);

    let encoded = base32_encode_lower(&hash[..15]);
    format!("did:plc:{encoded}")
}

/// Hex-encode bytes for storage.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex-decode a string back to bytes.
pub fn hex_decode(s: &str) -> Result<Vec<u8>, DidError> {
    if s.len() % 2 != 0 {
        return Err(DidError::InvalidHex("odd-length string".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| DidError::InvalidHex(format!("invalid byte at position {i}")))
        })
        .collect()
}

/// Base32 lowercase encoding without padding (RFC 4648 variant).
fn base32_encode_lower(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut result = String::with_capacity((data.len() * 8).div_ceil(5));
    let mut buffer: u64 = 0;
    let mut bits = 0;

    for &byte in data {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1f) as usize;
            result.push(ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
        result.push(ALPHABET[idx] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonic_round_trip() {
        let (private_key, public_key) = generate_recovery_keypair();
        let mnemonic = private_key_to_mnemonic(&private_key);
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 24, "BIP-39 256-bit entropy produces 24 words");

        let recovered = mnemonic_to_private_key(&mnemonic).unwrap();
        assert_eq!(recovered, private_key);

        let recovered_pub = ed25519_public_from_private(&recovered);
        assert_eq!(recovered_pub, public_key);
    }

    #[test]
    fn did_key_format() {
        let (_, public_key) = generate_recovery_keypair();
        let did = ed25519_to_did_key(&public_key);
        assert!(
            did.starts_with("did:key:z6Mk"),
            "Ed25519 did:key starts with z6Mk, got: {did}"
        );
    }

    #[test]
    fn did_key_deterministic() {
        let key = [42u8; 32];
        let a = ed25519_to_did_key(&key);
        let b = ed25519_to_did_key(&key);
        assert_eq!(a, b);
    }

    #[test]
    fn did_web_format() {
        let did = did_web("example.com", "alice");
        assert_eq!(did, "did:web:example.com:users:alice");
    }

    #[test]
    fn did_plc_format() {
        let (_, recovery_pub) = generate_recovery_keypair();
        let (_, signing_pub) = generate_recovery_keypair();
        let plc = generate_did_plc(&recovery_pub, &signing_pub);
        assert!(plc.starts_with("did:plc:"));
        let id_part = plc.strip_prefix("did:plc:").unwrap();
        assert_eq!(id_part.len(), 24, "did:plc id is 24 base32 chars (15 bytes)");
    }

    #[test]
    fn did_plc_deterministic() {
        let recovery = [1u8; 32];
        let signing = [2u8; 32];
        let a = generate_did_plc(&recovery, &signing);
        let b = generate_did_plc(&recovery, &signing);
        assert_eq!(a, b);
    }

    #[test]
    fn did_web_document_with_recovery_key() {
        let recovery_pub = [42u8; 32];
        let doc = did_web_document(
            "example.com",
            "alice",
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
            Some(&recovery_pub),
            &["did:key:z6Mk1234".to_string()],
        );
        assert_eq!(doc["id"], "did:web:example.com:users:alice");

        let methods = doc["verificationMethod"].as_array().unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0]["type"], "RsaVerificationKey2018");
        assert!(methods[0]["id"].as_str().unwrap().ends_with("#main-key"));
        assert_eq!(methods[1]["type"], "Ed25519VerificationKey2020");
        assert!(methods[1]["id"].as_str().unwrap().ends_with("#recovery-key"));
        assert!(methods[1]["publicKeyMultibase"].as_str().unwrap().starts_with('z'));

        assert_eq!(doc["alsoKnownAs"][0], "did:key:z6Mk1234");

        let auth = doc["authentication"].as_array().unwrap();
        assert_eq!(auth[0], "#main-key");
        let assertion = doc["assertionMethod"].as_array().unwrap();
        assert_eq!(assertion[0], "#main-key");
    }

    #[test]
    fn did_web_document_empty_aka() {
        let doc = did_web_document(
            "example.com",
            "bob",
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
            Some(&[1u8; 32]),
            &[],
        );
        assert!(doc["alsoKnownAs"].is_null());
    }

    #[test]
    fn did_web_document_no_recovery_key() {
        let doc = did_web_document(
            "example.com",
            "carol",
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
            None,
            &[],
        );
        let methods = doc["verificationMethod"].as_array().unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0]["type"], "RsaVerificationKey2018");

        let contexts = doc["@context"].as_array().unwrap();
        assert_eq!(contexts.len(), 1);
    }

    #[test]
    fn hex_round_trip() {
        let data = [0xde, 0xad, 0xbe, 0xef, 0x42];
        let encoded = hex_encode(&data);
        assert_eq!(encoded, "deadbeef42");
        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn hex_decode_rejects_odd_length() {
        assert!(hex_decode("abc").is_err());
    }

    #[test]
    fn hex_decode_rejects_invalid_chars() {
        assert!(hex_decode("zzzz").is_err());
    }

    #[test]
    fn invalid_mnemonic_rejected() {
        assert!(mnemonic_to_private_key("not a valid mnemonic phrase").is_err());
    }

    #[test]
    fn base32_known_vector() {
        // RFC 4648 test vector
        assert_eq!(base32_encode_lower(b"foobar"), "mzxw6ytboi");
    }
}
