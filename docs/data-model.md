# 数据模型 / Data Model

> **状态:已实现。** 本文档定义的 SQLite 模式已由 `migrations/0001_init.sql`
> 实现,连接池按本文档应用每连接 pragma。
>
> **Status: implemented.** The SQLite schema defined here is implemented by
> `migrations/0001_init.sql`, and the pool applies the per-connection pragmas
> described here.

相关文档 / Related documents: `api-design.md`、`security-design.md`、
`1panel-app.md`。

## 存储约定 / Storage Conventions

- 数据库文件 / Database file: `${DATA_DIR}/mihomo-subscription.db`
  (`DATA_DIR` 默认 / defaults to `/data`)。
- 连接设置 / Connection settings: `PRAGMA journal_mode = WAL;`
  `PRAGMA foreign_keys = ON;` `PRAGMA busy_timeout = 5000;`
  - `foreign_keys` 和 `busy_timeout` 是**每连接**生效的 pragma,必须在连接池
    的 after-connect 钩子里对**每个**连接设置,只设一次会让池中其余连接的外键
    约束静默失效(`ON DELETE CASCADE` 不触发,留下孤儿行)。
  - `foreign_keys` and `busy_timeout` are **per-connection** pragmas and must
    be applied to **every** pooled connection via an after-connect hook.
    Setting them once leaves other pool connections with foreign keys silently
    off (`ON DELETE CASCADE` won't fire, orphaning rows).
  - `busy_timeout` 让并发写在 SQLite 单写者锁上等待而非立即抛 `SQLITE_BUSY`;
    `journal_mode = WAL` 只需设一次,随数据库文件持久化。
  - `busy_timeout` makes concurrent writers wait on SQLite's single-writer
    lock instead of failing fast with `SQLITE_BUSY`; `journal_mode = WAL` is
    set once and persists with the database file.
- 主键 / Primary keys: UUID v4,`TEXT` 类型 / stored as `TEXT`.
- 时间戳 / Timestamps: RFC 3339 UTC 字符串,`TEXT` 类型 / strings in `TEXT`.
- 布尔值 / Booleans: `INTEGER`,`0`/`1`。
- 结构化字段 / Structured fields: JSON 文本存入 `TEXT` / JSON text in `TEXT`.

## 实体关系 / Entity Relationships

```text
app_settings (单行 / single row)

profiles 1 ──── 1 rulesets
profiles 1 ──── * custom_nodes
profiles 1 ──── * custom_groups
profiles 1 ──── * rule_providers
profiles 1 ──── 1 generated_cache
```

## 表定义 / Table Definitions

### app_settings

应用级设置。`public_path_prefix` 必须支持运行时重置(见 `security-design.md`
的 Token Rotation),因此存库而非只读环境变量;首次启动时若库中为空,则从
环境变量 `PUBLIC_PATH_PREFIX` 初始化,否则随机生成。

App-level settings. `public_path_prefix` must support runtime reset (see Token
Rotation in `security-design.md`), so it lives in the database rather than a
read-only environment variable; on first startup it is seeded from the
`PUBLIC_PATH_PREFIX` environment variable if set, otherwise randomly generated.

```sql
CREATE TABLE app_settings (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    public_path_prefix  TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);
```

### profiles

```sql
CREATE TABLE profiles (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL,
    source_type TEXT    NOT NULL CHECK (source_type IN ('mihomo','clash','surge','loon')),
    source_url  TEXT    NOT NULL,
    output_type TEXT    NOT NULL DEFAULT 'mihomo',
    token       TEXT    NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
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
CREATE INDEX idx_profiles_enabled      ON profiles (enabled);
```

- `token`:至少 32 随机字节,URL-safe 编码。自托管单用户场景下明文存储,
  以便管理页展示完整托管链接;不做哈希是有意取舍。
- `token`: at least 32 random bytes, URL-safe encoded. Stored in plaintext so
  the admin UI can display the full hosted link; not hashing it is a
  deliberate trade-off for the single-user self-hosted scenario.
- `source_url` 含机场凭据,视为敏感数据:不完整写日志,API 响应默认脱敏
  (见 `security-design.md`)。
- `source_url` contains provider secrets and is sensitive: never fully logged,
  masked by default in API responses (see `security-design.md`).
- `last_fetch_at` / `last_fetch_status`:最近一次机场拉取的观测字段,
  状态分类如 `success`、`http_error:502`、`ssrf_rejected`、`timeout`、
  `too_large`,供"原始订阅源"卡片展示。
- `last_fetch_at` / `last_fetch_status`: observability fields for the latest
  provider fetch; status values such as `success`, `http_error:502`,
  `ssrf_rejected`, `timeout`, `too_large`, displayed on the source card.
- 输出 `proxies` 由**两个块**拼接:**机场块**(机场代理,上游顺序,用户不可排序)和
  **自定义块**(自定义节点)。节点预览把这两块渲染为可折叠、可拖动先后的分组。
- `node_order`:**仅自定义块**内的节点顺序,存为自定义节点名 JSON 数组。`NULL`=默认
  (按 `created_at`)。列出的名字优先按序排列,未列出的(新增自定义节点)落末尾。
  生成时把输出里的自定义节点顺序快照回写本列(故新自定义节点持久化到末尾);管理员
  在自定义分组内拖拽通过 `PUT .../node-order` 覆盖本列。机场块顺序始终上游序,**不**入
  本列。迁移 `0002_node_order.sql`。
- `node_order`: order of nodes **within the custom block only**, a JSON array of
  custom node names. `NULL` = default (`created_at`). Listed names go first; any
  not listed (newly added custom node) fall to the end. Generation snapshots the
  output's custom-node order back into this column (so new customs persist at the
  end); an admin's drag inside the custom group overwrites it via
  `PUT .../node-order`. The provider block's order is always upstream and is
  **not** stored here. Migration `0002_node_order.sql`.
