# 数据模型 / Data Model

> **状态:规划阶段。** 本文档定义目标 SQLite 模式,尚未实现。当前代码中的
> `subscriptions` 表是早期原型,将在首个正式迁移中被 `profiles` 体系取代。
>
> **Status: planning.** This document defines the target SQLite schema; it is
> not implemented yet. The current `subscriptions` table is an early prototype
> and will be replaced by the `profiles` schema in the first real migration.

相关文档 / Related documents: `technical-roadmap.md`(模型来源 / model source)、
`api-design.md`、`security-design.md`。

## 存储约定 / Storage Conventions

- 数据库文件 / Database file: `${DATA_DIR}/mihomo-subscription.db`
  (`DATA_DIR` 默认 / defaults to `/data`)。
- 连接设置 / Connection settings: `PRAGMA journal_mode = WAL;`
  `PRAGMA foreign_keys = ON;`
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

- `members`:有序 JSON 数组,如 `["my-ss", "DIRECT", "ProviderGroup"]`,可引用
  机场节点/分组和自定义节点/分组;引用有效性在生成时校验(见
  `api-design.md`),不靠数据库约束。
- `members`: an ordered JSON array, e.g. `["my-ss", "DIRECT",
  "ProviderGroup"]`, referencing provider or custom nodes/groups; reference
  validity is checked at generation time (see `api-design.md`), not by the
  database.
- `options`:分组类型特有选项的 JSON 对象,如 `{"url": "...", "interval": 300}`。
- `options`: JSON object of group-type-specific options, e.g.
  `{"url": "...", "interval": 300}`.

### generated_cache

```sql
CREATE TABLE generated_cache (
    profile_id   TEXT PRIMARY KEY REFERENCES profiles (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    output_yaml  TEXT NOT NULL,
    generated_at TEXT NOT NULL
);
```

- 每个 profile 仅保留最新一份生成结果;TTL(默认 5–30 分钟,可配置)在应用层
  根据 `generated_at` 判断,过期即重新拉取生成。
- Only the latest generated output is kept per profile; TTL (configurable,
  default 5–30 minutes) is enforced in the application layer from
  `generated_at` — stale entries trigger a refresh.
- `content_hash`:对“配置输入 + 原始订阅内容”的哈希,用于跳过无变化的重复生成。
- `content_hash`: hash of "profile inputs + provider subscription content",
  used to skip regeneration when nothing changed.

## 迁移策略 / Migration Strategy

- 使用 `sqlx::migrate!`,迁移文件放在 `migrations/`,命名
  `NNNN_description.sql`(如 `0001_init.sql`),启动时自动执行。
- 已应用的迁移文件不可修改,只能追加新迁移。
- 项目处于规划阶段、无生产数据,首个迁移 `0001_init.sql` 直接建立本文档全部
  表,并丢弃原型 `subscriptions` 表,不做数据搬迁。
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
