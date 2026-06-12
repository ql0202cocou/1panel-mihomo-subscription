# API 设计 / API Design

> **状态:规划阶段。** 本文档描述目标 API 契约,尚未实现。当前 `src/main.rs`
> 中的 `subscriptions` CRUD 是早期原型,将按本文档重构为 profile 管理。
>
> **Status: planning.** This document defines the target API contract; it is
> not implemented yet. The current `subscriptions` CRUD in `src/main.rs` is an
> early prototype and will be reworked into profile management per this design.

相关文档 / Related documents: `plan.md`、`technical-roadmap.md`、
`security-design.md`、`data-model.md`。

## 设计原则 / Principles

- 管理 API 统一挂载在 `/api` 下,要求登录会话。
- 公开订阅端点不要求登录,但要求随机路径前缀和 per-profile token。
- 管理 API 使用 JSON;公开订阅端点输出 Mihomo YAML。
- 公开端点的任何校验失败统一返回 `404`,不泄露失败原因。
- 管理 API 响应默认对原始机场订阅 URL 脱敏。
- 管理 API 仅限同源访问:SPA 由 Axum 同源提供,不启用 CORS 层
  (见 `security-design.md` 的 CORS and CSRF)。

&nbsp;

- All management APIs live under `/api` and require an authenticated session.
- The public subscription endpoint requires no login, but requires the random
  public path prefix and a per-profile token.
- Management APIs speak JSON; the public endpoint returns Mihomo YAML.
- Any public-endpoint validation failure returns a uniform `404` without
  revealing which check failed.
- Management responses mask original provider subscription URLs by default.
- The management API is same-origin only: the SPA is served by Axum from the
  same origin and no CORS layer is enabled (see CORS and CSRF in
  `security-design.md`).
- All management request bodies are bounded by a maximum size (default 1 MB);
  oversized bodies are rejected with `413`. Admin-submitted node/group YAML is
  parsed with the same alias/nesting limits as provider content (see
  `security-design.md`).

## 通用约定 / Conventions

- 时间格式 / Timestamps: RFC 3339 UTC, e.g. `2026-06-12T08:00:00Z`.
- 标识符 / IDs: UUID v4 字符串 / UUID v4 strings.
- 请求与响应编码 / Encoding: `application/json; charset=utf-8`
  (公开端点为 / public endpoint uses `text/yaml`).

错误响应格式 / Error response shape:

```json
{
  "error": {
    "code": "validation_failed",
    "message": "Profile validation failed",
    "details": [
      "rules line 12 references unknown group `Auto`",
      "custom group `Proxy` conflicts with a provider group name"
    ]
  }
}
```

状态码 / Status codes:

| Code | 含义 / Meaning |
|------|----------------|
| 200 / 201 / 204 | 成功 / Success (read / created / no content) |
| 400 | 请求体格式错误或校验失败 / Malformed body or validation failure |
| 401 | 未登录或会话过期 / Not authenticated or session expired |
| 404 | 资源不存在;公开端点的统一失败响应 / Not found; uniform public-endpoint failure |
| 409 | 名称冲突(如分组重名)/ Name conflict (e.g. duplicate group name) |
| 413 | 请求体超过大小上限 / Request body exceeds the size limit |
| 429 | 触发限流 / Rate limited |
| 500 | 服务内部错误,不含敏感信息 / Internal error, no sensitive data |

## 认证 / Authentication

管理员凭据来自环境变量 `ADMIN_USERNAME` 和 `ADMIN_PASSWORD`(由 1Panel 安装表单
写入 compose)。登录成功后签发会话 Cookie:`HttpOnly`、`SameSite=Lax`,HTTPS 部署
时附加 `Secure`。登录失败按 IP 和账户限流。

Admin credentials come from the `ADMIN_USERNAME` and `ADMIN_PASSWORD`
environment variables (written into compose by the 1Panel install form). A
successful login issues a session cookie: `HttpOnly`, `SameSite=Lax`, plus
`Secure` under HTTPS. Failed logins are rate limited by IP and account.

```text
POST /api/auth/login     { "username": "...", "password": "..." }
  -> 204 + Set-Cookie: session=...
  -> 401 invalid credentials
  -> 429 too many attempts

POST /api/auth/logout
  -> 204 + clears session cookie

GET  /api/auth/session
  -> 200 { "username": "admin" }
  -> 401 not logged in
```

除 `/health`、登录端点和公开订阅端点外,所有路由要求有效会话,否则返回 `401`。

All routes except `/health`, the login endpoints, and the public subscription
endpoint require a valid session and otherwise return `401`.

## 端点总览 / Endpoint Overview

