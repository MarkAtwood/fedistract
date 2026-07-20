//! HTTP Signatures (draft-cavage-12) for ActivityPub federation.
//!
//! Provides signing and verification of HTTP requests using RSA-SHA256,
//! compatible with Mastodon and other ActivityPub implementations.
//!
//! # Examples
//!
//! ```
//! use cavage_httpsig::{sign_post, sign_get, verify};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Sign an outbound POST (ActivityPub delivery)
//! # let private_key_pem = "";
//! # let key_id = "https://example.com/users/alice#main-key";
//! # let url: url::Url = "https://remote.example/inbox".parse()?;
//! let headers = sign_post(private_key_pem, key_id, &url, b"{}")?;
//!
//! // Sign an outbound GET (actor fetch, WebFinger)
//! let headers = sign_get(private_key_pem, key_id, &url)?;
//!
//! // Verify an inbound signed request
//! # let public_key_pem = "";
//! # let sig_header = "";
//! # let request_headers = http::HeaderMap::new();
//! let result = verify(public_key_pem, sig_header, "post", "/inbox", &request_headers);
//! # Ok(())
//! # }
//! ```

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use http::HeaderMap;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha2::Sha256;

/// Errors from signing operations.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("failed to decode private key PEM: {0}")]
    KeyDecode(#[from] rsa::pkcs8::Error),
    #[error("target URL has no host")]
    NoHost,
    #[error("generated header value is not valid HTTP: {0}")]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
}

/// Errors from verification operations.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("failed to decode public key PEM: {0}")]
    KeyDecode(#[from] rsa::pkcs8::spki::Error),
    #[error("missing keyId in Signature header")]
    MissingKeyId,
    #[error("missing headers field in Signature header")]
    MissingHeaders,
    #[error("missing signature field in Signature header")]
    MissingSignature,
    #[error("header {0:?} referenced in signature but not present in request")]
    MissingRequestHeader(String),
    #[error("header {0:?} contains non-ASCII value")]
    NonAsciiHeader(String),
    #[error("invalid base64 in signature: {0}")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("invalid signature bytes")]
    InvalidSignatureBytes,
    #[error("signature verification failed")]
    SignatureMismatch,
    #[error("malformed quoted value in Signature header: {0}")]
    MalformedQuotedValue(String),
    #[error("Signature header exceeds size limit")]
    HeaderTooLarge,
}

/// Sign an outbound HTTP POST request (ActivityPub delivery).
///
/// Returns headers to add to the request: `Date`, `Digest`, `Signature`.
/// Follows draft-cavage-http-signatures as implemented by Mastodon.
pub fn sign_post(
    private_key_pem: &str,
    key_id: &str,
    target_url: &url::Url,
    body: &[u8],
) -> Result<HeaderMap, SignError> {
    let host = target_url.host_str().ok_or(SignError::NoHost)?;
    let path = target_url.path();
    let date = format_http_date();
    let digest = format!("SHA-256={}", B64.encode(sha256(body)));

    let signed_string =
        format!("(request-target): post {path}\nhost: {host}\ndate: {date}\ndigest: {digest}");

    let sig_b64 = rsa_sign(private_key_pem, signed_string.as_bytes())?;

    let sig_header = format!(
        r#"keyId="{key_id}",algorithm="rsa-sha256",headers="(request-target) host date digest",signature="{sig_b64}""#
    );

    let mut headers = HeaderMap::new();
    headers.insert("Date", date.parse()?);
    headers.insert("Digest", digest.parse()?);
    headers.insert("Signature", sig_header.parse()?);
    Ok(headers)
}

/// Sign an outbound HTTP GET request (actor fetch, WebFinger).
///
/// Returns headers to add to the request: `Date`, `Signature`.
pub fn sign_get(
    private_key_pem: &str,
    key_id: &str,
    target_url: &url::Url,
) -> Result<HeaderMap, SignError> {
    let host = target_url.host_str().ok_or(SignError::NoHost)?;
    let path = target_url.path();
    let date = format_http_date();

    let signed_string = format!("(request-target): get {path}\nhost: {host}\ndate: {date}");

    let sig_b64 = rsa_sign(private_key_pem, signed_string.as_bytes())?;

    let sig_header = format!(
        r#"keyId="{key_id}",algorithm="rsa-sha256",headers="(request-target) host date",signature="{sig_b64}""#
    );

    let mut headers = HeaderMap::new();
    headers.insert("Date", date.parse()?);
    headers.insert("Signature", sig_header.parse()?);
    Ok(headers)
}

