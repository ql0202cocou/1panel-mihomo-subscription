# API 设计 / API Design

> **状态:已实现。** 本文档描述的管理 API、认证流程、生成/预览与公开订阅端点
> 均已实现;实现中的细微取舍记录在 `docs/changelog.md`。
>
> **Status: implemented.** The management API, auth flow, generate/preview, and
> public subscription endpoint described here are implemented; minor
> implementation trade-offs are tracked in `docs/changelog.md`.

相关文档 / Related documents: `security-design.md`、`data-model.md`、
`1panel-app.md`。

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
| GET | `/api/profiles/:id/provider-rules` | 是/Yes | 拉取机场原始 `rules`(用于规则预览预填,实时拉取,不缓存) / Fetch the provider's `rules` (seeds the rule preview; live fetch, not cached) |
| GET | `/api/profiles/:id/proxies` | 是/Yes | 节点/分组预览:生成输出中的全部代理与分组(name+type,机场+自定义,只读) / Node/group preview: all proxies and groups (name+type) in the generated output (provider + custom, read-only) |
| PUT | `/api/profiles/:id/node-order` | 是/Yes | 保存**自定义块**内的节点顺序(自定义节点名数组)/ Save the node order **within the custom block** (array of custom node names) |
| PUT | `/api/profiles/:id/node-section-order` | 是/Yes | 保存两个节点块的先后(`["provider","custom"]` 的排列)/ Save the order of the two node blocks (a permutation of `["provider","custom"]`) |
| PUT | `/api/profiles/:id/group-order` | 是/Yes | 保存手动分组排序(分组名数组),决定生成 `proxy-groups` 与预览的顺序 / Save manual group ordering (array of names) driving generated `proxy-groups` and preview order |
| GET / POST | `/api/profiles/:id/nodes` | 是/Yes | 自定义节点 / Custom nodes |
| PUT / DELETE | `/api/profiles/:id/nodes/:node_id` | 是/Yes | 单个节点 / Single node |
| GET / POST | `/api/profiles/:id/groups` | 是/Yes | 自定义分组 / Custom groups |
| PUT / DELETE | `/api/profiles/:id/groups/:group_id` | 是/Yes | 单个分组 / Single group |
| POST | `/api/profiles/:id/import-provider-groups` | 是/Yes | 导入机场 `proxy-groups` 为可编辑自定义分组(实时拉取,跳过同名/不支持类型)/ Import the provider's `proxy-groups` as editable custom groups (live fetch; skips existing names / unsupported types) |
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
- `source_url` 在写入时即做静态校验:必须是 http/https、不得内嵌凭据、不得指向
  本地/私有地址(回环主机名或被封锁的字面 IP),否则返回 `400`。这是纵深防御,
  真正的 SSRF 校验仍在拉取时按 DNS 解析并钉死 IP(见 `security-design.md`)。
- `source_url` is statically validated on write: it must be http/https, carry no
  embedded credentials, and not point at a local/private address (loopback
  hostname or blocked literal IP), otherwise `400`. This is defense in depth; the
  authoritative SSRF check still runs at fetch time with DNS resolution and IP
  pinning (see `security-design.md`).
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

节点预览 / Node preview:

```text
GET /api/profiles/:id/proxies
{
  "generated": true,
  "generated_at": "2026-06-14T00:00:00Z",
  "proxies": [ { "name": "hk-1", "type": "ss" }, { "name": "my-ss", "type": "ss" } ],
  "groups": [ { "name": "Proxy", "type": "select" } ]
}
```

- 只读。`proxies` 与 `groups` 均为 `name`/`type` 对,解析自
  `generated_cache.output_yaml`,**直接返回缓存当前内容**(任何排序改动都会就地重写
  缓存,见下);未生成过时返回 `generated: false` 与空数组。响应另含
  `node_section_order`(两个节点块的先后,默认 `["provider","custom"]`)。`proxies`
  是「机场块 + 自定义块」按 `node_section_order` 拼接;前端据自定义名集合把它拆成两个
  分组渲染——机场块只读(上游序),自定义块可拖拽。`groups` 全部为自定义分组(可编辑)。
  `proxies`/`groups` 也作为自定义分组成员选择的候选。

