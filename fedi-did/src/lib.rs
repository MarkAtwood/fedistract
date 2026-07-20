//! DID (Decentralized Identifier) utilities for fediverse servers.
//!
//! Provides `did:scid`, `did:key`, `did:web`, and `did:plc` generation, BIP-39
//! mnemonic recovery key management, and W3C DID document construction.
//!
//! # Usage
//!
//! ```
//! use fedi_did::{generate_recovery_keypair, ed25519_to_did_key, did_web, did_scid_genesis};
//!
//! let (private_key, public_key) = generate_recovery_keypair();
//! let did_key = ed25519_to_did_key(&public_key);
//! assert!(did_key.starts_with("did:key:z6Mk"));
//!
//! let did = did_web("example.com", "alice");
//! assert_eq!(did, "did:web:example.com:users:alice");
//!
//! let (scid, chain) = did_scid_genesis(
//!     &public_key,
//!     None,
//!     "https://example.com/users/alice",
//!     None,
//!     &private_key,
//! );
//! assert!(scid.starts_with("did:scid:sha256:"));
//! ```

use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum DidError {
    #[error("invalid recovery phrase: {0}")]
    InvalidMnemonic(String),
    #[error("invalid hex: {0}")]
    InvalidHex(String),
    #[error("invalid did:scid chain: {0}")]
    InvalidChain(String),
    #[error("signature verification failed")]
    SignatureVerification,
}

// ---------------------------------------------------------------------------
// Key generation
// ---------------------------------------------------------------------------

/// Generate an Ed25519 recovery keypair.
///
/// Returns `(private_key, public_key)`. The private key is wrapped in
/// [`zeroize::Zeroizing`] so it is zeroed from memory on drop.
pub fn generate_recovery_keypair() -> (zeroize::Zeroizing<[u8; 32]>, [u8; 32]) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (zeroize::Zeroizing::new(signing_key.to_bytes()), verifying_key.to_bytes())
}

/// Derive the Ed25519 public key from a private key.
pub fn ed25519_public_from_private(private_key: &[u8; 32]) -> [u8; 32] {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(private_key);
    signing_key.verifying_key().to_bytes()
}

// ---------------------------------------------------------------------------
// did:key
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// did:web
// ---------------------------------------------------------------------------

/// Derive `did:web` from domain and username.
///
/// Returns `did:web:{domain}:users:{username}`.
pub fn did_web(domain: &str, username: &str) -> String {
    format!("did:web:{domain}:users:{username}")
}

/// Build a W3C DID document for `did:web`.
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

// ---------------------------------------------------------------------------
// did:scid
// ---------------------------------------------------------------------------

/// Canonicalize a JSON value for deterministic hashing.
///
/// Uses sorted keys and compact encoding (no whitespace). This is a simplified
/// JCS (RFC 8785) suitable for the did:scid use case.
pub fn canonicalize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut pairs: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            pairs.sort_by_key(|(k, _)| *k);
            let entries: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), canonicalize(v)))
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        serde_json::Value::Array(arr) => {
            let entries: Vec<String> = arr.iter().map(|v| canonicalize(v)).collect();
            format!("[{}]", entries.join(","))
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}

/// Compute SHA-256 hash of a canonical JSON value.
///
/// Returns `"sha256:{hex}"`.
pub fn hash_canonical(value: &serde_json::Value) -> String {
    let canonical = canonicalize(value);
    let hash = Sha256::digest(canonical.as_bytes());
    format!("sha256:{}", hex::encode(hash))
}

