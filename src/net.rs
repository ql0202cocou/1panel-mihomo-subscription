//! Client IP derivation behind a reverse proxy.
//!
//! The service runs behind the 1Panel reverse proxy, so the TCP peer is the
//! proxy, not the client. Per `docs/security-design.md`, trust
//! `X-Forwarded-For` only from the known proxy and count from the right: with
//! `trusted_hops` proxies the client is the entry `trusted_hops` from the end,
//! so an attacker prepending fake entries on the left cannot be mistaken for
//! the client. A missing or too-short header falls back to the TCP peer.

use std::net::IpAddr;

/// Derive the client IP from the `X-Forwarded-For` header and TCP peer.
///
/// `trusted_hops` is the number of reverse-proxy hops between this service and
/// the client. `0` means no proxy is trusted and the header is ignored.
pub fn client_ip(xff: Option<&str>, peer: Option<IpAddr>, trusted_hops: usize) -> Option<IpAddr> {
    if trusted_hops >= 1 {
        if let Some(header) = xff {
            let entries: Vec<&str> = header
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            // The client sits `trusted_hops` from the right; require at least
            // that many entries, otherwise the header is malformed/forged.
            if entries.len() >= trusted_hops {
                if let Ok(ip) = entries[entries.len() - trusted_hops].parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }
    peer
}

/// Like [`client_ip`] but returns a stable string key (`"unknown"` when the IP
/// cannot be determined), suitable for rate-limit keys and logs.
pub fn client_ip_string(xff: Option<&str>, peer: Option<IpAddr>, trusted_hops: usize) -> String {
    client_ip(xff, peer, trusted_hops)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn single_proxy_takes_rightmost_xff_entry() {
        // One proxy appends the real client; it is the rightmost entry.
        assert_eq!(
            client_ip(Some("203.0.113.9"), Some(ip("10.0.0.1")), 1),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn spoofed_left_entries_are_ignored() {
        // Attacker prepends a fake IP; with 1 trusted hop we still take the
        // rightmost (what our proxy actually observed).
        assert_eq!(
            client_ip(Some("1.1.1.1, 203.0.113.9"), Some(ip("10.0.0.1")), 1),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn two_trusted_hops_take_third_from_relevant_end() {
        // [client, proxy] with 2 trusted hops -> client is the leftmost.
        assert_eq!(
            client_ip(Some("203.0.113.9, 10.0.0.2"), Some(ip("10.0.0.1")), 2),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn missing_or_short_header_falls_back_to_peer() {
        assert_eq!(
            client_ip(None, Some(ip("10.0.0.1")), 1),
            Some(ip("10.0.0.1"))
        );
        // Header shorter than trusted hops -> malformed -> peer.
        assert_eq!(
            client_ip(Some("203.0.113.9"), Some(ip("10.0.0.1")), 2),
            Some(ip("10.0.0.1"))
        );
    }

    #[test]
    fn zero_trusted_hops_ignores_header() {
        // Direct exposure: never trust the client-controlled header.
        assert_eq!(
            client_ip(Some("1.1.1.1"), Some(ip("203.0.113.9")), 0),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn string_form_is_unknown_without_any_source() {
        assert_eq!(client_ip_string(None, None, 1), "unknown");
    }
}
