# 架构参考

> API 契约、数据模型、安全设计——后端实现的权威参考。已实现，取舍见 `changelog.md`。

---

## API 设计

### 原则

- 管理 API 挂 `/api`、需登录会话、用 JSON；公开订阅端点无需登录（需随机路径前缀 +
  per-profile token）、输出 Mihomo YAML。
- 公开端点任何校验失败统一 `404`（不泄因）；管理 API 默认对机场 URL 脱敏。
- 管理 API **同源**（SPA 由 Axum 同源提供，不启用 CORS；见安全设计节）。
- 管理请求体 ≤1MB（超限 `413`）；管理员提交的节点/分组 YAML 与机场内容用相同别名/嵌套限制解析。

### 约定

- 时间 RFC 3339 UTC；标识符 UUID v4；编码 `application/json`（公开端点 `text/yaml`）。

错误响应：`{ "error": { "code", "message", "details": [...] } }`。

| Code | 含义 |
|------|------|
| 200/201/204 | 成功（读/建/无内容） |
| 400 | 请求体格式错或校验失败 |
| 401 | 未登录/会话过期 |
| 404 | 资源不存在；公开端点统一失败响应 |
| 409 | 名称冲突（如重名） |
| 413 | 请求体超限 |
| 429 | 限流 |
| 500 | 内部错误（不含敏感信息） |

### 认证

凭据来自 `ADMIN_USERNAME` / `ADMIN_PASSWORD`（compose 环境变量）。登录签发会话 Cookie
（`HttpOnly`、`SameSite=Lax`，HTTPS 加 `Secure`）；登录失败按 IP + 账户限流。除 `/health`、
登录、公开端点外，所有路由需有效会话（否则 `401`）。所有状态变更请求（含登录/登出）必须带同源
`Origin`；生产环境按 `PUBLIC_BASE_URL` 的完整 origin（scheme + host + port）校验，否则 `403`。

```text
POST /api/auth/login  {username,password} -> 204+Set-Cookie | 401 | 429
POST /api/auth/logout -> 204 清除 cookie
GET  /api/auth/session -> 200 {username} | 401
```

### 端点总览

| Method | Path | 鉴权 | 说明 |
|--------|------|------|------|
| GET | `/health` | 否 | 健康检查 |
| POST | `/api/auth/login` | 否 | 登录 |
| POST | `/api/auth/logout` | 是 | 登出 |
| GET | `/api/auth/session` | 是 | 当前会话 |
| GET / POST | `/api/profiles` | 是 | 配置列表 / 创建 |
| GET / PUT / DELETE | `/api/profiles/:id` | 是 | 配置详情 / 更新 / 删除 |
| PUT | `/api/profiles/:id/rules` | 是 | 替换自定义规则 |
| GET | `/api/profiles/:id/provider-rules` | 是 | 拉取机场原始 `rules`（实时，不缓存） |
| GET | `/api/profiles/:id/proxies` | 是 | 节点/分组预览（生成输出中的代理与分组，只读） |
| PUT | `/api/profiles/:id/node-section-order` | 是 | 两个节点块的先后（`["provider","custom"]` 排列） |
| PUT | `/api/profiles/:id/group-order` | 是 | 分组顺序（分组名数组） |
| GET / POST | `/api/global-nodes` | 是 | 全局自定义节点池（跨订阅共享，自动追加到每条配置） |
| PUT / DELETE | `/api/global-nodes/:id` | 是 | 单个全局节点 |
| PUT | `/api/global-nodes/order` | 是 | 全局自定义块顺序；立即重排所有配置缓存 |
| GET / POST | `/api/rule-sets` | 是 | 全局用户规则库（② 模板 / 导入源；不托管、不参与生成） |
| PUT / DELETE | `/api/rule-sets/:id` | 是 | 单个全局规则集 |
| PUT | `/api/rule-sets/order` | 是 | 全局库显示顺序（仅展示序） |
| GET / POST | `/api/profiles/:id/rule-sets` | 是 | 订阅自有规则库（③）；生成只读此处 |
| PUT / DELETE | `/api/profiles/:id/rule-sets/:rsid` | 是 | 单个订阅规则集 |
| POST | `/api/profiles/:id/rule-sets/import` | 是 | 从全局 ② 复制规则集进本订阅 ③ + 追加 `RULE-SET` 规则行 |
| GET / POST | `/api/profiles/:id/groups` | 是 | 自定义分组 |
| PUT / DELETE | `/api/profiles/:id/groups/:group_id` | 是 | 单个分组 |
| POST | `/api/profiles/:id/import-provider-groups` | 是 | 导入机场 `proxy-groups` 为可编辑自定义分组（实时，跳过同名/不支持类型） |
| GET | `/api/profiles/:id/preview` | 是 | 预览生成的 YAML |
| POST | `/api/profiles/:id/generate` | 是 | 校验并生成 |
| POST | `/api/profiles/:id/reset-token` | 是 | 重置该配置 token |
| GET | `/api/settings` | 是 | 应用设置 |
| POST | `/api/settings/reset-public-path` | 是 | 重置公共路径前缀 |
| GET | `/:public_path_prefix/api/sub/:token` | 否 | 公开订阅下载 |
| GET | `/:public_path_prefix/api/sub/:token/r/:name/:file` | 否 | 公开规则集托管（③，按订阅 token 隔离；`:file` = `<behavior>.<format>`） |

