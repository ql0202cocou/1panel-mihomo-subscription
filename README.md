# Mihomo Subscription Manager

> **状态:早期规划阶段。** 设计文档已完成,按文档实施尚未开始;当前代码仅为
> 早期原型脚手架。
>
> **Status: early planning.** Design documents are complete; implementation
> against them has not started. The current code is an early prototype
> scaffold.

面向 1Panel 自托管场景的轻量级 Mihomo 订阅转换与分发服务(类 Sub-Store Lite):
录入机场订阅,配置自定义分流规则、节点和代理分组,生成长期有效的 Mihomo
订阅链接。

A lightweight self-hosted Mihomo subscription conversion and distribution
service for 1Panel (a Sub-Store Lite): register provider subscriptions,
configure custom rules, nodes, and proxy groups, and generate long-lived
Mihomo subscription links.

## 核心能力(规划)/ Planned Capabilities

- Web 管理页面,管理员登录(凭据来自 1Panel 安装参数)。
  Web admin UI with login (credentials from 1Panel install parameters).
- `mihomo/clash -> mihomo` 订阅转换,后续扩展 `surge`/`loon`。
  `mihomo/clash -> mihomo` conversion first; `surge`/`loon` later.
- 自定义规则替换、自定义节点和代理分组追加。
  Custom rule replacement; custom node and proxy group appending.
- 永久订阅链接:随机路径前缀 + per-profile token,支持重置。
  Permanent links: random path prefix + per-profile token, both resettable.
- SSRF 防护、订阅 URL 脱敏、生成缓存。
  SSRF protection, provider URL masking, generation caching.

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

```bash
cargo check
cargo fmt
cargo test

# 构建镜像 / build image
docker build -t mihomo-subscription:0.1.0 .
```

1Panel 应用包位于 / The 1Panel app package lives at
[apps/mihomo-subscription](apps/mihomo-subscription)。

## 许可证 / License

[MIT](LICENSE)