| Method | Path | Auth | 说明 / Description |
|--------|------|------|--------------------|
| GET | `/health` | 否/No | 健康检查 / Health check |
| POST | `/api/auth/login` | 否/No | 登录 / Login |
| POST | `/api/auth/logout` | 是/Yes | 登出 / Logout |
| GET | `/api/auth/session` | 是/Yes | 当前会话 / Current session |
| GET | `/api/profiles` | 是/Yes | 配置列表 / List profiles |
| POST | `/api/profiles` | 是/Yes | 创建配置 / Create profile |
| GET | `/api/profiles/:id` | 是/Yes | 配置详情 / Profile detail |
| PUT | `/api/profiles/:id` | 是/Yes | 更新基础信息 / Update base fields |
| DELETE | `/api/profiles/:id` | 是/Yes | 删除配置 / Delete profile |
| PUT | `/api/profiles/:id/rules` | 是/Yes | 替换自定义规则 / Replace custom rules |
| GET / POST | `/api/profiles/:id/nodes` | 是/Yes | 自定义节点 / Custom nodes |
| PUT / DELETE | `/api/profiles/:id/nodes/:node_id` | 是/Yes | 单个节点 / Single node |
| GET / POST | `/api/profiles/:id/groups` | 是/Yes | 自定义分组 / Custom groups |
| PUT / DELETE | `/api/profiles/:id/groups/:group_id` | 是/Yes | 单个分组 / Single group |
| GET | `/api/profiles/:id/preview` | 是/Yes | 预览生成的 YAML / Preview generated YAML |
| POST | `/api/profiles/:id/generate` | 是/Yes | 校验并生成托管链接 / Validate & generate hosted link |
| POST | `/api/profiles/:id/reset-token` | 是/Yes | 重置该配置 token / Reset profile token |
| GET | `/api/settings` | 是/Yes | 查看应用设置 / Read app settings |
| POST | `/api/settings/reset-public-path` | 是/Yes | 重置公共路径前缀 / Reset public path prefix |
| GET | `/:public_path_prefix/api/sub/:token` | 否/No | 公开订阅下载 / Public subscription download |

## Profile 资源 / Profile Resource

列表返回摘要,详情返回完整对象(含规则、节点、分组)。`source_url` 默认脱敏,
仅在创建/更新请求中接受完整值。

The list endpoint returns summaries; the detail endpoint returns the full
object including rules, nodes, and groups. `source_url` is masked in responses
and only accepted in full in create/update requests.

```json
{
  "id": "5f0c2c4e-...",
  "name": "My Profile",
  "source_type": "clash",
  "source_url_masked": "https://example.com/api/sub?token=***",
  "output_type": "mihomo",
  "enabled": true,
  "subscription_url": "https://sub.example.com/7fKp9mQx/api/sub/3w7s9xQm...",
  "last_generated_at": "2026-06-12T08:00:00Z",
  "last_fetch_at": "2026-06-12T08:00:00Z",
  "last_fetch_status": "success",
  "rules": { "content": "RULE-SET,...\nMATCH,Proxy", "updated_at": "..." },
  "nodes": [ { "id": "...", "name": "my-ss", "node_type": "ss", "enabled": true } ],
  "groups": [ { "id": "...", "name": "MyGroup", "group_type": "select",
                "members": ["my-ss", "DIRECT"], "enabled": true } ],
  "created_at": "2026-06-12T08:00:00Z",
  "updated_at": "2026-06-12T08:00:00Z"
}
```

创建请求 / Create request:

```json
{
  "name": "My Profile",
  "source_type": "clash",
  "source_url": "https://example.com/api/sub?token=abcdef",
  "enabled": true
}
```

- `source_type` ∈ `mihomo | clash | surge | loon`(MVP 仅实现 / MVP implements
  only `mihomo`/`clash`)。
- 创建时立即生成 `token`;`subscription_url` 由
  `PUBLIC_BASE_URL + public_path_prefix + token` 拼装,见 `security-design.md`。
- A `token` is generated at creation time; `subscription_url` is assembled from
  `PUBLIC_BASE_URL + public_path_prefix + token` (see `security-design.md`).
- `last_fetch_status`:最近一次机场拉取的结果分类,取值
  `success` / `http_error:<code>` / `ssrf_rejected` / `timeout` / `too_large`,
  供"原始订阅源"卡片展示;从未拉取时为 `null`。
- `last_fetch_status`: classification of the latest provider fetch —
  `success` / `http_error:<code>` / `ssrf_rejected` / `timeout` /
  `too_large`; `null` when never fetched. Displayed on the source card.

自定义节点/分组请求体 / Custom node and group request bodies:

```text
POST /api/profiles/:id/nodes
{
  "name": "my-ss",
  "node_type": "ss",
  "content": "<该节点完整 Mihomo proxy 映射的 YAML 文本>",
  "enabled": true
}

POST /api/profiles/:id/groups
{
  "name": "MyGroup",
  "group_type": "select",
  "members": ["my-ss", "DIRECT"],
  "options": { "url": "https://www.gstatic.com/generate_204", "interval": 300 },
  "enabled": true
}
```