### Profile 资源

列表返回摘要，详情返回完整对象。`source_url` 默认脱敏，仅创建/更新接受完整值。详情的
`nodes` 是**全局节点池的只读快照**（各配置一致，编辑/排序走 `/api/global-nodes`），供详情
展示与分组/规则引用建议。

```json
{
  "id": "...", "name": "My Profile",
  "source_url_masked": "https://example.com/api/sub?token=***",
  "output_type": "mihomo",
  "subscription_url": "https://sub.example.com/<prefix>/api/sub/<token>",
  "last_generated_at": "...", "last_fetch_at": "...", "last_fetch_status": "success",
  "rules": { "content": "...\nMATCH,Proxy", "updated_at": "..." },
  "nodes": [ { "id": "...", "name": "my-ss", "node_type": "ss", "enabled": true } ],
  "groups": [ { "id": "...", "name": "MyGroup", "group_type": "select",
               "members": ["my-ss","DIRECT"], "enabled": true } ],
  "created_at": "...", "updated_at": "..."
}
```

- 创建：`{name, source_url}`。创建成功后**同步触发一次生成/拉取**（尽力而为），故新订阅
  立即带有真实 `last_fetch_status`，不存在「未拉取」中间态。
- `source_url` 写时静态校验（http/https、无内嵌凭据、非本地/私有地址），否则 `400`；真正 SSRF
  在拉取时按 DNS 解析 + IP 固定。
- `last_fetch_status`：`success` / `http_error:<code>` / `ssrf_rejected` / `timeout` / `too_large`，
  从未拉取为 `null`。

请求体（节点走全局池，分组按配置）：

```text
POST /api/global-nodes { name, node_type, content, enabled? }  # name 全局唯一（重名 409）；content 为完整 Mihomo proxy 映射 YAML
POST /api/profiles/:id/groups { name, group_type, members, options?, enabled? }
```

- 节点 `content` 保存时结构校验，生成时原样并入**每条配置**输出的 `proxies`；`PUT` 同体整体替换。
- 全局节点为单一共享池：新建落末尾、`name` 全局唯一；增删改在下次生成（公共链接每拉取即重生）
  进入各配置输出，排序见下立即生效。

### 规则集库（三规则库模型）

规则下发与订阅**解耦**为三个库：① 机场原始规则（`GET .../provider-rules`，「导入机场规则」）、
② 全局用户规则库（`/api/rule-sets`）、③ 每订阅托管规则库（`/api/profiles/:id/rule-sets`）。**下发只读
③**；①② 仅作导入源。

#### ② 全局用户规则库（模板 / 导入源）

```text
GET    /api/rule-sets                        # 列表；无托管链接
POST   /api/rule-sets   { name, behavior, source?, format, ... }
   # name 全局唯一，限 [A-Za-z0-9._-]；source∈manual/remote
   # manual: { content }  format∈yaml/text
   # remote: { url, interval_hours?, cache? }  format∈yaml/text/mrs
PUT    /api/rule-sets/:id   { ...同上 }
DELETE /api/rule-sets/:id
PUT    /api/rule-sets/order { order: [规则集名] }  # 仅展示序
```

② 仅作可复用模板，通过 ③ 的导入端点复制进某订阅后才生效。

