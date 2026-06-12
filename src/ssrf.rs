//! SSRF protection: pure URL and IP validation.
//!
//! Implements the rules in `docs/security-design.md`. This module is
//! network-free and fully table-tested; the actual pinned fetch lives in
//! `fetch.rs` and calls into here for every URL and every redirect hop.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;

use ipnet::IpNet;
use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsrfError {
    /// Scheme is not http/https.
    Scheme,
    /// Missing or empty host.
    Host,
    /// Credentials embedded in the URL.
    Credentials,
    /// Hostname is a loopback name (e.g. localhost).
    BlockedHost,
    /// Resolved or literal IP falls in a blocked range.
    BlockedIp,
}

/// IPv4 ranges that must never be the connect target (see security-design.md).
const BLOCKED_V4: &[&str] = &[
    "0.0.0.0/8",
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.0.0.0/24",
    "192.0.2.0/24",
    "192.88.99.0/24",
    "192.168.0.0/16",
    "198.18.0.0/15",
    "198.51.100.0/24",
    "203.0.113.0/24",
    "224.0.0.0/4",
    "240.0.0.0/4",
];

/// IPv6 ranges blocked outright. The IPv4-embedding ranges (IPv4-mapped,
/// NAT64, 6to4) are handled separately by unwrapping the embedded IPv4.
const BLOCKED_V6: &[&str] = &["::/128", "::1/128", "fc00::/7", "fe80::/10", "ff00::/8"];

fn blocked_nets() -> &'static Vec<IpNet> {
    static NETS: OnceLock<Vec<IpNet>> = OnceLock::new();
    NETS.get_or_init(|| {
        BLOCKED_V4
            .iter()
            .chain(BLOCKED_V6.iter())
            .map(|c| c.parse().expect("valid CIDR literal"))
            .collect()
    })
}

/// Validate the static parts of a URL (scheme, credentials, host name). IP
/// reachability is checked separately via [`is_blocked_ip`] after resolution.
pub fn validate_url(url: &Url) -> Result<(), SsrfError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(SsrfError::Scheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(SsrfError::Credentials);
    }
    match url.host() {
        None => Err(SsrfError::Host),
        Some(Host::Domain("")) => Err(SsrfError::Host),
        Some(Host::Domain(d)) => {
            let h = d.to_ascii_lowercase();
            if h == "localhost" || h.ends_with(".localhost") {
                Err(SsrfError::BlockedHost)
            } else {
                Ok(())
            }
        }
        // A bare-IP host is validated here so blocked literals never resolve.
        Some(Host::Ipv4(ip)) => reject_if_blocked(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => reject_if_blocked(IpAddr::V6(ip)),
    }
}

fn reject_if_blocked(ip: IpAddr) -> Result<(), SsrfError> {
    if is_blocked_ip(ip) {
        Err(SsrfError::BlockedIp)
    } else {
        Ok(())
    }
}

/// True if connecting to `ip` is forbidden. For IPv6 that embeds an IPv4
/// address, the embedded IPv4 is extracted and checked against the IPv4
/// blocklist (closing the `::ffff:127.0.0.1` / NAT64 / 6to4 bypasses).
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = embedded_ipv4(v6) {
                return is_blocked_v4(v4);
            }
            blocked_nets()
                .iter()
                .any(|net| net.contains(&IpAddr::V6(v6)))
        }
    }
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    blocked_nets()
        .iter()
        .any(|net| net.contains(&IpAddr::V4(ip)))
}

/// Extract an embedded IPv4 address from IPv4-mapped (`::ffff:0:0/96`),
/// NAT64 (`64:ff9b::/96`), or 6to4 (`2002::/16`) IPv6 addresses.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return Some(v4);
    }
    let o = ip.octets();
    // NAT64 well-known prefix 64:ff9b::/96.
    if o[0] == 0x00 && o[1] == 0x64 && o[2] == 0xff && o[3] == 0x9b && o[4..12] == [0u8; 8] {
        return Some(Ipv4Addr::new(o[12], o[13], o[14], o[15]));
    }
    // 6to4 2002::/16 carries the IPv4 in octets 2..6.
    if o[0] == 0x20 && o[1] == 0x02 {
        return Some(Ipv4Addr::new(o[2], o[3], o[4], o[5]));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn blocks_every_documented_ipv4_range() {
        for (addr, blocked) in [
            ("0.0.0.0", true),
            ("10.1.2.3", true),
            ("100.64.0.1", true),
            ("127.0.0.1", true),
            ("169.254.1.1", true),
            ("172.16.5.4", true),
            ("192.0.2.5", true),
            ("192.88.99.1", true),
            ("192.168.1.1", true),
            ("198.18.0.1", true),
            ("198.51.100.7", true),
            ("203.0.113.9", true),
            ("224.0.0.1", true),
            ("240.0.0.1", true),
            // Public addresses must be allowed.
            ("1.1.1.1", false),
            ("8.8.8.8", false),
            ("93.184.216.34", false),
        ] {
            assert_eq!(is_blocked_ip(ip(addr)), blocked, "{addr}");
        }
    }

    #[test]
    fn blocks_ipv6_ranges_and_public_is_allowed() {
        assert!(is_blocked_ip(ip("::1")));
        assert!(is_blocked_ip(ip("::")));
        assert!(is_blocked_ip(ip("fc00::1")));
        assert!(is_blocked_ip(ip("fe80::1")));
        assert!(is_blocked_ip(ip("ff02::1")));
        assert!(!is_blocked_ip(ip("2606:4700:4700::1111")));
    }

    #[test]
    fn unwraps_ipv4_embedding_bypasses() {
        // IPv4-mapped loopback.
        assert!(is_blocked_ip(ip("::ffff:127.0.0.1")));
        // IPv4-mapped private.
        assert!(is_blocked_ip(ip("::ffff:10.0.0.1")));
        // IPv4-mapped public is allowed.
        assert!(!is_blocked_ip(ip("::ffff:8.8.8.8")));
        // NAT64 wrapping a private address.
        assert!(is_blocked_ip(ip("64:ff9b::192.168.0.1")));
        // 6to4 wrapping loopback (2002:7f00:0001::).
        assert!(is_blocked_ip("2002:7f00:0001::".parse().unwrap()));
    }

    #[test]
    fn url_validation_rules() {
        assert_eq!(
            validate_url(&Url::parse("ftp://x/").unwrap()),
            Err(SsrfError::Scheme)
        );
        assert_eq!(
            validate_url(&Url::parse("https://user:pass@example.com/").unwrap()),
            Err(SsrfError::Credentials)
        );
        assert_eq!(
            validate_url(&Url::parse("http://localhost/").unwrap()),
            Err(SsrfError::BlockedHost)
        );
        assert_eq!(
            validate_url(&Url::parse("http://127.0.0.1/").unwrap()),
            Err(SsrfError::BlockedIp)
        );
        assert_eq!(
            validate_url(&Url::parse("http://[::ffff:169.254.0.1]/").unwrap()),
            Err(SsrfError::BlockedIp)
        );
        assert!(validate_url(&Url::parse("https://example.com/sub?token=x").unwrap()).is_ok());
    }
}
