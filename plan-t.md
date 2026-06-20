# 静态代码审计问题清单（plan-t）

> 审计方式：仅阅读源码与配置文件，未执行编译。  
> 审计范围：`/Users/quinlanhoo/Code/mihomo-subscription/mihomo-subscription`

---

## 1. 管理员口令使用无盐 SHA256 摘要 — 高

- **文件/行号**：`src/auth.rs:59-65`、`src/auth.rs:42-56`
- **问题描述**：
  `AdminAuth` 将 `username + "\0" + password` 做一次 SHA256 摘要后保存在内存中。登录时重新计算摘要，并用 `subtle::ConstantTimeEq` 做常量时间比较。

  SHA256 不是密码哈希函数：无盐、无迭代拉伸，极易被彩虹表 / GPU 爆破；摘要一旦被读取，等价于静态口令。
- **证据**：
  ```rust
  fn credential_digest(username: &str, password: &str) -> [u8; 32] {
      let mut hasher = Sha256::new();
      hasher.update(username.as_bytes());
      hasher.update([0u8]);
      hasher.update(password.as_bytes());
      hasher.finalize().into()
  }
  ```
- **改进建议**：
  使用 `argon2`、`pbkdf2` 或 `scrypt` 等专用密码哈希，至少加盐并设置合理的内存/迭代参数。

---

## 2. 默认 `TraceLayer` 会把公共订阅 token 写入日志 — 中-高

- **文件/行号**：`src/app.rs:170`
- **问题描述**：
  `tower_http::trace::TraceLayer::new_for_http()` 默认在 DEBUG / TRACE 级别记录完整 URI。公共订阅路由 `/:public_path_prefix/api/sub/:token` 的路径同时包含 `public_path_prefix` 与 `profile_token`，一旦日志级别调低就会泄露到日志。
- **证据**：
  ```rust
  .layer(TraceLayer::new_for_http())
  ```
- **改进建议**：
  自定义 `TraceLayer`，对订阅路径做脱敏处理，或关闭对公共订阅路径的 URI 日志。

---

## 3. 多处 `std::sync::Mutex` 在异步上下文使用，panic 会导致 poison 级联 — 中

- **文件/行号**：
  - `src/auth.rs:91,105,120,125`
  - `src/app.rs:57,61`
  - `src/rate_limit.rs:55`
  - `src/single_flight.rs:25`
- **问题描述**：
  代码中大量使用 `self.inner.lock().unwrap()`。若某个异步任务在持有锁时 panic，锁会被标记为 poison，后续 `.lock().unwrap()` 直接 panic，造成服务级故障。
- **改进建议**：
  - 在异步代码中优先使用 `tokio::sync::Mutex`。
  - 若保留 `std::sync::Mutex`，应处理 poison，例如 `lock().unwrap_or_else(|e| e.into_inner())`。

---

## 4. `SingleFlight` 锁表只增不减，存在无界增长 — 中

- **文件/行号**：`src/single_flight.rs:7-27`
- **问题描述**：
  每个 profile ID 第一次被访问时都会永久创建 `Arc<AsyncMutex<()>>` 并保留在 HashMap 中，永不清理。长期运行或大量 profile 时内存持续增长。
- **证据**：
  ```rust
  pub fn lock_for(&self, key: &str) -> Arc<AsyncMutex<()>> {
      let mut map = self.locks.lock().unwrap();
      map.entry(key.to_string()).or_default().clone()
  }
  ```
- **改进建议**：
  在锁释放后或定期扫描，移除无持有者的 `AsyncMutex`（可通过 `Arc::strong_count()` 判断）。

---

## 5. `Origin` 校验仅在存在 Origin 头时生效，且无法校验 scheme — 中

- **文件/行号**：`src/auth.rs:187-207`
- **问题描述**：
  1. `check_origin` 只在校验 `Origin` 头存在时才比较；若请求不带 `Origin`（某些跨站或特殊客户端请求），直接放行，完全依赖 `SameSite=Lax`。
  2. `origin_matches_host` 只比较 `://` 后面的 host 部分，`Host` 头不含 scheme，无法阻止 `http://example.com` 对 HTTPS 部署的 CSRF / 降级攻击。
- **证据**：
  ```rust
  fn origin_matches_host(origin: Option<&str>, host: Option<&str>) -> bool {
      match (origin, host) {
          (Some(origin), Some(host)) => origin.split_once("://").map(|(_, a)| a) == Some(host),
          _ => false,
      }
  }
  ```
- **改进建议**：
  对状态变更管理接口可强制要求 `Origin` 头；在 HTTPS 部署时配合 HSTS 与 `Secure` cookie。

---

## 6. 迁移文件与文档不一致，遗留无用表 — 中