```text
PUT /api/profiles/:id/node-order          // 自定义块内顺序(自定义节点名数组)
{ "order": ["my-ss", "other"] }           // 空数组清除(回到 created_at 序)
-> 204 No Content

PUT /api/profiles/:id/node-section-order  // 两个节点块的先后
{ "order": ["custom", "provider"] }       // 必须是 ["provider","custom"] 的排列
-> 204 No Content

PUT /api/profiles/:id/group-order         // 分组名数组
{ "order": ["MyGroup", "Other"] }
-> 204 No Content
```

- 节点预览是**两个可折叠、可拖动先后的分组**:机场分组(机场代理,**只读**上游序,名称
  为机场名)与自定义分组(可拖动组内顺序)。`node-order` 存自定义块顺序、
  `node-section-order` 存两块先后(均见 `data-model.md`);`order` 中存在的名字优先排列,
  未列出的新自定义节点落末尾;名字超长/数组过大返回 `400`。分组预览仍由 `group-order`
  排序、分组均为自定义(可编辑)。这三个端点保存后都会**就地重写已生成缓存
  (`generated_cache.output_yaml`)、无需重新拉取机场**,故改动**立即生效**(预览与公共
  订阅链接随即反映);无缓存时在首次生成时生效。鉴权与同源校验同其他管理接口。
  此外,**每次生成会把输出里的自定义节点顺序与分组顺序快照回写 `node_order`/`group_order`**:
  故新增的自定义节点/分组持久化到各自块的末尾;机场块顺序始终上游序、不快照。
  Additionally, **each generation snapshots the output's custom-node order and
  group order back into `node_order`/`group_order`**, so newly added custom
  nodes/groups persist at the end of their block; the provider block's order is
  always upstream and is not snapshotted.
- Read-only. `proxies` and `groups` are `name`/`type` pairs parsed from
  `generated_cache.output_yaml` and **returned as the cache currently stands**
  (any reorder re-stitches the cache in place, below); before the first
  generation it returns `generated: false` and empty arrays. The response also
  carries `node_section_order` (the two node blocks' order, default
  `["provider","custom"]`). `proxies` is the provider block + custom block
  concatenated per `node_section_order`; the frontend splits it into two groups
  by the custom-name set — the provider block is read-only (upstream order), the
  custom block is sortable. `groups` are all custom (editable). `proxies`/`groups`
  also seed the custom-group member suggestions.
- The node preview is **two collapsible, drag-orderable groups**: the provider
  group (provider proxies, **read-only** upstream order, titled with the provider
  name) and the custom group (its nodes are sortable). `node-order` stores the
  custom block order and `node-section-order` stores the two blocks' order (both
  in `data-model.md`); listed names go first, newly added custom nodes fall to
  the end; over-long names / over-large arrays return `400`. The group preview is
  ordered by `group-order` and every group is custom (editable). All three
  endpoints re-stitch the generated cache (`generated_cache.output_yaml`) in
  place on save **without re-fetching the provider**, so changes take effect
  **immediately** (preview and public link reflect them right away); with no
  cache yet it applies on the first generate. Auth and same-origin checks match
  the other management endpoints.
- 规则预览同样支持拖拽排序,但规则顺序本身具有语义(自上而下命中即止),且
  `rulesets.content` 本就是有序文本,因此无需新增列:前端拖动后直接把重排后的规则
  行经 `PUT /api/profiles/:id/rules` 整体保存。该端点保存后同样会就地重写已生成缓存
  的 `rules` 块(规则与机场无关,可独立重建),因此排序(以及增删改)**立即生效**,
  无需重新拉取机场。
- The rule preview supports drag-and-drop sorting too, but rule order is itself
  semantic (top-down, first match wins) and `rulesets.content` is already an
  ordered text, so no new column is needed: the frontend reorders the rule lines
  and saves the whole content via `PUT /api/profiles/:id/rules`. On save that
  endpoint also rewrites the `rules` block of the generated cache in place (rules
  are provider-independent and can be rebuilt alone), so reordering — and any
  add/edit/delete — **takes effect immediately**, with no provider re-fetch.