/// Generate a did:scid genesis document and derive the DID from it.
///
/// The DID is `did:scid:sha256:{hex}` where the hex is the SHA-256 hash of
/// the canonical genesis document (before signature is added).
///
/// Returns `(did, signed_genesis_chain_json)`.
pub fn did_scid_genesis(
    signing_pub: &[u8; 32],
    recovery_pub: Option<&[u8; 32]>,
    actor_url: &str,
    fedsync_url: Option<&str>,
    signing_key: &[u8; 32],
) -> (String, serde_json::Value) {
    let signing_did_key = ed25519_to_did_key(signing_pub);

    let mut verification_methods = vec![serde_json::json!({
        "id": "#key-1",
        "type": "Ed25519VerificationKey2020",
        "publicKeyMultibase": multibase::encode(
            multibase::Base::Base58Btc,
            &ed25519_multicodec(signing_pub)
        )
    })];

    let mut authentication = vec![serde_json::json!("#key-1")];

    if let Some(rpk) = recovery_pub {
        verification_methods.push(serde_json::json!({
            "id": "#recovery",
            "type": "Ed25519VerificationKey2020",
            "publicKeyMultibase": multibase::encode(
                multibase::Base::Base58Btc,
                &ed25519_multicodec(rpk)
            )
        }));
        authentication.push(serde_json::json!("#recovery"));
    }

    let mut services = vec![serde_json::json!({
        "id": "#activitypub",
        "type": "ActivityPubActor",
        "serviceEndpoint": actor_url
    })];

    if let Some(fs_url) = fedsync_url {
        services.push(serde_json::json!({
            "id": "#fedsync",
            "type": "FedSyncEndpoint",
            "serviceEndpoint": fs_url
        }));
    }

    let mut also_known_as = vec![
        serde_json::Value::String(actor_url.to_string()),
        serde_json::Value::String(signing_did_key),
    ];
    if let Some(rpk) = recovery_pub {
        also_known_as.push(serde_json::Value::String(ed25519_to_did_key(rpk)));
    }

    // Build unsigned genesis entry (prev is null for genesis)
    let unsigned_entry = serde_json::json!({
        "version": 1,
        "verificationMethod": verification_methods,
        "authentication": authentication,
        "service": services,
        "alsoKnownAs": also_known_as,
        "prev": null
    });

    // Hash the unsigned entry to derive the DID
    let did_hash = hash_canonical(&unsigned_entry);
    let did = format!("did:scid:{did_hash}");

    // Sign the entry
    let signature = sign_entry(&unsigned_entry, signing_key);

    // Build the full chain document
    let chain = serde_json::json!({
        "did": did,
        "chain": [{
            "version": 1,
            "verificationMethod": verification_methods,
            "authentication": authentication,
            "service": services,
            "alsoKnownAs": also_known_as,
            "prev": null,
            "signature": signature
        }]
    });

    (did, chain)
}

/// Create a key rotation entry for an existing did:scid chain.
///
/// The new entry is signed with `current_signing_key` (which must correspond to
/// a key in the previous entry's `authentication`). The entry's `prev` field is
/// the hash of the previous chain entry.
///
/// Returns the new signed chain entry (caller appends to the chain array).
pub fn did_scid_rotate(
    prev_entry: &serde_json::Value,
    new_signing_pub: &[u8; 32],
    recovery_pub: Option<&[u8; 32]>,
    actor_url: &str,
    fedsync_url: Option<&str>,
    current_signing_key: &[u8; 32],
) -> serde_json::Value {
    let prev_hash = hash_canonical(prev_entry);

    let mut verification_methods = vec![serde_json::json!({
        "id": "#key-1",
        "type": "Ed25519VerificationKey2020",
        "publicKeyMultibase": multibase::encode(
            multibase::Base::Base58Btc,
            &ed25519_multicodec(new_signing_pub)
        )
    })];

    let mut authentication = vec![serde_json::json!("#key-1")];

    if let Some(rpk) = recovery_pub {
        verification_methods.push(serde_json::json!({
            "id": "#recovery",
            "type": "Ed25519VerificationKey2020",
            "publicKeyMultibase": multibase::encode(
                multibase::Base::Base58Btc,
                &ed25519_multicodec(rpk)
            )
        }));
        authentication.push(serde_json::json!("#recovery"));
    }

    let mut services = vec![serde_json::json!({
        "id": "#activitypub",
        "type": "ActivityPubActor",
        "serviceEndpoint": actor_url
    })];

    if let Some(fs_url) = fedsync_url {
        services.push(serde_json::json!({
            "id": "#fedsync",
            "type": "FedSyncEndpoint",
            "serviceEndpoint": fs_url
        }));
    }

    let unsigned_entry = serde_json::json!({
        "version": 1,
        "verificationMethod": verification_methods,
        "authentication": authentication,
        "service": services,
        "alsoKnownAs": [actor_url],
        "prev": prev_hash
    });

    let signature = sign_entry(&unsigned_entry, current_signing_key);

    serde_json::json!({
        "version": 1,
        "verificationMethod": verification_methods,
        "authentication": authentication,
        "service": services,
        "alsoKnownAs": [actor_url],
        "prev": prev_hash,
        "signature": signature
    })
}

