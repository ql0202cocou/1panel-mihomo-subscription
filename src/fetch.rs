//! SSRF-protected provider fetch.
//!
//! For every URL — and every redirect hop — this validates the URL, resolves
//! the host, checks the resolved IP against the blocklist, and pins reqwest to
//! that exact validated IP so a later DNS answer cannot redirect the
//! connection (DNS-rebinding safe). The response body is read with a streamed
//! byte cap rather than trusting `Content-Length`. See `docs/security-design.md`.

use std::net::SocketAddr;
use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION};
use reqwest::redirect::Policy;
use tokio::net::lookup_host;
use url::{Host, Url};

use crate::ssrf::{self, SsrfError};

const MAX_REDIRECTS: usize = 3;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Default `User-Agent` for provider fetches. Many airport panels (SSPanel,
/// V2board, …) gate the subscription on a Clash-family UA and return `403`/`401`
/// — or a non-YAML page — to unknown clients. `clash.meta` both matches the
/// common `/clash/i` check and signals Meta support so panels serve Mihomo YAML.
/// Overridable via `FETCH_USER_AGENT`.
pub const DEFAULT_USER_AGENT: &str = "clash.meta/1.0";

/// Abstraction over the provider fetch so the generate/public paths can be
/// tested without real network access. Production uses [`HttpFetcher`].
#[async_trait::async_trait]
pub trait SubscriptionFetcher: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<Fetched, FetchError>;
}

/// The real SSRF-protected fetcher with configured timeout and size cap.
pub struct HttpFetcher {
    pub timeout: Duration,
    pub max_bytes: usize,
    /// `User-Agent` sent on provider requests (see [`DEFAULT_USER_AGENT`]).
    pub user_agent: String,
}

#[async_trait::async_trait]
impl SubscriptionFetcher for HttpFetcher {
    async fn fetch(&self, url: &str) -> Result<Fetched, FetchError> {
        fetch_subscription(url, self.timeout, self.max_bytes, &self.user_agent).await
    }
}

/// A successful provider fetch.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub body: String,
    /// Sanitized `subscription-userinfo` header, if present and well-formed.
    pub subscription_userinfo: Option<String>,
}

#[derive(Debug)]
pub enum FetchError {
    Ssrf(SsrfError),
    Dns,
    Timeout,
    TooLarge,
    TooManyRedirects,
    Http(u16),
    BadContentType,
    Network,
}

impl FetchError {
    /// Map to a `last_fetch_status` label (see `docs/data-model.md`).
    pub fn status_label(&self) -> String {
        match self {
            FetchError::Ssrf(_) => "ssrf_rejected".to_string(),
            FetchError::Dns => "dns_error".to_string(),
            FetchError::Timeout => "timeout".to_string(),
            FetchError::TooLarge => "too_large".to_string(),
            FetchError::TooManyRedirects => "too_many_redirects".to_string(),
            FetchError::Http(code) => format!("http_error:{code}"),
            FetchError::BadContentType => "bad_content_type".to_string(),
            FetchError::Network => "network_error".to_string(),
        }
    }
}

impl From<SsrfError> for FetchError {
    fn from(e: SsrfError) -> Self {
        FetchError::Ssrf(e)
    }
}

/// Fetch a provider subscription with full SSRF protection and limits.
///
/// `total_timeout` bounds the whole request; `max_bytes` caps the streamed
/// response body.
pub async fn fetch_subscription(
    raw_url: &str,
    total_timeout: Duration,
    max_bytes: usize,
    user_agent: &str,
) -> Result<Fetched, FetchError> {
    let mut url = Url::parse(raw_url).map_err(|_| FetchError::Ssrf(SsrfError::Host))?;

    for _ in 0..=MAX_REDIRECTS {
        ssrf::validate_url(&url)?;
        let addr = resolve_validated(&url).await?;

        let host = url.host_str().ok_or(FetchError::Ssrf(SsrfError::Host))?;
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(total_timeout)
            .user_agent(user_agent) // many panels gate the subscription on a Clash-family UA
            .redirect(Policy::none()) // we follow redirects manually to re-validate each hop
            .resolve(host, addr) // pin the validated IP
            .build()
            .map_err(|_| FetchError::Network)?;

        let resp = client
            .get(url.clone())
            .header(ACCEPT, "text/yaml, text/plain, application/x-yaml, */*")
            .send()
            .await
            .map_err(classify_reqwest)?;

        let status = resp.status();

        if status.is_redirection() {
            let location = resp
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(FetchError::Network)?;
            // Resolve relative redirects against the current URL, then loop to
            // re-validate the new target.
            url = url
                .join(location)
                .map_err(|_| FetchError::Ssrf(SsrfError::Host))?;
            continue;
        }

        if !status.is_success() {
            return Err(FetchError::Http(status.as_u16()));
        }

        reject_binary_content(&resp)?;
        let subscription_userinfo = resp
            .headers()
            .get("subscription-userinfo")
            .and_then(|v| v.to_str().ok())
            .and_then(sanitize_userinfo);

        let body = read_limited(resp, max_bytes).await?;
        return Ok(Fetched {
            body,
            subscription_userinfo,
        });
    }

    Err(FetchError::TooManyRedirects)
}

