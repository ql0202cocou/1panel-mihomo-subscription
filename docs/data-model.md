# 数据模型

> **状态:已实现。** 本文档定义的 SQLite 模式已由 `migrations/0001_init.sql`
> 实现,连接池按本文档应用每连接 pragma。

相关文档:`api-design.md`、`security-design.md`、`1panel-app.md`。

## 存储约定

- 数据库文件:`${DATA_DIR}/mihomo-subscription.db`(`DATA_DIR` 默认 `/data`)。
- 连接设置:`PRAGMA journal_mode = WAL;` `PRAGMA foreign_keys = ON;`
  `PRAGMA busy_timeout = 5000;`
  - `foreign_keys` 和 `busy_timeout` 是**每连接**生效的 pragma,必须在连接池
    的 after-connect 钩子里对**每个**连接设置,只设一次会让池中其余连接的外键
    约束静默失效(`ON DELETE CASCADE` 不触发,留下孤儿行)。
  - `busy_timeout` 让并发写在 SQLite 单写者锁上等待而非立即抛 `SQLITE_BUSY`;
    `journal_mode = WAL` 只需设一次,随数据库文件持久化。
- 主键:UUID v4,`TEXT` 类型。
- 时间戳:RFC 3339 UTC 字符串,`TEXT` 类型。
- 布尔值:`INTEGER`,`0`/`1`。
- 结构化字段:JSON 文本存入 `TEXT`。

## 实体关系

```text
app_settings (单行)

profiles 1 ──── 1 rulesets
profiles 1 ──── * custom_nodes
profiles 1 ──── * custom_groups
profiles 1 ──── 1 generated_cache
```

## 表定义

### app_settings

应用级设置。`public_path_prefix` 必须支持运行时重置(见 `security-design.md`
的 Token Rotation),因此存库而非只读环境变量;首次启动时若库中为空,则从
环境变量 `PUBLIC_PATH_PREFIX` 初始化,否则随机生成。

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
- `source_url` 含机场凭据,视为敏感数据:不完整写日志,API 响应默认脱敏
  (见 `security-design.md`)。
- `last_fetch_at` / `last_fetch_status`:最近一次机场拉取的观测字段,
  状态分类如 `success`、`http_error:502`、`ssrf_rejected`、`timeout`、
  `too_large`,供"原始订阅源"卡片展示。
- 输出 `proxies` 由**两个块**拼接:**机场块**(机场代理,上游顺序,用户不可排序)和
  **自定义块**(自定义节点)。节点预览把这两块渲染为可折叠、可拖动先后的分组。
- `node_order`:**仅自定义块**内的节点顺序,存为自定义节点名 JSON 数组。`NULL`=默认
  (按 `created_at`)。列出的名字优先按序排列,未列出的(新增自定义节点)落末尾。
  生成时把输出里的自定义节点顺序快照回写本列(故新自定义节点持久化到末尾);管理员
  在自定义分组内拖拽通过 `PUT .../node-order` 覆盖本列。机场块顺序始终上游序,**不**入
  本列。迁移 `0002_node_order.sql`。
- `node_section_order`:两个节点块的先后,JSON 两元数组(`"provider"`/`"custom"` 的
  排列),`NULL`=默认 `["provider","custom"]`(机场块在前)。由 `PUT .../node-section-order`
  写入。决定生成 `proxies` 里两块的拼接顺序。迁移 `0004_node_section_order.sql`。
- `group_order`:与 `node_order` 同义(含每次生成的快照回写与刷新语义),但作用于
  `proxy-groups`(分组名,机场 + 自定义)。决定生成 `proxy-groups` 的顺序与分组
  预览展示顺序。迁移 `0003_group_order.sql`。

### rulesets

MVP 阶段每个 profile 一份规则文本(`UNIQUE (profile_id)`);保留
`priority`/`name` 字段,便于后续扩展为多规则集而无需改表。

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
- `content`:该节点的 Mihomo proxy 配置(YAML 片段或等价 JSON),生成时整体
  并入输出 `proxies`。

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
- `members`:有序 JSON 数组,如 `["my-ss", "DIRECT", "MyGroup"]`,可引用机场节点
  (仍透传)与自定义节点/分组;引用有效性在生成时校验(见 `api-design.md`),不靠
  数据库约束。
- `options`:分组类型特有选项的 JSON 对象,如 `{"url": "...", "interval": 300}`。

> **已移除自定义规则集(rule-providers)托管。** 迁移 `0005_rule_providers.sql`
> 曾建 `rule_providers` 表用于自定义规则集 CRUD,但本项目不再托管/管理自定义规则集:
> converter 仅**透传**机场自带的 `rule-providers:`(导入的机场 `RULE-SET` 规则仍能
> 解析),不再合并自定义条目。迁移 `0006_drop_rule_providers.sql` 用
> `DROP TABLE IF EXISTS` 删除该表(对旧装机幂等安全)。规则里仍可用
> `RULE-SET,<name>,<policy>` 引用机场自带规则集的名称。

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
- 每个 profile 仅保留最新一份生成结果。**公共订阅端点每次拉取都重拉机场并重新生成**,
  本缓存仅作机场拉取失败时的兜底;`CACHE_TTL_MINUTES`(默认 15 分钟,按 `generated_at`
  判断)现仅用于管理端 `preview` 的新鲜度。
- `content_hash`:对“配置输入 + 原始订阅内容”的哈希,用于跳过无变化的重复生成。

## 迁移策略

- 使用 `sqlx::migrate!`,迁移文件放在 `migrations/`,命名
  `NNNN_description.sql`(如 `0001_init.sql`),启动时自动执行。
- 已应用的迁移文件不可修改,只能追加新迁移。
- 首版无生产数据,首个迁移 `0001_init.sql` 直接建立本文档全部表,并丢弃原型
  `subscriptions` 表,不做数据搬迁。
- SQLite 的 `ALTER TABLE` 能力有限,后续修改列时采用
  “建新表 → 拷贝 → 改名”模式。