/// Verify a did:scid chain.
///
/// Checks that:
/// 1. The genesis hash matches the DID
/// 2. Each entry's `prev` matches the hash of the previous entry
/// 3. Each entry's signature is valid against a key in the previous entry's authentication
///    (or the entry's own keys for genesis)
pub fn verify_did_scid_chain(
    did: &str,
    chain: &[serde_json::Value],
) -> Result<(), DidError> {
    if chain.is_empty() {
        return Err(DidError::InvalidChain("empty chain".into()));
    }

    // Verify genesis: hash of unsigned genesis must match DID
    let genesis = &chain[0];
    let unsigned_genesis = strip_signature(genesis);
    let genesis_hash = hash_canonical(&unsigned_genesis);
    let expected_did = format!("did:scid:{genesis_hash}");

    if did != expected_did {
        return Err(DidError::InvalidChain(format!(
            "DID mismatch: expected {expected_did}, got {did}"
        )));
    }

    // Verify genesis signature against its own keys
    verify_entry_signature(genesis, genesis)?;

    // Verify each subsequent entry
    for i in 1..chain.len() {
        let entry = &chain[i];
        let prev_entry = &chain[i - 1];

        // Check prev hash (hash of the full signed previous entry)
        let expected_prev = hash_canonical(prev_entry);
        let actual_prev = entry["prev"].as_str().unwrap_or("");
        if actual_prev != expected_prev {
            return Err(DidError::InvalidChain(format!(
                "prev mismatch at entry {i}: expected {expected_prev}, got {actual_prev}"
            )));
        }

        // Verify signature against PREVIOUS entry's keys
        verify_entry_signature(entry, prev_entry)?;
    }

    Ok(())
}

/// Sign a chain entry with an Ed25519 key.
fn sign_entry(unsigned_entry: &serde_json::Value, private_key: &[u8; 32]) -> String {
    use ed25519_dalek::{Signer, SigningKey};

    let canonical = canonicalize(unsigned_entry);
    let signing_key = SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(canonical.as_bytes());
    format!("ed25519:{}", hex::encode(signature.to_bytes()))
}

/// Verify an entry's signature against the keys in `key_source` entry.
fn verify_entry_signature(
    entry: &serde_json::Value,
    key_source: &serde_json::Value,
) -> Result<(), DidError> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let sig_str = entry["signature"]
        .as_str()
        .ok_or_else(|| DidError::InvalidChain("missing signature".into()))?;

    let sig_hex = sig_str
        .strip_prefix("ed25519:")
        .ok_or_else(|| DidError::InvalidChain("signature must start with ed25519:".into()))?;

    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| DidError::InvalidChain(format!("invalid signature hex: {e}")))?;

    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| DidError::InvalidChain("invalid signature length".into()))?;

    let unsigned = strip_signature(entry);
    let canonical = canonicalize(&unsigned);

    // Try each verification method in key_source
    let methods = key_source["verificationMethod"]
        .as_array()
        .ok_or_else(|| DidError::InvalidChain("missing verificationMethod".into()))?;

    for method in methods {
        if method["type"] != "Ed25519VerificationKey2020" {
            continue;
        }
        let multibase_str = match method["publicKeyMultibase"].as_str() {
            Some(s) => s,
            None => continue,
        };
        let (_, decoded) = multibase::decode(multibase_str)
            .map_err(|e| DidError::InvalidChain(format!("invalid multibase: {e}")))?;

        // Strip multicodec prefix (0xed01)
        if decoded.len() != 34 || decoded[0] != 0xed || decoded[1] != 0x01 {
            continue;
        }
        let pub_bytes: [u8; 32] = decoded[2..34]
            .try_into()
            .map_err(|_| DidError::InvalidChain("invalid key length".into()))?;

        let verifying_key = VerifyingKey::from_bytes(&pub_bytes)
            .map_err(|_| DidError::InvalidChain("invalid public key".into()))?;

        if verifying_key.verify(canonical.as_bytes(), &signature).is_ok() {
            return Ok(());
        }
    }

    Err(DidError::SignatureVerification)
}