/// Resolve the URL host to a single validated, connectable socket address.
async fn resolve_validated(url: &Url) -> Result<SocketAddr, FetchError> {
    let port = url.port_or_known_default().unwrap_or(80);
    match url.host().ok_or(FetchError::Ssrf(SsrfError::Host))? {
        Host::Ipv4(ip) => ok_if_allowed((ip, port).into()),
        Host::Ipv6(ip) => ok_if_allowed((ip, port).into()),
        Host::Domain(domain) => {
            let candidates = lookup_host((domain, port))
                .await
                .map_err(|_| FetchError::Dns)?;
            let mut saw_any = false;
            for addr in candidates {
                saw_any = true;
                if !ssrf::is_blocked_ip(addr.ip()) {
                    return Ok(addr);
                }
            }
            if saw_any {
                Err(FetchError::Ssrf(SsrfError::BlockedIp))
            } else {
                Err(FetchError::Dns)
            }
        }
    }
}

fn ok_if_allowed(addr: SocketAddr) -> Result<SocketAddr, FetchError> {
    if ssrf::is_blocked_ip(addr.ip()) {
        Err(FetchError::Ssrf(SsrfError::BlockedIp))
    } else {
        Ok(addr)
    }
}

fn reject_binary_content(resp: &reqwest::Response) -> Result<(), FetchError> {
    if let Some(ct) = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        let major = ct
            .split('/')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if matches!(major.as_str(), "image" | "audio" | "video" | "font") {
            return Err(FetchError::BadContentType);
        }
    }
    Ok(())
}

async fn read_limited(resp: reqwest::Response, max_bytes: usize) -> Result<String, FetchError> {
    let mut resp = resp;
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(classify_reqwest)? {
        if buf.len() + chunk.len() > max_bytes {
            return Err(FetchError::TooLarge);
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf).map_err(|_| FetchError::BadContentType)
}

/// Accept a `subscription-userinfo` value only if it is a single line free of
/// control characters (header-injection safe), per `docs/security-design.md`.
fn sanitize_userinfo(value: &str) -> Option<String> {
    if value.is_empty() || value.chars().any(|c| c.is_control()) {
        None
    } else {
        Some(value.to_string())
    }
}

fn classify_reqwest(err: reqwest::Error) -> FetchError {
    if err.is_timeout() {
        FetchError::Timeout
    } else {
        FetchError::Network
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_user_agent_is_clash_compatible() {
        // Panels commonly gate on a case-insensitive `clash` match; keep it so.
        assert!(DEFAULT_USER_AGENT.to_ascii_lowercase().contains("clash"));
    }

    #[test]
    fn sanitize_rejects_control_characters() {
        assert_eq!(
            sanitize_userinfo("upload=1; download=2; total=3"),
            Some("upload=1; download=2; total=3".to_string())
        );
        assert_eq!(sanitize_userinfo("inject\r\nSet-Cookie: x"), None);
        assert_eq!(sanitize_userinfo(""), None);
    }

    #[tokio::test]
    async fn rejects_blocked_host_before_any_connection() {
        let err = fetch_subscription("http://127.0.0.1/x", Duration::from_secs(5), 1024, "ua")
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Ssrf(SsrfError::BlockedIp)));
        assert_eq!(err.status_label(), "ssrf_rejected");
    }

    #[tokio::test]
    async fn rejects_non_http_scheme() {
        let err = fetch_subscription("file:///etc/passwd", Duration::from_secs(5), 1024, "ua")
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Ssrf(SsrfError::Scheme)));
    }

    #[tokio::test]
    async fn rejects_ipv4_mapped_ipv6_loopback() {
        let err = fetch_subscription(
            "http://[::ffff:127.0.0.1]/x",
            Duration::from_secs(5),
            1024,
            "ua",
        )
        .await
        .unwrap_err();
        assert!(matches!(err, FetchError::Ssrf(SsrfError::BlockedIp)));
    }
}