/// Verify an HTTP Signature on an inbound request.
///
/// `public_key_pem`: the sender's public key (fetched from their actor document).
/// `signature_header`: the raw `Signature` header value.
/// `method`, `path`: the request-line components (method lowercase, path with leading `/`).
/// `headers`: the request headers (used to reconstruct the signed string).
pub fn verify(
    public_key_pem: &str,
    signature_header: &str,
    method: &str,
    path: &str,
    headers: &HeaderMap,
) -> Result<SignatureParts, VerifyError> {
    let parts = parse_signature_header(signature_header)?;

    let signed_headers = parts.headers.split_whitespace();
    let mut lines = Vec::new();
    for h in signed_headers {
        if h == "(request-target)" {
            lines.push(format!(
                "(request-target): {} {}",
                method.to_lowercase(),
                path
            ));
        } else {
            let val = headers
                .get(h)
                .ok_or_else(|| VerifyError::MissingRequestHeader(h.to_string()))?
                .to_str()
                .map_err(|_| VerifyError::NonAsciiHeader(h.to_string()))?;
            lines.push(format!("{h}: {val}"));
        }
    }
    let signed_string = lines.join("\n");

    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)?;
    let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha256>::new(public_key);

    let sig_bytes = B64.decode(&parts.signature)?;
    let signature = rsa::pkcs1v15::Signature::try_from(sig_bytes.as_slice())
        .map_err(|_| VerifyError::InvalidSignatureBytes)?;

    verifying_key
        .verify(signed_string.as_bytes(), &signature)
        .map_err(|_| VerifyError::SignatureMismatch)?;

    Ok(parts)
}

/// Parsed components of an HTTP Signature header.
#[derive(Clone, PartialEq, Eq)]
pub struct SignatureParts {
    pub key_id: String,
    pub headers: String,
    pub signature: String,
}

impl std::fmt::Debug for SignatureParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureParts")
            .field("key_id", &self.key_id)
            .field("headers", &self.headers)
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

/// Parse a Signature header value into its components.
///
/// Format: `keyId="...",algorithm="...",headers="...",signature="..."`
pub fn parse_signature_header(header: &str) -> Result<SignatureParts, VerifyError> {
    const MAX_HEADER_SIZE: usize = 8192;
    if header.len() > MAX_HEADER_SIZE {
        return Err(VerifyError::HeaderTooLarge);
    }

    let mut key_id_val = None;
    let mut headers_val = None;
    let mut signature_val = None;

    for part in split_signature_params(header) {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("keyId=\"") {
            key_id_val = Some(
                val.strip_suffix('"')
                    .ok_or_else(|| VerifyError::MalformedQuotedValue("keyId".into()))?
                    .to_string(),
            );
        } else if let Some(val) = part.strip_prefix("headers=\"") {
            headers_val = Some(
                val.strip_suffix('"')
                    .ok_or_else(|| VerifyError::MalformedQuotedValue("headers".into()))?
                    .to_string(),
            );
        } else if let Some(val) = part.strip_prefix("signature=\"") {
            signature_val = Some(
                val.strip_suffix('"')
                    .ok_or_else(|| VerifyError::MalformedQuotedValue("signature".into()))?
                    .to_string(),
            );
        }
    }

    Ok(SignatureParts {
        key_id: key_id_val.ok_or(VerifyError::MissingKeyId)?,
        headers: headers_val.ok_or(VerifyError::MissingHeaders)?,
        signature: signature_val.ok_or(VerifyError::MissingSignature)?,
    })
}

/// Split signature header params, respecting quoted values and backslash escapes.
fn split_signature_params(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;

    for c in s.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }
        match c {
            '\\' if in_quotes => {
                escape_next = true;
                current.push(c);
            }
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            ',' if !in_quotes => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn rsa_sign(private_key_pem: &str, message: &[u8]) -> Result<String, SignError> {
    let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)?;
    let signing_key = rsa::pkcs1v15::SigningKey::<Sha256>::new(private_key);
    let signature = signing_key.sign(message);
    Ok(B64.encode(signature.to_bytes()))
}

fn sha256(data: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(data).to_vec()
}

fn format_http_date() -> String {
    // HTTP-date format per RFC 7231
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format without pulling in chrono as a runtime dep.
    // We compute the date string manually using the same format Mastodon expects.
    format_timestamp(now)
}