- Read-only. Both `proxies` and `groups` are `name`/`type` pairs parsed from
  `generated_cache.output_yaml`; before the first generation it returns
  `generated: false` and empty arrays. `proxies` contain provider (read-only) and
  custom (editable) entries, which the frontend distinguishes via the custom name
  set; `groups` are all custom groups (provider groups are replaced), so every
  group in the preview is editable. The node and group previews share one
  interaction, and `proxies`/`groups` also seed the custom-group member
  suggestions.
- 编辑自定义节点与自定义分组均通过结构化表单完成:节点给出常用字段 + 高级键值;
  分组按类型给出选项(`url`/`interval`/`tolerance`/`lazy`/`strategy`)+ 高级键值,
  成员从候选下拉中选择。前端保存时分别序列化为节点 `content` 的 Mihomo proxy YAML
  与分组 `options` 的 JSON 对象。
- Both custom nodes and custom groups are edited through structured forms: nodes
  expose common fields plus advanced key/value rows; groups expose per-type
  options (`url`/`interval`/`tolerance`/`lazy`/`strategy`) plus advanced rows,
  with members chosen from suggestions. The frontend serializes these to the
  node `content` Mihomo proxy YAML and the group `options` JSON object on save.

## 生成与校验 / Generate and Validation

`POST /api/profiles/:id/generate` 执行完整校验,成功后刷新缓存并返回托管链接;
失败返回 `400` 和逐条错误,与 Web 弹窗文案一一对应。

`POST /api/profiles/:id/generate` runs full validation, refreshes the cache on
success, and returns the hosted link; on failure it returns `400` with
itemized errors matching the Web UI modal copy.

详情页"原始订阅源"卡片的手动刷新按钮**复用本端点**,不另设 refresh 端点。

The source card's manual refresh button **reuses this endpoint**; there is no
separate refresh endpoint.

`GET /api/profiles/:id/preview` 是 generate 的只读版本:有新鲜缓存时返回缓存,
否则实时拉取生成;**不**写入缓存,不影响托管链接和 `last_*` 字段。

`GET /api/profiles/:id/preview` is the read-only counterpart of generate: it
returns fresh cache when available, otherwise fetches and generates live; it
**never** persists the cache or touches the hosted link or `last_*` fields.

校验规则 / Validation rules:

- 规则引用的分组必须存在于已启用的自定义分组中(机场分组不在输出里,除非已导入为
  自定义分组)。
- 自定义分组名称可与机场分组重名(机场分组被整体替换,「导入机场分组」正是这样生成
  同名自定义分组)。
- 自定义分组成员必须引用存在的机场节点(仍透传)、已启用的自定义节点,或已启用的
  自定义分组。
- 输出必须是合法的 Mihomo YAML。

&nbsp;

- Every group referenced by the rules must exist among enabled custom groups
  (provider groups are not in the output unless imported as custom groups).
- A custom group name may reuse a provider group name (provider groups are
  replaced; importing provider groups produces exactly such same-named customs).
- Custom group members must reference existing provider proxies (still passed
  through), enabled custom nodes, or enabled custom groups.
- The output must be valid Mihomo YAML.

顶层键处理 / Top-Level Key Handling:

转换器对拉取到的机场配置的每个顶层键都显式处理(实现见 `src/converter.rs`):

The converter treats every top-level key of the fetched provider config
explicitly (implemented in `src/converter.rs`):

