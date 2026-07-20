//! SSRF protection for outbound HTTP requests.
//!
//! Validates URLs and IP addresses against private, internal, and reserved
//! ranges before making HTTP requests. Covers IPv4, IPv6, IPv4-mapped IPv6,
//! CGNAT, and known private hostnames.
//!
//! # Usage
//!
//! ```
//! use ssrf_guard::validate_url;
//! use url::Url;
//!
//! let url = Url::parse("https://example.com/api").unwrap();
//! assert!(validate_url(&url).is_ok());
//!
//! let url = Url::parse("https://192.168.1.1/admin").unwrap();
//! assert!(validate_url(&url).is_err());
//! ```
//!
//! # Features
//!
//! - **`resolve`** — Enables `is_private_host_resolved()` for async DNS
//!   resolution with fail-closed semantics (requires `tokio`).
//! - **`reqwest`** — Enables `SsrfSafeResolver`, a custom DNS resolver for
//!   `reqwest::Client` that blocks private IPs at connect time, eliminating
//!   DNS rebinding TOCTOU (requires `reqwest` + `tokio`).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, thiserror::Error)]
pub enum SsrfError {
    #[error("URL has no host")]
    NoHost,
    #[error("refusing to connect to private/internal host: {0}")]
    PrivateHost(String),
    #[error("refusing non-HTTPS URL: {0}")]
    NotHttps(String),
}

/// Check whether an IPv4 address belongs to a private, reserved, or internal range.
///
/// Blocked ranges: loopback (127.0.0.0/8), RFC 1918 (10/8, 172.16/12, 192.168/16),
/// link-local (169.254/16), CGNAT (100.64/10), 0.0.0.0/8, broadcast, unspecified.
pub fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.octets()[0] == 0 // 0.0.0.0/8
        || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64) // CGNAT 100.64.0.0/10
}

/// Check whether an IPv6 address belongs to a private, reserved, or internal range.
///
/// Blocked ranges: loopback (::1), unspecified (::), ULA (fc00::/7),
/// link-local (fe80::/10), and IPv4-mapped addresses (::ffff:0:0/96) whose
/// embedded IPv4 address is private.
pub fn is_private_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || (ip.segments()[0] & 0xFE00) == 0xFC00 // ULA fc00::/7
        || (ip.segments()[0] & 0xFFC0) == 0xFE80 // link-local fe80::/10
        || ip
            .to_ipv4_mapped()
            .is_some_and(|v4| is_private_ipv4(v4))
}

/// Check whether an IP address (v4 or v6) is private/internal/reserved.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_ipv4(v4),
        IpAddr::V6(v6) => is_private_ipv6(v6),
    }
}

/// Check whether a hostname string is a known private/internal name or IP literal.
///
/// Matches `localhost`, `localhost.localdomain`, `*.local`, and `*.internal` hostnames.
/// If the host parses as an IP address, delegates to [`is_private_ip`].
pub fn is_private_host(host: &str) -> bool {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_ip(ip);
    }
    matches!(host, "localhost" | "localhost.localdomain")
        || host.ends_with(".local")
        || host.ends_with(".internal")
}

/// Validate that a URL is safe for outbound fetch.
///
/// Rejects URLs with no host, private/internal hosts, and non-HTTPS schemes.
pub fn validate_url(url: &url::Url) -> Result<(), SsrfError> {
    let host = url.host_str().ok_or(SsrfError::NoHost)?;
    if is_private_host(host) {
        return Err(SsrfError::PrivateHost(host.to_string()));
    }
    if url.scheme() != "https" {
        return Err(SsrfError::NotHttps(url.to_string()));
    }
    Ok(())
}

// ponytail: DNS rebinding TOCTOU — is_private_host checks the hostname string
// but can't prevent a public hostname from resolving to a private IP at connect
// time. The `resolve` and `reqwest` features below close this gap.

