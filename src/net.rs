//! 反向代理后的客户端 IP 推导。
//!
//! 服务跑在 1Panel 反向代理后,故 TCP 对端是代理而非客户端。按 `docs/security-design.md`:
//! 只信任来自已知代理的 `X-Forwarded-For`,并从右往左数——有 `trusted_hops` 个代理时,客户端
//! 是从末尾起第 `trusted_hops` 个条目,故攻击者在左侧拼接的伪造条目不会被误当成客户端。头缺失
//! 或过短时回退到 TCP 对端。

use std::net::IpAddr;

/// 由 `X-Forwarded-For` 头与 TCP 对端推导客户端 IP。
///
/// `trusted_hops` 是本服务与客户端之间的反向代理跳数。`0` 表示不信任任何代理、忽略该头。
pub fn client_ip(xff: Option<&str>, peer: Option<IpAddr>, trusted_hops: usize) -> Option<IpAddr> {
    if trusted_hops >= 1 {
        if let Some(header) = xff {
            let entries: Vec<&str> = header
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            // 客户端位于从右数第 `trusted_hops` 个;至少要有这么多条目,否则视为头格式错误/伪造。
            if entries.len() >= trusted_hops {
                if let Ok(ip) = entries[entries.len() - trusted_hops].parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }
    peer
}

/// 同 [`client_ip`],但返回稳定的字符串 key(无法确定 IP 时为 `"unknown"`),适合做限流 key 与日志。
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
        // 单个代理把真实客户端追加在最右,故它是最右条目。
        assert_eq!(
            client_ip(Some("203.0.113.9"), Some(ip("10.0.0.1")), 1),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn spoofed_left_entries_are_ignored() {
        // 攻击者在前面拼接伪造 IP;1 个受信跳时我们仍取最右(代理实际观测到的)。
        assert_eq!(
            client_ip(Some("1.1.1.1, 203.0.113.9"), Some(ip("10.0.0.1")), 1),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn two_trusted_hops_take_third_from_relevant_end() {
        // [client, proxy] + 2 个受信跳 -> 客户端是最左。
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
        // 头比受信跳数还短 -> 视为格式错误 -> 回退对端。
        assert_eq!(
            client_ip(Some("203.0.113.9"), Some(ip("10.0.0.1")), 2),
            Some(ip("10.0.0.1"))
        );
    }

    #[test]
    fn zero_trusted_hops_ignores_header() {
        // 直接暴露:绝不信任客户端可控的头。
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
