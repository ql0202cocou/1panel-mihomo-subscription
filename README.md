# Mihomo Subscription Manager

> **状态:MVP 已实现,尚未发布。** 后端(认证、Profile 管理、SSRF 防护拉取、
> 订阅转换、生成/缓存/公开端点、限流)与 Web SPA 均已实现并有测试与多阶段镜像;
> 发布前仍需补齐 1Panel 应用包安装表单(见 `docs/1panel-app.md`)。
>
> **Status: MVP implemented, not yet released.** The backend (auth, profile
> management, SSRF-protected fetch, conversion, generate/cache/public endpoint,
> rate limiting) and the web SPA are implemented with tests and a multi-stage
> image. Before release, the 1Panel app package install form still needs
> updating (see `docs/1panel-app.md`).

面向 1Panel 自托管场景的轻量级 Mihomo 订阅转换与分发服务(类 Sub-Store Lite):
录入机场订阅,配置自定义分流规则、节点和代理分组,生成长期有效的 Mihomo
订阅链接。

A lightweight self-hosted Mihomo subscription conversion and distribution
service for 1Panel (a Sub-Store Lite): register provider subscriptions,
configure custom rules, nodes, and proxy groups, and generate long-lived
Mihomo subscription links.

## 核心能力 / Capabilities

- Web 管理页面,管理员登录(凭据来自 1Panel 安装参数)。
  Web admin UI with login (credentials from 1Panel install parameters).
- `mihomo/clash -> mihomo` 订阅转换,后续扩展 `surge`/`loon`。
  `mihomo/clash -> mihomo` conversion first; `surge`/`loon` later.
- 自定义规则替换、自定义节点和代理分组追加。
  Custom rule replacement; custom node and proxy group appending.
- 永久订阅链接:随机路径前缀 + per-profile token,支持重置。
  Permanent links: random path prefix + per-profile token, both resettable.
- 安全:SSRF 防护(连接时钉死已验证 IP)、订阅 URL 全程脱敏、生成缓存与单飞、
  令牌桶限流、YAML 炸弹防护、同源 CSRF 校验。
  Security: SSRF protection (connect-time pinned IP), provider URL masking,
  generation caching with single-flight, token-bucket rate limiting, YAML-bomb
  guarding, and same-origin CSRF checks.

## 架构 / Architecture

```text
Web UI (Vite + React + TypeScript)
  |
  | REST API (session auth)
  v
Rust / Axum Service
  |-- SQLite: profiles, rulesets, custom nodes/groups, generated cache
  |-- Converter: fetch -> parse -> append -> replace rules -> Mihomo YAML
  |-- Security: admin auth, tokens, SSRF protection, masking
  |
  v
Public link: https://<host>/<public-path-prefix>/api/sub/<profile-token>
```

## 文档 / Documentation

| 文档 / Document | 内容 / Contents |
|------|------|
| [docs/plan.md](docs/plan.md) | 产品计划与 MVP 范围 / Product plan and MVP scope |
| [docs/technical-roadmap.md](docs/technical-roadmap.md) | 架构与实施路线 / Architecture and roadmap |
| [docs/api-design.md](docs/api-design.md) | API 契约与认证行为 / API contracts and auth |
| [docs/data-model.md](docs/data-model.md) | SQLite 模式与迁移 / SQLite schema and migrations |
| [docs/security-design.md](docs/security-design.md) | 安全设计 / Security design |
| [docs/release.md](docs/release.md) | 发布流程 / Release process |
| [docs/1panel-app.md](docs/1panel-app.md) | 1Panel 应用打包 / 1Panel app packaging |
| [docs/changelog.md](docs/changelog.md) | 变更日志 / Changelog |
| [AGENTS.md](AGENTS.md) | 编码代理协作指南 / Coding-agent guide |

## 开发 / Development

后端门禁(与 CI 一致)/ Backend gates (same as CI):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit   # 需 / needs: cargo install cargo-audit；忽略项见 / ignores in .cargo/audit.toml
```

前端(在 `web/` 内)/ Frontend (inside `web/`):

```bash
npm install   # 首次 / first time
npm run dev    # Vite 开发服务器,代理 /api 与 /health / dev server, proxies /api and /health
npm run build  # tsc --noEmit + vite build -> web/dist(由 Axum 托管 / served by Axum）
```

构建镜像 / Build the image:

```bash
docker build -t mihomo-subscription:0.1.0 .
```

1Panel 应用包位于 / The 1Panel app package lives at
[apps/mihomo-subscription](apps/mihomo-subscription)。

## 许可证 / License

[MIT](LICENSE)