/// Check whether a hostname resolves to private/internal addresses via DNS.
///
/// Fails closed: returns `true` (blocked) if DNS resolution fails.
/// This catches hostnames that resolve to private IPs, mitigating DNS-based SSRF.
#[cfg(feature = "resolve")]
pub async fn is_private_host_resolved(host: &str) -> bool {
    if is_private_host(host) {
        return true;
    }
    match tokio::net::lookup_host(format!("{host}:443")).await {
        Ok(addrs) => addrs.into_iter().any(|addr| is_private_ip(addr.ip())),
        Err(_) => true,
    }
}

/// Custom DNS resolver that blocks private/internal IPs at connect time.
///
/// Eliminates DNS rebinding TOCTOU: reqwest uses this resolver for the actual
/// connection, so there is no gap between the safety check and the TCP handshake.
///
/// # Usage
///
/// ```no_run
/// use ssrf_guard::SsrfSafeResolver;
/// use std::sync::Arc;
///
/// let client = reqwest::Client::builder()
///     .dns_resolver(Arc::new(SsrfSafeResolver))
///     .redirect(reqwest::redirect::Policy::none())
///     .build()
///     .unwrap();
/// ```
#[cfg(feature = "reqwest")]
pub struct SsrfSafeResolver;

#[cfg(feature = "reqwest")]
impl reqwest::dns::Resolve for SsrfSafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            if is_private_host(&host) {
                return Err(format!("blocked: {host} is a private hostname").into());
            }
            let addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host(format!("{host}:0"))
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                    .filter(|addr| !is_private_ip(addr.ip()))
                    .collect();
            if addrs.is_empty() {
                return Err(format!("blocked: {host} resolves only to private IPs").into());
            }
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_private_ipv4 ---

    #[test]
    fn ipv4_loopback() {
        assert!(is_private_ipv4(Ipv4Addr::LOCALHOST));
        assert!(is_private_ipv4("127.0.0.1".parse().unwrap()));
        assert!(is_private_ipv4("127.255.255.255".parse().unwrap()));
    }

    #[test]
    fn ipv4_rfc1918() {
        assert!(is_private_ipv4("10.0.0.1".parse().unwrap()));
        assert!(is_private_ipv4("10.255.255.255".parse().unwrap()));
        assert!(is_private_ipv4("172.16.0.1".parse().unwrap()));
        assert!(is_private_ipv4("172.31.255.255".parse().unwrap()));
        assert!(is_private_ipv4("192.168.0.1".parse().unwrap()));
        assert!(is_private_ipv4("192.168.255.255".parse().unwrap()));
    }

    #[test]
    fn ipv4_link_local() {
        assert!(is_private_ipv4("169.254.0.1".parse().unwrap()));
        assert!(is_private_ipv4("169.254.255.255".parse().unwrap()));
    }

    #[test]
    fn ipv4_cgnat() {
        assert!(is_private_ipv4("100.64.0.0".parse().unwrap()));
        assert!(is_private_ipv4("100.64.0.1".parse().unwrap()));
        assert!(is_private_ipv4("100.127.255.255".parse().unwrap()));
        // Just outside CGNAT
        assert!(!is_private_ipv4("100.128.0.0".parse().unwrap()));
        assert!(!is_private_ipv4("100.63.255.255".parse().unwrap()));
    }

    #[test]
    fn ipv4_zero_network() {
        assert!(is_private_ipv4("0.0.0.0".parse().unwrap()));
        assert!(is_private_ipv4("0.255.255.255".parse().unwrap()));
    }

    #[test]
    fn ipv4_broadcast() {
        assert!(is_private_ipv4("255.255.255.255".parse().unwrap()));
    }

    #[test]
    fn ipv4_public() {
        assert!(!is_private_ipv4("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ipv4("1.1.1.1".parse().unwrap()));
        assert!(!is_private_ipv4("93.184.216.34".parse().unwrap()));
        assert!(!is_private_ipv4("203.0.113.1".parse().unwrap()));
    }

    // --- is_private_ipv6 ---

    #[test]
    fn ipv6_loopback() {
        assert!(is_private_ipv6(Ipv6Addr::LOCALHOST));
    }

    #[test]
    fn ipv6_unspecified() {
        assert!(is_private_ipv6(Ipv6Addr::UNSPECIFIED));
    }

    #[test]
    fn ipv6_ula() {
        assert!(is_private_ipv6("fc00::1".parse().unwrap()));
        assert!(is_private_ipv6("fd00::1".parse().unwrap()));
        assert!(is_private_ipv6("fdff:ffff::1".parse().unwrap()));
    }

    #[test]
    fn ipv6_link_local() {
        assert!(is_private_ipv6("fe80::1".parse().unwrap()));
        assert!(is_private_ipv6("fe80::1234:5678".parse().unwrap()));
        assert!(is_private_ipv6("febf::1".parse().unwrap()));
    }

    #[test]
    fn ipv6_mapped_private_v4() {
        // ::ffff:127.0.0.1
        assert!(is_private_ipv6("::ffff:127.0.0.1".parse().unwrap()));
        // ::ffff:10.0.0.1
        assert!(is_private_ipv6("::ffff:10.0.0.1".parse().unwrap()));
        // ::ffff:192.168.1.1
        assert!(is_private_ipv6("::ffff:192.168.1.1".parse().unwrap()));
        // ::ffff:100.64.0.1 (CGNAT)
        assert!(is_private_ipv6("::ffff:100.64.0.1".parse().unwrap()));
    }

    #[test]
    fn ipv6_mapped_public_v4() {
        // ::ffff:8.8.8.8
        assert!(!is_private_ipv6("::ffff:8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn ipv6_public() {
        assert!(!is_private_ipv6("2001:db8::1".parse().unwrap()));
        assert!(!is_private_ipv6("2607:f8b0:4004:800::200e".parse().unwrap()));
    }

    // --- is_private_host ---

    #[test]
    fn host_localhost() {
        assert!(is_private_host("localhost"));
        assert!(is_private_host("localhost.localdomain"));
    }

    #[test]
    fn host_dotlocal() {
        assert!(is_private_host("myserver.local"));
        assert!(is_private_host("printer.local"));
    }

    #[test]
    fn host_dotinternal() {
        assert!(is_private_host("service.internal"));
        assert!(is_private_host("db.prod.internal"));
    }

    #[test]
    fn host_ip_literal() {
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("10.0.0.1"));
        assert!(!is_private_host("8.8.8.8"));
    }

    #[test]
    fn host_public() {
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("mastodon.social"));
    }

    // --- validate_url ---

    #[test]
    fn url_valid_https() {
        let url = url::Url::parse("https://example.com/api").unwrap();
        assert!(validate_url(&url).is_ok());
    }

    #[test]
    fn url_rejects_http() {
        let url = url::Url::parse("http://example.com/api").unwrap();
        assert!(matches!(validate_url(&url), Err(SsrfError::NotHttps(_))));
    }

    #[test]
    fn url_rejects_private_ip() {
        let url = url::Url::parse("https://192.168.1.1/admin").unwrap();
        assert!(matches!(validate_url(&url), Err(SsrfError::PrivateHost(_))));
    }

    #[test]
    fn url_rejects_localhost() {
        let url = url::Url::parse("https://localhost/secret").unwrap();
        assert!(matches!(validate_url(&url), Err(SsrfError::PrivateHost(_))));
    }

    #[test]
    fn url_rejects_dotlocal() {
        let url = url::Url::parse("https://app.local/metrics").unwrap();
        assert!(matches!(validate_url(&url), Err(SsrfError::PrivateHost(_))));
    }

    #[test]
    fn url_rejects_cgnat() {
        let url = url::Url::parse("https://100.64.0.1/").unwrap();
        assert!(matches!(validate_url(&url), Err(SsrfError::PrivateHost(_))));
    }

    #[test]
    fn url_no_host() {
        let url = url::Url::parse("file:///etc/passwd").unwrap();
        assert!(matches!(validate_url(&url), Err(SsrfError::NoHost)));
    }
}