- `node_section_order`:两个节点块的先后,JSON 两元数组(`"provider"`/`"custom"` 的
  排列),`NULL`=默认 `["provider","custom"]`(机场块在前)。由 `PUT .../node-section-order`
  写入。决定生成 `proxies` 里两块的拼接顺序。迁移 `0004_node_section_order.sql`。
- `node_section_order`: order of the two node blocks, a 2-element JSON array (a
  permutation of `"provider"`/`"custom"`); `NULL` = default `["provider","custom"]`
  (provider block first). Written by `PUT .../node-section-order`; drives how the
  two blocks are concatenated in the output `proxies`. Migration
  `0004_node_section_order.sql`.
- `group_order`:与 `node_order` 同义(含每次生成的快照回写与刷新语义),但作用于
  `proxy-groups`(分组名,机场 + 自定义)。决定生成 `proxy-groups` 的顺序与分组
  预览展示顺序。迁移 `0003_group_order.sql`。
- `group_order`: same as `node_order` (including the per-generation snapshot and
  refresh semantics) but for `proxy-groups` (group names, provider + custom).
  Drives the generated `proxy-groups` order and the group-preview display order.
  Migration `0003_group_order.sql`.

### rulesets

MVP 阶段每个 profile 一份规则文本(`UNIQUE (profile_id)`);保留
`priority`/`name` 字段,便于后续扩展为多规则集而无需改表。

In the MVP each profile has exactly one rule text (`UNIQUE (profile_id)`); the
`priority`/`name` columns are kept so multi-ruleset support can land later
without a schema change.

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

### custom_nodes

```sql
CREATE TABLE custom_nodes (
    id          TEXT    PRIMARY KEY,
    profile_id  TEXT    NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    node_type   TEXT    NOT NULL,
    content     TEXT    NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    UNIQUE (profile_id, name)
);

CREATE INDEX idx_custom_nodes_profile ON custom_nodes (profile_id);
```

- `node_type`:`ss` / `vmess` / `vless` / `trojan` / `hysteria2` 等,不用
  CHECK 约束,避免新增协议需要迁移。
- `node_type`: `ss` / `vmess` / `vless` / `trojan` / `hysteria2`, etc. No
  CHECK constraint so new protocols don't require a migration.
- `content`:该节点的 Mihomo proxy 配置(YAML 片段或等价 JSON),生成时整体
  并入输出 `proxies`。
- `content`: the node's Mihomo proxy config (YAML fragment or equivalent
  JSON), merged into the output `proxies` at generation time.

### custom_groups

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

- `custom_groups` 是输出 `proxy-groups` 的**唯一来源**:转换器整体替换机场分组(同
  `rules`),机场原生分组不透传。机场分组经 `POST .../import-provider-groups` 落为
  自定义分组后才可编辑并进入输出;不导入则机场更新分组不生效(见 `api-design.md`)。
- `custom_groups` is the **sole source** of the output `proxy-groups`: the
  converter replaces provider groups entirely (like `rules`), so provider groups
  are not passed through. They become editable and enter the output only after
  `POST .../import-provider-groups` writes them as custom groups; without
  importing, provider group updates have no effect (see `api-design.md`).
- `members`:有序 JSON 数组,如 `["my-ss", "DIRECT", "MyGroup"]`,可引用机场节点
  (仍透传)与自定义节点/分组;引用有效性在生成时校验(见 `api-design.md`),不靠
  数据库约束。
- `members`: an ordered JSON array, e.g. `["my-ss", "DIRECT", "MyGroup"]`,
  referencing provider proxies (still passed through) or custom nodes/groups;
  reference validity is checked at generation time (see `api-design.md`), not by
  the database.