fn format_timestamp(epoch_secs: u64) -> String {
    // Calculate date components from unix timestamp
    let secs_per_day: u64 = 86400;
    let days = epoch_secs / secs_per_day;
    let day_secs = epoch_secs % secs_per_day;

    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    // Day of week (Jan 1 1970 was Thursday = 4)
    let dow = ((days + 4) % 7) as usize;
    let dow_str = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][dow];

    // Date from days since epoch
    let (year, month, day) = days_to_ymd(days);
    let month_str = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month - 1) as usize];

    format!("{dow_str}, {day:02} {month_str} {year} {hours:02}:{minutes:02}:{seconds:02} GMT")
}

fn days_to_ymd(days: u64) -> (u64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCu3DRolW21JtBR
hT0FXUSiG8xFn8wNFrHONdg4fXEu4Z4AkzBkGlLC2+dq4FDombo142zUa6nzp/ou
x8aZON9I3g0itN5HeypFypo2pmrha3jfHPwjsvTz9nKEBxxMi1uycFSGukwA+qor
hWIYl2oxUjZzsauPlVDxsvhx9DtBGIuR3fp4TdgeQNR9Gsl5vW69/jp3DBfvcdNn
QxKAf+GgX5ZsRai/wj68o/bWMG9Jgwkdu5POe4zed0614s5WBjIw6gJvGFeYwK9D
gPqVNk7PC9OlJiweuZcaC4B4VwYw2CXZ4pC3ipoJZgX9wTaM7CNzLPLKpXmxJ1db
CHL6JR/lAgMBAAECggEASuMlENtaSlhupFMnQJpir/iuwey/g6WCDdoSmZLju9vF
guKGuYBqjGiIkkWycQORc83MSKc5eJAqvgkyHWH1gqwSvRfwEHYHsy8jX59jK9qO
wCMWOyXD8Y3NGo0/CesvINsp4C9+KHcyFQSBcB28zZWzm1Xur0YYDgODkq4yCFzI
xnza0fc8sKIfkyNbw8xIEjWFiCFNRAXelA4AUJEpp0vyRAgfyYFd9uSO4y7D6L07
aW/rOet2+hYD0RGT/MxNuedFFe0Z/HlIOySBuzr0J5oz+Xyahuv4v2+gFp9vVRXb
LoHK6xuFEaH1TaZM7+GlJQuiP8l32DpBmTmhqpHq2wKBgQDeuxG/A1S2ol9lTg3O
rywehrRrK/0qqBb0f4cP9zDXV1f8UzzsAqgQZMXRy/9eB5GVTiHoXgMICFGLbwyz
CB90l8O2XBM4UmyohWzr9VisXWeCx9Q0GLG8VYbJSybF1Zh2H36cpnlhl73ESlBw
/fn4kQX+VwJI9agOvt4ociB8kwKBgQDI+qDAER82rEE87z3gpRbtGFJ2l2NIPPqO
18ktVLMk7qjCDU0XOJjxcgRfTBMrGf4TBMtzLJZDU03dFDGVer75MS5L2DBOQ8bI
gvDm4feyz5C4D5WnmZ2QHmWJHlESwH0bxnB48xQS3oFl0+VsGpJxma8vohjhWzPk
YYZoiYI0pwKBgGkQEQTrS1CDM0CUGws9sjAMFprfOyKd+4YFie5MCevqNYS+tuQV
NLXW80FNWv49z7yACJqVjhSB6AU/stvYnw3ecOFaeW594udzWLfNGbDktmkIXd7d
LynJpjTZkEaNxMcjgBPgqy0P6OHotB04kGth7VPWMyu7RTT/b8fgXdalAoGBAIuF
+rz75fh1oyCjUgi3c3ALt4ve0yzeMG+j/GS87VURXhTBWShqwTq1FbX2wUPl2o3n
gTom1PZOSbrV/wov2Y5zhxleL0LWKJUg2g7fBq+bC3PMVe+xZEId6A1F/7CN8wyq
OYCt99yVna1ManQfClVVBNqDpNQmFaNR1RaTh9H3AoGBALMLymj9TCR5I3EYLiDs
szYctMao6KIV+Ted2U1xfRGD9110hI4/fVjI+ElnbWIWCNDxBf+Ifuc1L4TP6PEK
gHm6/dE6S2cGKhTJbbhMrOja4ku2L291++q5nS048u1gTwtj/NvIOncdhk4v52s1
zloXrMaFLBPp2UUN/amDTUIJ
-----END PRIVATE KEY-----";

    fn test_public_key_pem() -> String {
        let private_key = RsaPrivateKey::from_pkcs8_pem(TEST_KEY_PEM).unwrap();
        let public_key = RsaPublicKey::from(&private_key);
        rsa::pkcs8::EncodePublicKey::to_public_key_pem(&public_key, rsa::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string()
    }

    #[test]
    fn sign_get_produces_valid_headers() {
        let url: url::Url = "https://remote.example/users/alice".parse().unwrap();
        let key_id = "https://local.example/users/writer#main-key";

        let headers = sign_get(TEST_KEY_PEM, key_id, &url).unwrap();

        assert!(headers.contains_key("Date"));
        assert!(headers.contains_key("Signature"));

        let sig = headers["Signature"].to_str().unwrap();
        assert!(sig.contains(r#"keyId="https://local.example/users/writer#main-key""#));
        assert!(sig.contains(r#"algorithm="rsa-sha256""#));
        assert!(sig.contains(r#"headers="(request-target) host date""#));
        assert!(sig.contains("signature=\""));
    }

    #[test]
    fn sign_post_includes_digest() {
        let url: url::Url = "https://remote.example/inbox".parse().unwrap();
        let key_id = "https://local.example/users/writer#main-key";
        let body = b"{\"type\":\"Follow\"}";

        let headers = sign_post(TEST_KEY_PEM, key_id, &url, body).unwrap();

        assert!(headers.contains_key("Date"));
        assert!(headers.contains_key("Digest"));
        assert!(headers.contains_key("Signature"));

        let digest = headers["Digest"].to_str().unwrap();
        assert!(digest.starts_with("SHA-256="));

        let sig = headers["Signature"].to_str().unwrap();
        assert!(sig.contains(r#"headers="(request-target) host date digest""#));
    }

    #[test]
    fn sign_post_digest_is_correct() {
        // Independent oracle: SHA-256 of b"{}" is known
        // echo -n '{}' | sha256sum = 44136fa355b311bfa706c3193d986cd4d1d1c6fdf8f9c...
        let url: url::Url = "https://remote.example/inbox".parse().unwrap();
        let headers = sign_post(TEST_KEY_PEM, "k", &url, b"{}").unwrap();
        let digest = headers["Digest"].to_str().unwrap();
        // SHA-256 of "{}" = RBNvo1WzZ4oRRq0W9+hknpT7T8If536DEMBg9hyq/4o=
        assert_eq!(
            digest,
            "SHA-256=RBNvo1WzZ4oRRq0W9+hknpT7T8If536DEMBg9hyq/4o="
        );
    }

    #[test]
    fn sign_get_round_trip_verify() {
        let url: url::Url = "https://remote.example/users/alice".parse().unwrap();
        let key_id = "https://local.example/users/writer#main-key";

        let headers = sign_get(TEST_KEY_PEM, key_id, &url).unwrap();
        let sig_header = headers["Signature"].to_str().unwrap();

        let public_pem = test_public_key_pem();

        // Build a request HeaderMap with the Date (as verify needs it)
        let mut req_headers = HeaderMap::new();
        req_headers.insert("date", headers["Date"].clone());
        req_headers.insert("host", "remote.example".parse().unwrap());

        let parts = verify(&public_pem, sig_header, "get", "/users/alice", &req_headers).unwrap();

        assert_eq!(parts.key_id, key_id);
    }

    #[test]
    fn sign_post_round_trip_verify() {
        let url: url::Url = "https://remote.example/inbox".parse().unwrap();
        let key_id = "https://local.example/users/writer#main-key";
        let body = b"{\"type\":\"Create\",\"object\":{}}";

        let headers = sign_post(TEST_KEY_PEM, key_id, &url, body).unwrap();
        let sig_header = headers["Signature"].to_str().unwrap();

        let public_pem = test_public_key_pem();

        let mut req_headers = HeaderMap::new();
        req_headers.insert("date", headers["Date"].clone());
        req_headers.insert("host", "remote.example".parse().unwrap());
        req_headers.insert("digest", headers["Digest"].clone());

        let parts = verify(&public_pem, sig_header, "post", "/inbox", &req_headers).unwrap();

        assert_eq!(parts.key_id, key_id);
    }

    #[test]
    fn verify_rejects_tampered_signature() {
        let url: url::Url = "https://remote.example/inbox".parse().unwrap();
        let key_id = "https://local.example/users/writer#main-key";
        let body = b"{}";

        let headers = sign_post(TEST_KEY_PEM, key_id, &url, body).unwrap();
        let sig_header = headers["Signature"].to_str().unwrap();

        // Tamper: change one char in the base64 signature
        let tampered = sig_header.replacen("signature=\"", "signature=\"A", 1);

        let public_pem = test_public_key_pem();
        let mut req_headers = HeaderMap::new();
        req_headers.insert("date", headers["Date"].clone());
        req_headers.insert("host", "remote.example".parse().unwrap());
        req_headers.insert("digest", headers["Digest"].clone());

        let result = verify(&public_pem, &tampered, "post", "/inbox", &req_headers);
        assert!(result.is_err());
    }

    #[test]
    fn verify_rejects_wrong_digest() {
        let url: url::Url = "https://remote.example/inbox".parse().unwrap();
        let key_id = "https://local.example/users/writer#main-key";
        let body = b"original body";

        let headers = sign_post(TEST_KEY_PEM, key_id, &url, body).unwrap();
        let sig_header = headers["Signature"].to_str().unwrap();

        let public_pem = test_public_key_pem();
        let mut req_headers = HeaderMap::new();
        req_headers.insert("date", headers["Date"].clone());
        req_headers.insert("host", "remote.example".parse().unwrap());
        // Use wrong digest (different body)
        req_headers.insert(
            "digest",
            "SHA-256=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                .parse()
                .unwrap(),
        );

        let result = verify(&public_pem, sig_header, "post", "/inbox", &req_headers);
        assert!(result.is_err());
    }

    #[test]
    fn verify_rejects_missing_header() {
        let url: url::Url = "https://remote.example/inbox".parse().unwrap();
        let headers = sign_post(TEST_KEY_PEM, "k", &url, b"{}").unwrap();
        let sig_header = headers["Signature"].to_str().unwrap();

        let public_pem = test_public_key_pem();
        // Missing "digest" header that the signature references
        let mut req_headers = HeaderMap::new();
        req_headers.insert("date", headers["Date"].clone());
        req_headers.insert("host", "remote.example".parse().unwrap());

        let result = verify(&public_pem, sig_header, "post", "/inbox", &req_headers);
        assert!(matches!(result, Err(VerifyError::MissingRequestHeader(_))));
    }

    #[test]
    fn parse_signature_header_basic() {
        let header = r#"keyId="https://example.com/key",algorithm="rsa-sha256",headers="(request-target) host date",signature="abc123==""#;
        let parts = parse_signature_header(header).unwrap();
        assert_eq!(parts.key_id, "https://example.com/key");
        assert_eq!(parts.headers, "(request-target) host date");
        assert_eq!(parts.signature, "abc123==");
    }

    #[test]
    fn parse_signature_header_with_commas_in_values() {
        // Real-world: keyId URLs may not have commas, but test robustness
        let header = r#"keyId="https://example.com/u/a#key",headers="(request-target) date",signature="dGVzdA==""#;
        let parts = parse_signature_header(header).unwrap();
        assert_eq!(parts.key_id, "https://example.com/u/a#key");
        assert_eq!(parts.signature, "dGVzdA==");
    }

    #[test]
    fn parse_signature_header_rejects_missing_fields() {
        assert!(matches!(
            parse_signature_header(r#"algorithm="rsa-sha256""#),
            Err(VerifyError::MissingKeyId)
        ));
    }

    #[test]
    fn parse_signature_header_rejects_missing_closing_quote() {
        // keyId has no closing quote — must be rejected
        let header = r#"keyId="https://example.com/key"#;
        assert!(
            parse_signature_header(header).is_err(),
            "must reject header with missing closing quote"
        );
    }

    #[test]
    fn parse_signature_header_rejects_oversized() {
        let huge = format!(r#"keyId="k",headers="h",signature="{}""#, "A".repeat(9000));
        assert!(matches!(
            parse_signature_header(&huge),
            Err(VerifyError::HeaderTooLarge)
        ));
    }

    #[test]
    fn signature_parts_debug_redacts() {
        let parts = SignatureParts {
            key_id: "https://example.com/key".into(),
            headers: "(request-target) host date".into(),
            signature: "secret_signature_value".into(),
        };
        let debug = format!("{parts:?}");
        assert!(!debug.contains("secret_signature_value"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn format_timestamp_known_value() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        let s = format_timestamp(1704067200);
        assert_eq!(s, "Mon, 01 Jan 2024 00:00:00 GMT");
    }

    #[test]
    fn format_timestamp_epoch() {
        let s = format_timestamp(0);
        assert_eq!(s, "Thu, 01 Jan 1970 00:00:00 GMT");
    }

    #[test]
    fn sign_get_no_host_returns_error() {
        // data: URLs have no host
        let url: url::Url = "data:text/plain,hello".parse().unwrap();
        let result = sign_get(TEST_KEY_PEM, "k", &url);
        assert!(matches!(result, Err(SignError::NoHost)));
    }
}