| 键 / Key | 处理 / Handling |
|-----|----------|
| `proxies` | 机场块(机场代理,上游序)+ 自定义块(启用的自定义节点,按 `node_order` 排),按 `node_section_order` 拼接 / Provider block (provider proxies, upstream order) + custom block (enabled custom nodes ordered by `node_order`), concatenated per `node_section_order` |
| `proxy-groups` | 整体替换为启用的自定义分组(机场分组不透传,需「导入机场分组」),再按 `group_order` 重排 / Replaced entirely with enabled custom groups (provider groups are not passed through; import them), then reordered by `group_order` |
| `rules` | 整体替换为用户规则 / Replaced entirely with the user-defined rules |
| `rule-providers` | 原样透传(用户规则可引用机场的 `RULE-SET`)/ Passed through unchanged (user rules may reference provider `RULE-SET`s) |
| `proxy-providers` | **MVP 阶段剥离**:远程节点提供者会让客户端拉取绕过本服务 SSRF 防护与缓存的 URL,并可能暴露机场 URL / **Stripped in the MVP**: remote node providers would make the client fetch URLs that bypass this service's SSRF protection and caching, and may expose provider URLs |
| 其余 (`port`、`dns`、`tun`、`sniffer`…) / All others | 原样透传 / Passed through unchanged |

未知键透传而非丢弃,使新的 Mihomo 选项无需改转换器即可继续工作。

Unknown keys are passed through rather than dropped, so newer Mihomo options
keep working without converter updates.

`proxy-groups` 与 `rules` 同为「替换」模型:机场原生分组不再进入输出,机场更新分组
也不会自动生效。`POST /api/profiles/:id/import-provider-groups` 实时拉取机场订阅,把
其 `proxy-groups` 解析为**可编辑的自定义分组**写入(`name`/`type`/`proxies`→成员,其余
键→`options`;跳过同名与不支持类型),返回 `{ imported, skipped }`。导入只改
`custom_groups`,与新增自定义分组一样需重新「生成」才进入输出。鉴权与 SSRF 防护同
`provider-rules`。

Like `rules`, `proxy-groups` is a "replace" model: the provider's own groups no
longer enter the output, and provider group updates never apply automatically.
`POST /api/profiles/:id/import-provider-groups` live-fetches the provider and
imports its `proxy-groups` as **editable custom groups** (`name`/`type`/`proxies`
→ members, the rest → `options`; skipping existing names and unsupported types),
returning `{ imported, skipped }`. Import only writes `custom_groups`, so — like
adding a custom group — it reaches the output on the next generate. Auth and SSRF
protection match `provider-rules`.

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

- **每次拉取都实时重拉机场并重新生成**,使客户端永远拿到机场最新节点;不再有
  「缓存新鲜就直接返回」的 TTL 短路。并发拉取由 per-profile single-flight 合并为
  一次机场拉取(后到的请求若发现缓存已在本批刷新过,直接复用)。`generated_cache`
  现仅作**机场拉取失败时的兜底**(返回上一份缓存,无则 `503`);`CACHE_TTL_MINUTES`
  现只影响管理端 `preview`,不影响本端点。
- 「生成配置」按钮已移除——公共链接始终实时,无需手动生成;管理端预览里的机场节点
  通过「原始订阅源 → 刷新」(`POST /generate`)更新。
- 响应和错误中绝不包含原始机场 URL。
- 按 token 和来源 IP 限流(配合 single-flight 合并,约束机场侧负载)。

&nbsp;

- **Every pull re-fetches the provider and regenerates**, so the client always
  gets the latest provider nodes; there is no "serve fresh cache within TTL"
  short-circuit. Concurrent pulls are coalesced into one provider fetch by the
  per-profile single-flight (a later request reuses the cache if it was already
  refreshed in this batch). `generated_cache` is now only a **fallback when the
  provider fetch fails** (serve the previous cache, else `503`);
  `CACHE_TTL_MINUTES` now affects only the admin `preview`, not this endpoint.
- The "generate" button is gone — the public link is always live, no manual
  generate needed; provider nodes in the admin preview are refreshed via
  "source → refresh" (`POST /generate`).
- Responses and errors never contain the original provider URL.
- Rate limited by token and source IP (with single-flight coalescing, this bounds
  the load on the provider).

## 兼容性说明 / Compatibility Notes

- 早期原型的 `/api/v1/subscriptions*` 与 `/api/v1/merged` 路由已移除,
  无兼容层。
- The prototype's `/api/v1/subscriptions*` and `/api/v1/merged` routes have
  been removed with no compatibility shim.