#### ③ 每订阅托管规则库（下发来源）

```text
GET    /api/profiles/:id/rule-sets              # 列表；含托管链接、last_fetch_status
POST   /api/profiles/:id/rule-sets   { name, behavior, source?, format, ... }  # 字段同 ②，name 订阅内唯一
PUT    /api/profiles/:id/rule-sets/:rsid
DELETE /api/profiles/:id/rule-sets/:rsid
POST   /api/profiles/:id/rule-sets/import  { names: [②规则集名], policy }  # 复制 ②→③ + 追加 RULE-SET 行
```

- manual / remote 行为与 ② 一致（共用 `src/rulelib.rs`）。托管在按订阅 token 隔离的链接
  `/<prefix>/api/sub/<token>/r/<name>/<behavior>.<format>`；remote 关缓存则直接注入上游 URL。
- 被 `RULE-SET,<name>` 引用时注入指向托管链接的 `rule-providers:` 条目（定义改动即时生效）。
- 撞名机场 `rule-providers` 时本订阅托管版覆盖机场版；生成端点响应 `ruleset_conflicts` 列出撞名。

### 节点/分组预览与排序

```text
GET /api/profiles/:id/proxies
{ "generated": true, "generated_at": "...",
  "proxies": [{ "name":"hk-1","type":"ss" }, { "name":"my-ss","type":"ss" }],
  "node_section_order": ["provider","custom"],
  "groups": [{ "name":"Proxy","type":"select" }] }
```

只读，解析自 `generated_cache.output_yaml`；未生成返回 `generated:false` + 空数组。`proxies` =
机场块 + 自定义块按 `node_section_order` 拼接；前端据全局节点名集合拆两块渲染。`groups` 全为
自定义分组，两者也作分组成员候选。

```text
PUT /api/global-nodes/order               { order: [节点名] }              # 全局
PUT /api/profiles/:id/node-section-order  { order: ["custom","provider"] } # per-profile
PUT /api/profiles/:id/group-order         { order: [分组名] }              # per-profile
-> 均 204
```

- 自定义块序由全局 `global-nodes/order` 决定；两块先后由 per-profile `node-section-order`；
  分组序由 per-profile `group-order`。
- 保存后**就地重写已生成缓存**，改动立即生效；全局排序重排**每条配置**缓存。
- 规则拖拽同理：`PUT .../rules` 整体保存，就地重写缓存 `rules` 块。
- 每次生成快照分组序回写 `group_order`（新增落末尾）；节点序全局 `global_nodes.position`，
  机场块恒上游序。
- 节点/分组均结构化表单录入，前端序列化为节点 `content` YAML 与分组 `options` JSON。

### 生成与校验

- `POST .../generate`：完整校验，成功刷新缓存并返回托管链接，失败 `400` + 逐条错误。
  详情页「原始订阅源」手动刷新复用本端点。
- `GET .../preview`：只读版，有新鲜缓存则返回，否则实时拉取生成；不写缓存。
- 校验要点：规则引用分组须存在；分组成员须引用存在的节点/分组；输出须合法 Mihomo YAML。

顶层键处理（转换器逐键显式处理）：

| 键 | 处理 |
|-----|------|
| `proxies` | 机场块（上游序）+ 自定义块（启用全局节点按 `global_nodes.position` 排），按 `node_section_order` 拼接 |
| `proxy-groups` | 整体替换为启用的自定义分组（机场分组不透传，需「导入机场分组」），按 `group_order` 重排 |
| `rules` | 整体替换为用户规则 |
| `rule-providers` | 机场原样透传；另把被 `RULE-SET` 引用、本订阅自有（③）规则集合并在上（同名覆盖，指向按订阅 token 隔离的托管链接） |
| `proxy-providers` | **剥离**（会让客户端拉取绕过 SSRF/缓存的 URL，可能暴露机场 URL） |
| 其余（`port`/`dns`/`tun`/`sniffer`…） | 原样透传（新 Mihomo 选项无需改转换器） |

- `import-provider-groups`：实时拉取机场 `proxy-groups` 解析为可编辑自定义分组
  （跳过同名/不支持类型），返回 `{imported,skipped}`；需重新生成才进输出。
- 规则集：按 `RULE-SET,<name>,<policy>` 注入本订阅自有规则库（③）中同名条目，指向按订阅
  token 隔离的托管链接；全局 ② 库不参与生成。

