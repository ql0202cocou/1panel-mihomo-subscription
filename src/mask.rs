//! Deterministic provider-URL masking, per `docs/security-design.md`.
//!
//! Keep the scheme, host, and path; replace every query parameter value with
//! `***`; drop any userinfo so credentials are never echoed. The same rule is
//! applied everywhere a provider URL might surface (responses, logs, errors).

/// Mask a provider subscription URL. Unparseable input collapses to `***`
/// rather than leaking the raw string.
pub fn mask_url(raw: &str) -> String {
    let Ok(url) = url::Url::parse(raw) else {
        return "***".to_string();
    };

    let mut out = String::new();
    out.push_str(url.scheme());
    out.push_str("://");
    if let Some(host) = url.host_str() {
        out.push_str(host);
    }
    if let Some(port) = url.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    out.push_str(url.path());

    if let Some(query) = url.query() {
        let masked = query
            .split('&')
            .map(|pair| {
                let key = pair.split('=').next().unwrap_or("");
                format!("{key}=***")
            })
            .collect::<Vec<_>>()
            .join("&");
        out.push('?');
        out.push_str(&masked);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::mask_url;

    #[test]
    fn masks_query_values_and_drops_userinfo() {
        assert_eq!(
            mask_url("https://example.com/api/sub?token=abcdef"),
            "https://example.com/api/sub?token=***"
        );
        assert_eq!(
            mask_url("https://u:p@example.com:8443/x?a=1&b=2"),
            "https://example.com:8443/x?a=***&b=***"
        );
        assert_eq!(mask_url("not a url"), "***");
    }
}