- `options`:分组类型特有选项的 JSON 对象,如 `{"url": "...", "interval": 300}`。
- `options`: JSON object of group-type-specific options, e.g.
  `{"url": "...", "interval": 300}`.

### rule_providers

自定义规则集(`规则集` / Mihomo `rule-providers`),被规则里的 `RULE-SET,<name>,<policy>`
按名引用。`provider_type`/`behavior` 为一等列(用于展示与校验),其余键(url、path、
payload、format、interval、size-limit、proxy 等)放进 `options` JSON。迁移
`0005_rule_providers.sql`。

Custom rule-providers (`规则集`), referenced by name from `RULE-SET,<name>,<policy>`
rules. `provider_type`/`behavior` are first-class columns (for display and
validation); the remaining keys (url, path, payload, format, interval,
size-limit, proxy, …) live in the `options` JSON blob. Migration
`0005_rule_providers.sql`.

```sql
CREATE TABLE rule_providers (
    id            TEXT    PRIMARY KEY,
    profile_id    TEXT    NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    name          TEXT    NOT NULL,
    provider_type TEXT    NOT NULL CHECK (provider_type IN ('http','file','inline')),
    behavior      TEXT    NOT NULL CHECK (behavior IN ('domain','ipcidr','classical')),
    options       TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT    NOT NULL,
    updated_at    TEXT    NOT NULL,
    UNIQUE (profile_id, name)
);

CREATE INDEX idx_rule_providers_profile ON rule_providers (profile_id);
```

- 与 `rules`/`custom_groups` 的"整体替换"不同,自定义规则集是**合并**进输出的
  `rule-providers`:机场的仍透传,自定义条目按名覆盖。这样导入的机场 `RULE-SET`
  规则仍能解析;新增/修改在下次生成时生效(见 `api-design.md`)。
- Unlike the "replace entirely" handling of `rules`/`custom_groups`, custom
  rule-providers are **merged** into the output `rule-providers`: the provider's
  still pass through and custom entries override by name. This keeps imported
  provider `RULE-SET` rules resolving; adds/edits take effect on the next
  generation (see `api-design.md`).

### generated_cache

```sql
CREATE TABLE generated_cache (
    profile_id            TEXT PRIMARY KEY REFERENCES profiles (id) ON DELETE CASCADE,
    content_hash          TEXT NOT NULL,
    output_yaml           TEXT NOT NULL,
    subscription_userinfo TEXT,
    generated_at          TEXT NOT NULL
);
```

- `subscription_userinfo`:原始订阅响应的 `subscription-userinfo` 头原文,
  随缓存保存并在公开端点透传(见 `api-design.md`);上游未提供时为 NULL。
- `subscription_userinfo`: the raw `subscription-userinfo` header from the
  provider response, stored with the cache and passed through on the public
  endpoint (see `api-design.md`); NULL when the provider does not send it.

- 每个 profile 仅保留最新一份生成结果。**公共订阅端点每次拉取都重拉机场并重新生成**,
  本缓存仅作机场拉取失败时的兜底;`CACHE_TTL_MINUTES`(默认 15 分钟,按 `generated_at`
  判断)现仅用于管理端 `preview` 的新鲜度。
- Only the latest generated output is kept per profile. **The public subscription
  endpoint re-fetches the provider and regenerates on every pull**, so this cache
  is only a fallback when that fetch fails; `CACHE_TTL_MINUTES` (default 15 min,
  evaluated from `generated_at`) now governs only the admin `preview` freshness.
- `content_hash`:对“配置输入 + 原始订阅内容”的哈希,用于跳过无变化的重复生成。
- `content_hash`: hash of "profile inputs + provider subscription content",
  used to skip regeneration when nothing changed.

## 迁移策略 / Migration Strategy

- 使用 `sqlx::migrate!`,迁移文件放在 `migrations/`,命名
  `NNNN_description.sql`(如 `0001_init.sql`),启动时自动执行。
- 已应用的迁移文件不可修改,只能追加新迁移。
- 首版无生产数据,首个迁移 `0001_init.sql` 直接建立本文档全部表,并丢弃原型
  `subscriptions` 表,不做数据搬迁。
- SQLite 的 `ALTER TABLE` 能力有限,后续修改列时采用
  “建新表 → 拷贝 → 改名”模式。

&nbsp;

- Use `sqlx::migrate!` with files under `migrations/`, named
  `NNNN_description.sql` (e.g. `0001_init.sql`), applied automatically at
  startup.
- Never edit an applied migration; only append new ones.
- The project is in planning with no production data, so the first migration
  `0001_init.sql` creates every table in this document and drops the prototype
  `subscriptions` table with no data migration.
- SQLite has limited `ALTER TABLE` support; later column changes should use
  the "create new table → copy → rename" pattern.