### 公开订阅端点

```text
GET /:public_path_prefix/api/sub/:token
  -> 200 text/yaml   有效前缀 + token + 配置启用
  -> 503             有效但无缓存且上游拉取失败（通用）
  -> 404             其余一切（统一）
```

响应头：`content-type: text/yaml`、`content-disposition: attachment; filename="<name>.yaml"`、
`subscription-userinfo`（从机场透传，无则省略）、`profile-update-interval: 24`（小时，MVP 固定）。

- 公共拉取在 `PUBLIC_REFRESH_MIN_SECONDS`（默认 30s）内复用缓存，下限外回源重新生成；
  并发由 per-profile single-flight 合并。拉取失败用旧缓存兜底（无则 `503`）。
  响应/错误绝不含机场 URL；按 token + 来源 IP 限流。

兼容：早期 `/api/v1/subscriptions*`、`/api/v1/merged` 已移除，无兼容层。

### 公开规则集托管端点

```text
GET /:public_path_prefix/api/sub/:token/r/:name/:file
  -> 200 text/plain | application/octet-stream(mrs)
  -> 503             remote 镜像首拉失败且无缓存
  -> 404             其余一切（统一）
```

- 托管本订阅自有规则库（③），按订阅 token 隔离，与订阅端点共用 `public_path_prefix`。
- manual：`yaml`→Mihomo `payload:` 列表、`text`→逐行（忽略空行与 `#` 注释）。
  remote（`cache=1`）：single-flight 懒刷新，缓存超 `interval_hours` 才回源（SSRF 安全），
  失败回退旧缓存；字节原样托管（`mrs` 为 `application/octet-stream`）。
  remote（`cache=0`）不托管，统一 `404`。

---

## 数据模型

> SQLite 模式由 `migrations/` 实现；连接池按本文档对**每个**连接应用 pragma。

### 存储约定

- DB：`${DATA_DIR}/mihomo-subscription.db`（`DATA_DIR` 默认 `/data`）。
- 每连接 pragma：`foreign_keys = ON`、`busy_timeout = 5000`（须在连接池 after-connect 钩子对
  **每个**连接设置）；`journal_mode = WAL` 设一次随文件持久化。
- 主键 UUID v4（`TEXT`）；时间戳 RFC 3339 UTC（`TEXT`）；布尔 `INTEGER` 0/1；结构化字段 JSON 存 `TEXT`。

### 实体关系

```text
app_settings (单行)
global_nodes (全局池，不挂任何 profile)

profiles 1 ── 1 rulesets
profiles 1 ── * custom_groups
profiles 1 ── 1 generated_cache
```

`global_nodes` 是**跨订阅共享**的自定义节点池，自动追加到每条 profile 输出，不与 profile 外键
关联（删 profile 不影响它）。

### 表定义

#### app_settings

应用级设置；`public_path_prefix` 支持运行时重置（故存库），首启时空则从 `PUBLIC_PATH_PREFIX`
初始化，否则随机生成。

```sql
CREATE TABLE app_settings (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    public_path_prefix  TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);
```

#### profiles

```sql
CREATE TABLE profiles (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL,
    source_url  TEXT    NOT NULL,
    output_type TEXT    NOT NULL DEFAULT 'mihomo',
    token       TEXT    NOT NULL,
    last_fetch_at     TEXT,
    last_fetch_status TEXT,
    node_order  TEXT,
    node_section_order TEXT,
    group_order TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE UNIQUE INDEX idx_profiles_token ON profiles (token);
CREATE UNIQUE INDEX idx_profiles_name  ON profiles (name);
```

- `token`：≥32 随机字节 URL-safe，明文存储以展示完整链接。
- `last_fetch_*`：最近机场拉取观测（`success`/`http_error:<code>`/`ssrf_rejected`/`timeout`/`too_large`）。
- 输出 `proxies` = 机场块（上游序）+ 自定义块（`global_nodes.position`），按 `node_section_order` 拼接。
- `node_order`：**已弃用**（0007 起恒 NULL），由 `global_nodes.position` 替代（0002）。
- `node_section_order`：两块先后排列，JSON 数组；per-profile（0004）。
- `group_order`：分组序，生成时快照回写；新增落末尾（0003）。

#### rulesets

