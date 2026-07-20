//! Content-addressed media: compute and verify SRI hashes for media files.
//!
//! Provides SHA-256 SRI (Subresource Integrity) hashing for media attachments.
//! The hash is stored in the `integrity` column of the `media` table, enabling
//! content-addressed verification and deduplication.
//!
//! # SRI Format
//!
//! Hashes use the SRI format: `sha256-{base64}` (e.g., `sha256-uU0nuZNNPgilLlLX2n2r+sSE7+N6U4DukIj3rOLvzek=`).
//! This matches the W3C Subresource Integrity spec and is widely understood.

use sha2::{Digest, Sha256};

/// Compute the SRI hash of a byte slice.
///
/// Returns a string in `sha256-{base64}` format.
pub fn compute_sri(data: &[u8]) -> String {
    use base64::Engine;
    let hash = Sha256::digest(data);
    let b64 = base64::engine::general_purpose::STANDARD.encode(hash);
    format!("sha256-{b64}")
}

/// Verify data against an expected SRI hash.
///
/// Returns `true` if the hash matches.
pub fn verify_sri(data: &[u8], expected: &str) -> bool {
    compute_sri(data) == expected
}

/// Check if two media files are identical by comparing their SRI hashes.
///
/// Useful for deduplication: if two uploads produce the same hash,
/// they're the same file and only need to be stored once.
pub fn is_duplicate(hash_a: &str, hash_b: &str) -> bool {
    hash_a == hash_b
}

/// Populate the `integrity` field of a [`super::media_db::MediaRow`] from file bytes.
///
/// Call this before `insert_media` to store the content hash.
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub fn set_media_integrity(row: &mut super::media_db::MediaRow, data: &[u8]) {
    row.integrity = Some(compute_sri(data));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_sri_deterministic() {
        let data = b"hello world";
        let h1 = compute_sri(data);
        let h2 = compute_sri(data);
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256-"));
    }

    #[test]
    fn compute_sri_known_vector() {
        // echo -n "hello world" | openssl dgst -sha256 -binary | base64
        // = uU0nuZNNPgilLlLX2n2r+sSE7+N6U4DukIj3rOLvzek=
        let hash = compute_sri(b"hello world");
        assert_eq!(hash, "sha256-uU0nuZNNPgilLlLX2n2r+sSE7+N6U4DukIj3rOLvzek=");
    }

    #[test]
    fn verify_sri_valid() {
        let data = b"test data for hashing";
        let hash = compute_sri(data);
        assert!(verify_sri(data, &hash));
    }

    #[test]
    fn verify_sri_invalid() {
        let hash = compute_sri(b"original");
        assert!(!verify_sri(b"tampered", &hash));
    }

    #[test]
    fn empty_data_has_valid_hash() {
        let hash = compute_sri(b"");
        assert!(hash.starts_with("sha256-"));
        assert!(verify_sri(b"", &hash));
    }

    #[test]
    fn is_duplicate_same() {
        let h1 = compute_sri(b"same content");
        let h2 = compute_sri(b"same content");
        assert!(is_duplicate(&h1, &h2));
    }

    #[test]
    fn is_duplicate_different() {
        let h1 = compute_sri(b"content a");
        let h2 = compute_sri(b"content b");
        assert!(!is_duplicate(&h1, &h2));
    }

    #[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
    #[test]
    fn set_media_integrity_populates_field() {
        let mut row = super::super::media_db::MediaRow {
            id: 1,
            user_id: "u1".into(),
            persona_id: "p1".into(),
            post_id: None,
            file_path: "test.png".into(),
            mime_type: "image/png".into(),
            file_size: 100,
            width: None,
            height: None,
            blurhash: None,
            integrity: None,
            description: String::new(),
            created_at: 1000,
        };

        assert!(row.integrity.is_none());
        set_media_integrity(&mut row, b"file content");
        assert!(row.integrity.is_some());
        assert!(row.integrity.as_ref().unwrap().starts_with("sha256-"));
    }
}
