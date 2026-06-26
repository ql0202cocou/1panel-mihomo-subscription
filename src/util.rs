//! 共享小工具:时间戳,以及由 CSPRNG 支撑的随机标识符。

use base64::Engine;
use rand::RngCore;

/// 当前时间,RFC 3339 UTC 字符串(本项目的时间戳格式)。
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// per-profile 的订阅 token:32 随机字节,URL-safe(≥256 位),见 `docs/security-design.md`。
pub fn random_token() -> String {
    random_b64(32)
}

/// 随机公共路径前缀:16 随机字节,URL-safe(~22 字符,落在 `docs/security-design.md`
/// 推荐的 16-24 字符区间内)。
pub fn random_path_prefix() -> String {
    random_b64(16)
}

fn random_b64(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