每 profile 一份规则文本（`UNIQUE(profile_id)`）；保留 `priority`/`name` 备扩展。

```sql
CREATE TABLE rulesets (
    id          TEXT    PRIMARY KEY,
    profile_id  TEXT    NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    name        TEXT    NOT NULL DEFAULT 'default',
    content     TEXT    NOT NULL,
    priority    INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT    NOT NULL
);

CREATE UNIQUE INDEX idx_rulesets_profile ON rulesets (profile_id);
```

#### global_nodes

**全局自定义节点池（跨订阅共享，0007）。** 自动追加到每条 profile 输出；编辑/排序统一
在「节点配置」页（`/api/global-nodes`），详情页只读。

```sql
CREATE TABLE global_nodes (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    node_type   TEXT    NOT NULL,
    content     TEXT    NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE INDEX idx_global_nodes_position ON global_nodes (position);
```

- `name` 全局唯一；`node_type` 不加 CHECK 免迁移；`content` 为完整 Mihomo proxy 映射。
- `position`：全局自定义块顺序（`ORDER BY position, name`）；新建取 `MAX+1`，
  `PUT /api/global-nodes/order` 重写为 `0..n-1` 并即时重排所有 profile 缓存。
- 迁移：原各 profile `custom_nodes` 按 `name` 去重合并进本表，随后 `DROP TABLE custom_nodes`。

#### rule_sets

**全局用户规则库 / 导入源（②，0008）。** 仅作导入模板，不再公开托管、不参与生成。
订阅通过「导入托管规则」把 ② 条目复制进 `profile_rule_sets`（③）后生效。② 表保留
`url`/`cached_*` 列但不再镜像（导入到 ③ 后由 ③ 镜像）。

```sql
CREATE TABLE rule_sets (
    id                TEXT    PRIMARY KEY,
    name              TEXT    NOT NULL UNIQUE,
    behavior          TEXT    NOT NULL CHECK (behavior IN ('domain', 'ipcidr', 'classical')),
    format            TEXT    NOT NULL CHECK (format IN ('yaml', 'text', 'mrs')),
    source            TEXT    NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'remote')),
    content           TEXT    NOT NULL DEFAULT '',  -- manual payload（每行一条）
    rule_count        INTEGER NOT NULL DEFAULT 0,   -- 列表展示用，免读 BLOB
    url               TEXT,                          -- remote 上游 URL
    interval_hours    INTEGER NOT NULL DEFAULT 24,
    cache             INTEGER NOT NULL DEFAULT 1,    -- remote 是否本地二次托管
    cached_body       BLOB,                          -- 镜像字节（text/yaml/mrs 二进制）
    cached_at         TEXT,
    last_fetch_status TEXT,
    enabled           INTEGER NOT NULL DEFAULT 1,
    position          INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL,
    updated_at        TEXT    NOT NULL
);

CREATE INDEX idx_rule_sets_position ON rule_sets (position);
```

- `name` 全局唯一，限定 `[A-Za-z0-9._-]`。
- **manual**：`content` 为 payload；托管时 `yaml`→`payload:` 列表、`text`→逐行；format 限 `yaml`/`text`。
- **remote**：`cache=1` 按 `interval_hours` 懒拉取（SSRF 安全）、镜像到 `cached_body` 二次托管，失败回退旧缓存；`cache=0` 不托管，转换时直接注入上游 URL。更新规则集会清空缓存列。
- `position`：仅展示顺序（`ORDER BY position, name`）。

#### profile_rule_sets

**每订阅自包含规则库（③，0011）。** 镜像 `rule_sets` 字段，按 `profile_id` 隔离、去
`position`（rule-providers 是 map）。托管在按订阅 token 隔离的链接
`/<prefix>/api/sub/<token>/r/<name>/<behavior>.<format>`，不同订阅可复用同名。

