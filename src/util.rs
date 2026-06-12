//! Small shared helpers: timestamps and CSPRNG-backed random identifiers.

use base64::Engine;
use rand::RngCore;

/// Current time as an RFC 3339 UTC string (the project's timestamp format).
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// A per-profile subscription token: 32 random bytes, URL-safe (≥256 bits),
/// per `docs/security-design.md`.
pub fn random_token() -> String {
    random_b64(32)
}

/// A random public path prefix: 16 random bytes, URL-safe (~22 chars, within
/// the 16-24 char range recommended in `docs/security-design.md`).
pub fn random_path_prefix() -> String {
    random_b64(16)
}

fn random_b64(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