/// Strip the "signature" field from a chain entry for hashing/verification.
fn strip_signature(entry: &serde_json::Value) -> serde_json::Value {
    let mut obj = entry.clone();
    if let Some(map) = obj.as_object_mut() {
        map.remove("signature");
    }
    obj
}

/// Build the multicodec-prefixed Ed25519 public key bytes.
fn ed25519_multicodec(public_key: &[u8; 32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(34);
    buf.push(0xed);
    buf.push(0x01);
    buf.extend_from_slice(public_key);
    buf
}

// ---------------------------------------------------------------------------
// did:plc
// ---------------------------------------------------------------------------

/// Generate a `did:plc` from recovery and signing public keys.
///
/// Computes SHA-256 of a canonical genesis operation, then base32-encodes
/// the first 15 bytes (lowercase, no padding) to produce the identifier.
pub fn generate_did_plc(
    recovery_pub: &[u8; 32],
    signing_pub: &[u8; 32],
) -> String {
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

// ---------------------------------------------------------------------------
// BIP-39 mnemonic
// ---------------------------------------------------------------------------

/// Encode recovery private key as BIP-39 24-word mnemonic.
pub fn private_key_to_mnemonic(private_key: &[u8; 32]) -> String {
    let mnemonic =
        bip39::Mnemonic::from_entropy(private_key).expect("32 bytes is valid BIP-39 entropy");
    mnemonic.to_string()
}

/// Decode BIP-39 mnemonic back to private key bytes.
///
/// The returned key is wrapped in [`zeroize::Zeroizing`] so it is zeroed
/// from memory on drop.
pub fn mnemonic_to_private_key(phrase: &str) -> Result<zeroize::Zeroizing<[u8; 32]>, DidError> {
    use zeroize::Zeroize;

    let mnemonic = bip39::Mnemonic::parse(phrase)
        .map_err(|e| DidError::InvalidMnemonic(e.to_string()))?;
    let mut entropy = mnemonic.to_entropy();
    if entropy.len() != 32 {
        entropy.zeroize();
        return Err(DidError::InvalidMnemonic(
            "recovery phrase must encode exactly 32 bytes (24 words)".into(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&entropy);
    entropy.zeroize();
    Ok(zeroize::Zeroizing::new(key))
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Hex-encode bytes for storage.
pub fn hex_encode(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Hex-decode a string back to bytes.
pub fn hex_decode(s: &str) -> Result<Vec<u8>, DidError> {
    hex::decode(s).map_err(|e| DidError::InvalidHex(e.to_string()))
}

/// Base32 lowercase encoding without padding (RFC 4648 variant).
fn base32_encode_lower(data: &[u8]) -> String {
    data_encoding::BASE32_NOPAD.encode(data).to_lowercase()
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
        assert_eq!(*recovered, *private_key);

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
    fn did_scid_genesis_format() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (did, chain) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );
        assert!(did.starts_with("did:scid:sha256:"));
        assert!(chain["chain"].as_array().unwrap().len() == 1);
        assert_eq!(chain["did"], did);
    }

    #[test]
    fn did_scid_deterministic() {
        let signing_key = [42u8; 32];
        let signing_pub = ed25519_public_from_private(&signing_key);
        let (a, _) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );
        let (b, _) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn did_scid_with_recovery_key() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (_, recovery_pub) = generate_recovery_keypair();
        let (did, chain) = did_scid_genesis(
            &signing_pub,
            Some(&recovery_pub),
            "https://example.com/users/alice",
            Some("https://example.com/fedsync/users/alice"),
            &signing_key,
        );
        assert!(did.starts_with("did:scid:sha256:"));

        let entry = &chain["chain"][0];
        let methods = entry["verificationMethod"].as_array().unwrap();
        assert_eq!(methods.len(), 2);
        let auth = entry["authentication"].as_array().unwrap();
        assert_eq!(auth.len(), 2);
        let services = entry["service"].as_array().unwrap();
        assert_eq!(services.len(), 2);
    }

    #[test]
    fn did_scid_verify_genesis() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (did, chain) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );
        let chain_array = chain["chain"].as_array().unwrap();
        verify_did_scid_chain(&did, chain_array).unwrap();
    }

    #[test]
    fn did_scid_verify_with_rotation() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (did, chain) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );

        // Rotate to new key
        let (new_signing_key, new_signing_pub) = generate_recovery_keypair();
        let genesis_entry = &chain["chain"][0];
        let rotation_entry = did_scid_rotate(
            genesis_entry,
            &new_signing_pub,
            None,
            "https://new.example.com/users/alice",
            None,
            &signing_key, // sign with current key
        );

        let full_chain = vec![genesis_entry.clone(), rotation_entry.clone()];
        verify_did_scid_chain(&did, &full_chain).unwrap();

        // Verify new key can sign next rotation
        let (_, next_pub) = generate_recovery_keypair();
        let next_entry = did_scid_rotate(
            &rotation_entry,
            &next_pub,
            None,
            "https://new.example.com/users/alice",
            None,
            &new_signing_key,
        );
        let full_chain = vec![genesis_entry.clone(), rotation_entry, next_entry];
        verify_did_scid_chain(&did, &full_chain).unwrap();
    }

    #[test]
    fn did_scid_recovery_key_can_rotate() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (recovery_key, recovery_pub) = generate_recovery_keypair();
        let (did, chain) = did_scid_genesis(
            &signing_pub,
            Some(&recovery_pub),
            "https://example.com/users/alice",
            None,
            &signing_key,
        );

        // Simulate server death: rotate using recovery key
        let (_, new_pub) = generate_recovery_keypair();
        let genesis_entry = &chain["chain"][0];
        let rotation_entry = did_scid_rotate(
            genesis_entry,
            &new_pub,
            Some(&recovery_pub),
            "https://new-server.example/users/alice",
            None,
            &recovery_key, // sign with recovery key
        );

        let full_chain = vec![genesis_entry.clone(), rotation_entry];
        verify_did_scid_chain(&did, &full_chain).unwrap();
    }

    #[test]
    fn did_scid_rejects_tampered_chain() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (did, chain) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );

        // Tamper with the chain
        let mut tampered = chain["chain"][0].clone();
        tampered["service"][0]["serviceEndpoint"] =
            serde_json::Value::String("https://evil.com".into());

        let result = verify_did_scid_chain(&did, &[tampered]);
        assert!(result.is_err());
    }

    #[test]
    fn did_scid_rejects_wrong_did() {
        let (signing_key, signing_pub) = generate_recovery_keypair();
        let (_, chain) = did_scid_genesis(
            &signing_pub,
            None,
            "https://example.com/users/alice",
            None,
            &signing_key,
        );
        let chain_array = chain["chain"].as_array().unwrap();

        let result = verify_did_scid_chain("did:scid:sha256:0000000000", chain_array);
        assert!(result.is_err());
    }

    #[test]
    fn canonicalize_sorts_keys() {
        let value = serde_json::json!({"z": 1, "a": 2, "m": 3});
        let canonical = canonicalize(&value);
        assert_eq!(canonical, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn canonicalize_nested() {
        let value = serde_json::json!({"b": {"z": 1, "a": 2}, "a": [3, 2, 1]});
        let canonical = canonicalize(&value);
        assert_eq!(canonical, r#"{"a":[3,2,1],"b":{"a":2,"z":1}}"#);
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