- 节点 `content` 为完整的 Mihomo proxy 映射(YAML 文本),保存时做结构校验,
  生成时原样并入输出 `proxies`(对应 `data-model.md` 的 `custom_nodes.content`)。
- Node `content` is the complete Mihomo proxy mapping as YAML text,
  structurally validated on save and merged verbatim into the output
  `proxies` at generation time (matches `custom_nodes.content` in
  `data-model.md`).
- `PUT` 使用与 `POST` 相同的请求体,整体替换。
- `PUT` takes the same body as `POST` and replaces the resource wholesale.

## 生成与校验 / Generate and Validation

`POST /api/profiles/:id/generate` 执行完整校验,成功后刷新缓存并返回托管链接;
失败返回 `400` 和逐条错误,与 Web 弹窗文案一一对应(见 `plan.md`)。

`POST /api/profiles/:id/generate` runs full validation, refreshes the cache on
success, and returns the hosted link; on failure it returns `400` with
itemized errors matching the Web UI modal copy (see `plan.md`).

详情页"原始订阅源"卡片的手动刷新按钮**复用本端点**,不另设 refresh 端点。

The source card's manual refresh button **reuses this endpoint**; there is no
separate refresh endpoint.

`GET /api/profiles/:id/preview` 是 generate 的只读版本:有新鲜缓存时返回缓存,
否则实时拉取生成;**不**写入缓存,不影响托管链接和 `last_*` 字段。

`GET /api/profiles/:id/preview` is the read-only counterpart of generate: it
returns fresh cache when available, otherwise fetches and generates live; it
**never** persists the cache or touches the hosted link or `last_*` fields.

校验规则 / Validation rules:

- 规则引用的分组必须存在于原始订阅分组或已启用的自定义分组中。
- 自定义分组名称不得与原始订阅分组重名(MVP 采用追加策略,不覆盖)。
- 自定义分组成员必须引用存在的机场节点、机场分组或已启用的自定义节点/分组。
- 输出必须是合法的 Mihomo YAML。

&nbsp;

- Every group referenced by the rules must exist among provider groups or
  enabled custom groups.
- Custom group names must not collide with provider group names (MVP uses an
  append-only strategy, no overrides).
- Custom group members must reference existing provider nodes/groups or
  enabled custom nodes/groups.
- The output must be valid Mihomo YAML.

成功响应 / Success response:

```json
{
  "subscription_url": "https://sub.example.com/7fKp9mQx/api/sub/3w7s9xQm...",
  "generated_at": "2026-06-12T08:00:00Z"
}
```

## 公开订阅端点 / Public Subscription Endpoint

```text
GET /:public_path_prefix/api/sub/:token
  -> 200 text/yaml         有效路径 + 有效 token + 配置启用
  -> 503                   请求有效,但无任何缓存且上游拉取失败(通用响应)
  -> 404 Not Found         其余一切情况(统一响应)
```

- 缓存过期且重新拉取失败时:返回过期缓存并记录告警(见 `security-design.md`)。
- 完全无缓存且拉取失败时:返回通用 `503`,响应体不含任何上游信息。
- When the cache is stale and refresh fails: return the stale cache with a
  logged warning (see `security-design.md`).
- When no cache exists at all and the fetch fails: return a generic `503`
  whose body contains no upstream details.

成功响应头 / Success response headers:

```text
content-type: text/yaml; charset=utf-8
content-disposition: attachment; filename="<profile-name>.yaml"
subscription-userinfo: upload=...; download=...; total=...; expire=...
profile-update-interval: 24
```

- `subscription-userinfo` 从原始订阅响应透传,随生成缓存一起保存,使客户端能
  显示流量和到期信息;上游未提供时省略该头。
- `subscription-userinfo` is passed through from the provider response and
  stored with the generated cache so clients can display traffic and expiry
  info; omitted when the provider does not send it.
- `profile-update-interval`(小时)提示客户端自动更新周期,MVP 固定为 `24`。
- `profile-update-interval` (hours) hints the client auto-update period;
  fixed at `24` in the MVP.

行为 / Behavior:

- 命中新鲜缓存直接返回;缓存缺失或过期时重新拉取并生成(见
  `security-design.md` 缓存策略)。
- 响应和错误中绝不包含原始机场 URL。
- 按 token 和来源 IP 限流。

&nbsp;

- Serves fresh cache when available; refreshes on miss or staleness (see the
  cache strategy in `security-design.md`).
- Responses and errors never contain the original provider URL.
- Rate limited by token and source IP.

## 兼容性说明 / Compatibility Notes

- 现原型路由 `/api/v1/subscriptions*` 与 `/api/v1/merged` 将被本设计取代,
  不保留兼容层(项目尚未发布)。
- The prototype routes `/api/v1/subscriptions*` and `/api/v1/merged` will be
  replaced by this design with no compatibility shim (nothing is released yet).