```sql
CREATE TABLE profile_rule_sets (
    id                TEXT    PRIMARY KEY,
    profile_id        TEXT    NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    name              TEXT    NOT NULL,
    behavior          TEXT    NOT NULL CHECK (behavior IN ('domain', 'ipcidr', 'classical')),
    format            TEXT    NOT NULL CHECK (format IN ('yaml', 'text', 'mrs')),
    source            TEXT    NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'remote')),
    content           TEXT    NOT NULL DEFAULT '',
    rule_count        INTEGER NOT NULL DEFAULT 0,
    url               TEXT,
    interval_hours    INTEGER NOT NULL DEFAULT 24,
    cache             INTEGER NOT NULL DEFAULT 1,
    cached_body       BLOB,
    cached_at         TEXT,
    last_fetch_status TEXT,
    enabled           INTEGER NOT NULL DEFAULT 1,
    created_at        TEXT    NOT NULL,
    updated_at        TEXT    NOT NULL,
    UNIQUE (profile_id, name)
);

CREATE INDEX idx_profile_rule_sets_profile ON profile_rule_sets (profile_id);
```

- `name` 在单个订阅内唯一（`UNIQUE(profile_id, name)`），限定 `[A-Za-z0-9._-]`。
- manual / remote 行为与 `rule_sets` 完全一致（共用 `src/rulelib.rs`）；区别是托管链接含订阅 token。
- 「导入托管规则」从 ② 复制条目进本表（后端复制真实 URL，前端只见脱敏值）。

#### custom_groups

输出 `proxy-groups` 的**唯一来源**（转换器整体替换机场分组；经 `import-provider-groups` 落为自定义分组才可编辑入输出）。

```sql
CREATE TABLE custom_groups (
    id          TEXT    PRIMARY KEY,
    profile_id  TEXT    NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    group_type  TEXT    NOT NULL CHECK (group_type IN
                    ('select','url-test','fallback','load-balance','relay')),
    members     TEXT    NOT NULL,
    options     TEXT,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    UNIQUE (profile_id, name)
);

CREATE INDEX idx_custom_groups_profile ON custom_groups (profile_id);
```

- `members`：有序 JSON 数组，可引用机场节点（透传）/自定义节点/分组；引用有效性在生成时校验。
  `options`：类型特有选项 JSON（如 `{"url":"...","interval":300}`）。

> **已移除 rule_providers 托管：** 0005/0006 删除了 `rule_providers` 表。转换器只透传机场自带
> `rule-providers:`；规则仍可 `RULE-SET,<name>,<policy>` 引用机场条目名。

#### generated_cache

```sql
CREATE TABLE generated_cache (
    profile_id            TEXT PRIMARY KEY REFERENCES profiles (id) ON DELETE CASCADE,
    content_hash          TEXT NOT NULL,
    output_yaml           TEXT NOT NULL,
    subscription_userinfo TEXT,
    generated_at          TEXT NOT NULL
);
```

- 每 profile 仅留最新一份。公共端点 `PUBLIC_REFRESH_MIN_SECONDS`（默认 30s）内复用缓存，
  下限外回源重新生成；拉取失败兜底。`CACHE_TTL_MINUTES`（默认 15）仅管理端 `preview`。
- `subscription_userinfo`：机场响应头原文透传。`content_hash`：输入+机场内容哈希，跳过无变化重复生成。

### 迁移

- `sqlx::migrate!`，文件 `migrations/NNNN_description.sql`，启动自动执行；已应用文件不可改，只追加。
- `0001` 直接建全表（无生产数据，丢弃原型 `subscriptions`）。SQLite `ALTER` 受限，改列用
  「建新表 → 拷贝 → 改名」。

---

## 安全设计

自托管于 1Panel，默认安全：不泄露机场秘密、不被当内网扫描器、链接不易枚举、管理面全程
鉴权、错误/日志不含秘密。

**信任边界（区别对待）：** ① 管理员浏览器 → Web UI / 管理 API：需认证；② 公共客户端 →
订阅端点：无登录，需随机路径前缀 + per-profile token；③ 后端 → 机场 URL：每次出站获取受
SSRF 保护。

### 公共链接

```text
https://<PUBLIC_BASE_URL>/<PUBLIC_PATH_PREFIX>/api/sub/<profile_token>
```

- `PUBLIC_PATH_PREFIX` 随机 16-24 字符；`profile_token` ≥32 随机字节、每配置独立；链接不含
  库 ID 或机场 URL。
- 放行 = 前缀匹配 **且** token 存在 **且** 配置启用；否则一律 `404`（不透露哪步失败）。
- **防时序侧信道**：无论前缀是否匹配都执行 token 查找，前缀恒定时间比较；规则集托管端点同样先
  查 token 再判定前缀。