- **文件/位置**：`migrations/0005_rule_providers.sql`、`docs/data-model.md:166-170`
- **问题描述**：
  `data-model.md` 明确说明已移除自定义规则集托管，并应通过 `0006_drop_rule_providers.sql` 删除 `rule_providers` 表；但 `migrations/` 目录只有 `0001`–`0005`，导致该表仍被创建且从未使用。
- **改进建议**：
  补 `0006_drop_rule_providers.sql`，或更新文档说明保留原因。

---

## 7. 前端 `vite` / `esbuild` 存在已知漏洞 — 中-高

- **文件/位置**：`web/package-lock.json`
- **问题描述**：
  `npm audit` 报告 2 个漏洞：
  - `esbuild <=0.24.2`：开发服务器 CSRF（GHSA-67mh-4wv8-2f99，moderate）
  - `vite <=6.4.2`：optimized deps `.map` 路径遍历（GHSA-4w7w-66w2-5vf9，high）
- **改进建议**：
  升级 `vite` 到 8.x，并定期运行 `npm audit fix`。

---

## 8. 敏感环境变量读取后未从进程环境清除 — 低

- **文件/行号**：`src/main.rs:27-28`
- **问题描述**：
  `ADMIN_PASSWORD` 读取后仍保留在进程环境中，任何能读取 `/proc/self/environ` 或子进程 dump 的本地用户都可能泄露。
- **改进建议**：
  读取后立即 `std::env::remove_var("ADMIN_PASSWORD")`。

---

## 9. 会话未绑定客户端指纹 — 低

- **文件/行号**：`src/auth.rs:69-127`
- **问题描述**：
  `SessionStore` 仅保存 `session_id -> last_seen`，未绑定 IP、User-Agent 等指纹。会话 Cookie 一旦被窃取即可复用。
- **改进建议**：
  自托管单实例场景可接受，但应在文档中声明风险；如需更强保护，可绑定 TCP 侧 IP 或 UA。

---

## 10. 部分 DB 更新错误被静默吞掉 — 低

- **文件/行号**：`src/generate.rs:245,249`、`src/generate.rs:213`
- **问题描述**：
  `update_last_fetch` 等使用 `let _ = ...` 忽略错误，属于 best-effort，但会隐藏潜在问题。
- **改进建议**：
  至少记录 `tracing::warn!`。

---

## 11. 环境变量解析失败静默回退 — 低

- **文件/行号**：`src/main.rs:84-87`、`src/main.rs:102-107`
- **问题描述**：
  `PORT`、`FETCH_TIMEOUT_SECONDS` 等解析失败时直接回退默认值，可能隐藏配置错误。
- **改进建议**：
  对无法解析的配置项记录 warning。

---

## 12. Rust 依赖存在维护债 — 低-中

- **文件/位置**：`Cargo.toml`、`Cargo.lock`
- **问题描述**：
  - `reqwest 0.11` 较旧，主流已进 `0.12.x`。
  - `serde_yaml` 已弃用，crate 官方标记为 `+deprecated`。
  - `base64 0.21` 可升级至 `0.22`。
- **当前 CVE**：
  `cargo audit` 未报告 Rust 依赖存在已知漏洞。
- **改进建议**：
  逐步升级到新版依赖；`serde_yaml` 迁移到 `serde_yml` 或 `yaml-rust2`。

---

## 13. 测试覆盖薄弱

- **位置**：`tests/generate.rs`、`tests/auth.rs`
- **问题描述**：
  - `HttpFetcher` 的 DNS 解析、IP 固定、重定向、超时、大小限制等 SSRF 机制未通过本地 mock HTTP server 做集成测试。
  - 会话 7 天 idle 过期、Origin 校验边界、限流器内存清理等缺乏测试。
- **改进建议**：
  增加使用 `tokio::net::TcpListener` 或 `wiremock` 的本地 HTTP 集成测试，以及时间旅行 / 边界测试。

---

## 执行优先级建议

| 优先级 | 问题 |
|--------|------|
| P0 | 1. 管理员口令使用 SHA256 无盐摘要 |
| P0 | 2. 默认 `TraceLayer` 可能泄露公共订阅 token |
| P1 | 3. 异步代码中 `Mutex::lock().unwrap()` poison 风险 |
| P1 | 4. `SingleFlight` 锁表无界增长 |
| P1 | 5. `Origin` 校验不完整 |
| P1 | 6. 迁移 `0006` 缺失 / `rule_providers` 遗留表 |
| P1 | 7. 前端 `vite` / `esbuild` 漏洞 |
| P2 | 8-13. 低风险项、代码质量债、测试补全 |

---

## 备注

- 项目 SSRF 防护整体实现较好，未发现明显绕过。
- 错误响应、请求体/YAML 大小限制、CORS/CSRF 策略等基本符合 `docs/security-design.md`。