- **规则集托管** `…/<PUBLIC_PATH_PREFIX>/api/sub/<profile_token>/r/<name>/<behavior>.<format>`
  共用同一前缀与 profile token（重置任一秘密均使其失效）。规则集内容是规则清单、非私密，按名可枚举
  可接受；仍按源 IP 限流。`name` 限 `[A-Za-z0-9._-]`，杜绝路径穿越。

### Token 轮换

重置单配置 token、重置全局 `PUBLIC_PATH_PREFIX`（使所有链接失效）均支持；机场变化时链接保持
稳定，除非显式重置。

### 管理员认证

- 凭据来自 `ADMIN_USERNAME` / `ADMIN_PASSWORD`，未设置拒绝启动；恒定时间比较；登录失败按 IP + 账户限流。
- 会话 Cookie：≥128 位 CSPRNG ID、`HttpOnly` + `SameSite=Lax`、HTTPS 加 `Secure`；存内存（重启失效），空闲超时默认 7 天。TLS 终止代理后需显式 `SECURE_COOKIES=true`。
- **不启用 CORS 层**（SPA 同源）；状态变更请求必须带同源 `Origin`，按 `PUBLIC_BASE_URL` 校验（缺失/不匹配 `403`）。

### SSRF 保护

所有出站获取走单一保护获取器（含规则集远程镜像的 `fetch_bytes` 字节路径）。

- 仅 `http`/`https`；拒空主机、内嵌凭据、`localhost` 回环名、阻止段裸 IP。
- 解析域名 → 检查解析 IP → **连接时固定该 IP**（防 DNS 重绑定 TOCTOU）；每重定向同规则重查，上限 3。
- IPv6 内嵌 IPv4（`::ffff:0:0/96`、`64:ff9b::/96`、`2002::/16`）须解包出 IPv4 再按 IPv4 段查。
- 出站限制：连接超时 5-10s、总超时 10-20s、最大响应 5-10MB（按流字节计）、重定向 ≤3。

阻止 IPv4：`0.0.0.0/8 10.0.0.0/8 100.64.0.0/10 127.0.0.0/8 169.254.0.0/16 172.16.0.0/12
192.0.0.0/24 192.0.2.0/24 192.88.99.0/24 192.168.0.0/16 198.18.0.0/15 198.51.100.0/24
203.0.113.0/24 224.0.0.0/4 240.0.0.0/4`
阻止 IPv6：`::/128 ::1/128 ::ffff:0:0/96 64:ff9b::/96 2002::/16 fc00::/7 fe80::/10 ff00::/8`

### 不受信任内容（机场响应不可信）

- YAML 解析：**先**限锚点/别名数（防十亿笑），**再**限嵌套深度/节点数；管理员提交的节点/分组 YAML 同等限制；请求体 ≤1MB（`413`）。
- `subscription-userinfo` 存/回显前校验格式（仅 `key=value; ...`，拒 CR/LF，防头注入）。
- 机场节点/分组名视为纯数据，渲染时转义，绝不拼入 HTML/shell。

### 敏感数据

不写完整机场 URL 进日志/公共输出/错误；管理 API 默认脱敏；Web UI 仅持脱敏值。脱敏规则：留 scheme/host/path，每个查询值 → `***`。HTTP trace 中 `PUBLIC_PATH_PREFIX` 与 profile token 均替换为占位值。

### 限流与客户端 IP

- 登录按 IP + 账户；公共下载按源 IP（独立于 token，`404` 也计数）。
- 默认 `TRUSTED_PROXY_HOPS=0`，按 TCP 对端限流。需真实客户端 IP 时，设置 `TRUSTED_PROXY_CIDRS`（逗号分隔的反代网段）；仅 TCP 对端在网段内才读 `X-Forwarded-For`，取最右不受信跳。

### 缓存与刷新

- 公共端点 `PUBLIC_REFRESH_MIN_SECONDS`（默认 30s）为最小回源间隔；间隔外回源重新生成，失败兜底旧缓存（无则 `503`）。`CACHE_TTL_MINUTES`（默认 15）仅管理端 `preview`。
- **single-flight**：同配置并发刷新合并为一次上游获取。

### 错误处理

- 公共端点：无效路径/token → `404`；有效但无缓存且拉取失败 → `503`（体内无上游细节）。
- 管理 API：返回有用校验错误但不含机场秘密；内部细节脱敏后入日志。
