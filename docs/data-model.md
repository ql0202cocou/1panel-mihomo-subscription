# 数据模型

> SQLite 模式由 `migrations/` 实现;连接池按本文档对**每个**连接应用 pragma。

相关:`api-design.md`、`security-design.md`、`1panel-app.md`。

## 存储约定

- DB:`${DATA_DIR}/mihomo-subscription.db`(`DATA_DIR` 默认 `/data`)。
- 每连接 pragma:`foreign_keys = ON`、`busy_timeout = 5000`——**每连接**生效,须在连接池
  after-connect 钩子里对每个连接设(只设一次会让其余连接外键静默失效、留孤儿行);busy_timeout
  让并发写在锁上等待而非抛 `SQLITE_BUSY`。`journal_mode = WAL` 设一次随文件持久化。
- 主键 UUID v4(`TEXT`);时间戳 RFC 3339 UTC(`TEXT`);布尔 `INTEGER` 0/1;结构化字段 JSON 存 `TEXT`。

## 实体关系

```text
app_settings (单行)
global_nodes (全局池,不挂任何 profile)

profiles 1 ── 1 rulesets
profiles 1 ── * custom_groups
profiles 1 ── 1 generated_cache
```

`global_nodes` 是**跨订阅共享**的自定义节点池,自动追加到每条 profile 输出,不与 profile 外键
关联(删 profile 不影响它)。

## 表定义

### app_settings

应用级设置;`public_path_prefix` 支持运行时重置(故存库),首启时空则从 `PUBLIC_PATH_PREFIX`
初始化,否则随机生成。

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

- `token`:≥32 随机字节、URL-safe;自托管单用户场景明文存储以便展示完整链接(有意不哈希)。
  `source_url` 含机场凭据,视敏感:不完整入日志,API 默认脱敏。
- `last_fetch_*`:最近机场拉取观测(`success`/`http_error:502`/`ssrf_rejected`/`timeout`/`too_large`)。
- 输出 `proxies` = **机场块**(机场代理,上游序,不可排)+ **自定义块**(全局 `global_nodes`,各
  profile 一致)拼接。
- `node_order`:**已弃用**(自 `0007` 恒 NULL;列保留仅避 `DROP COLUMN`)。自定义块顺序改由全局
  `global_nodes.position` 决定。迁移 `0002`。
- `node_section_order`:两块先后,JSON 两元数组(`["provider","custom"]` 排列,NULL=机场块在前);
  **仍 per-profile**,由 `PUT .../node-section-order` 写。迁移 `0004`。
- `group_order`:`proxy-groups` 顺序(分组名数组,NULL=创建序);生成时快照回写、新增落末尾;
  `PUT .../group-order` 覆盖。迁移 `0003`。

### rulesets

每 profile 一份规则文本(`UNIQUE(profile_id)`);保留 `priority`/`name` 备扩展。

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

### global_nodes

**全局自定义节点池(跨订阅共享)。** 自 `0007_global_nodes.sql` 起自定义节点不再隶属单个
profile,而是一份全局集合自动追加到**每条** profile 输出的自定义块;编辑/排序统一在「节点配置」
页(`/api/global-nodes`),详情页只读。

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

- `name` 全局唯一;`node_type`(`ss`/`vmess`/…)不加 CHECK 免迁移;`content` 为完整 Mihomo proxy
  映射,生成时并入每条 profile 输出。
- `position`:全局自定义块顺序(`ORDER BY position, name`,name 作确定性兜底);新建取 `MAX+1`,
  `PUT /api/global-nodes/order` 重写为 `0..n-1` 并即时重排所有 profile 缓存。
- 迁移:原各 profile `custom_nodes` 按 `name` 去重(取 `updated_at` 最新)合并进本表(初始
  `position` 全 0,初始序按 name),随后 `DROP TABLE custom_nodes`。

### rule_sets

**全局用户规则库 / 导入源(② 用户规则库),`0008_rule_sets.sql`。** 管理员在「规则托管」页维护命名
规则集模板(手动 payload 或远程来源)。**自「订阅自包含规则库」起 ② 仅作导入源:不再公开托管、不再
参与生成**(对比早期版本曾托管在 `/<prefix>/r/<name>/...` 并按引用注入)。订阅通过「导入托管规则」
把所选 ② 条目复制进自己的 `profile_rule_sets`(③);生成只读 ③。表结构不变;`url`(remote 上游)、
`cached_*` 列保留但 ② 不再镜像(导入到 ③ 后由 ③ 镜像)。

```sql
CREATE TABLE rule_sets (
    id                TEXT    PRIMARY KEY,
    name              TEXT    NOT NULL UNIQUE,
    behavior          TEXT    NOT NULL CHECK (behavior IN ('domain', 'ipcidr', 'classical')),
    format            TEXT    NOT NULL CHECK (format IN ('yaml', 'text', 'mrs')),
    source            TEXT    NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'remote')),
    content           TEXT    NOT NULL DEFAULT '',  -- manual payload(每行一条)
    rule_count        INTEGER NOT NULL DEFAULT 0,   -- 列表展示用,免读 BLOB
    url               TEXT,                          -- remote 上游 URL
    interval_hours    INTEGER NOT NULL DEFAULT 24,
    cache             INTEGER NOT NULL DEFAULT 1,    -- remote 是否本地二次托管
    cached_body       BLOB,                          -- 镜像字节(text/yaml/mrs 二进制)
    cached_at         TEXT,
    last_fetch_status TEXT,
    enabled           INTEGER NOT NULL DEFAULT 1,
    position          INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL,
    updated_at        TEXT    NOT NULL
);

CREATE INDEX idx_rule_sets_position ON rule_sets (position);
```

- `name` 全局唯一,同时是 URL 路径段与 `RULE-SET` 引用名,故限定 `[A-Za-z0-9._-]`。
- **manual**:`content` 为 payload(每行一条);托管时 `yaml` 渲染为 `payload:` 列表、`text` 逐行原样;
  format 限 `yaml`/`text`。
- **remote**:`url` 为上游;`cache=1` 时面板按 `interval_hours` 懒拉取(每拉取检查新鲜度,过期才回源,
  SSRF 安全)、把原始字节存入 `cached_body`(BLOB,故二进制 `mrs` 不损坏)并以稳定链接二次托管,失败
  回退旧缓存;`cache=0` 则不托管,转换时直接注入上游 `url`。`last_fetch_status` 同 profile 拉取标签。
  更新规则集会清空缓存列(下次拉取重新回源)。
- `position`:仅「规则托管」页的展示顺序(`ORDER BY position, name`)。

### profile_rule_sets

**每订阅自包含规则库(③ 托管规则库),`0011_profile_rule_sets.sql`。** 镜像 `rule_sets` 的字段但按
`profile_id` 隔离、去掉无语义的 `position`(rule-providers 是 map)。下发时 `RULE-SET,<name>` 引用按名
注入本订阅自己的定义;托管在**按订阅 token 隔离**的链接
`/<prefix>/api/sub/<token>/r/<name>/<behavior>.<format>`,故不同订阅可复用同名而不冲突。

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

- `name` 在**单个订阅内**唯一(`UNIQUE(profile_id, name)`),是 URL 路径段与 `RULE-SET` 引用名,限定
  `[A-Za-z0-9._-]`。
- **manual / remote** 行为与 `rule_sets` 完全一致(校验/渲染/镜像逻辑由 `src/rulelib.rs` 共用);唯一
  区别是托管链接含订阅 token、且这是生成时**唯一**的规则集来源。
- 「导入托管规则」从 ② 复制条目进本表(含真实远程 URL,由后端复制,前端只见脱敏 URL)。

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

- 是输出 `proxy-groups` 的**唯一来源**(转换器整体替换机场分组,机场原生分组不透传;经
  `import-provider-groups` 落为自定义分组才可编辑入输出)。
- `members`:有序 JSON 数组,可引用机场节点(透传)/自定义节点/分组;引用有效性在生成时校验,
  不靠 DB 约束。`options`:类型特有选项 JSON(如 `{"url":"...","interval":300}`)。

> **已移除自定义规则集(rule-providers)托管:** `0005` 曾建 `rule_providers` 表,
> `0006_drop_rule_providers.sql` 用 `DROP TABLE IF EXISTS` 删除(对旧装机幂等)。转换器只透传
> 机场自带 `rule-providers:`;规则里仍可 `RULE-SET,<name>,<policy>` 引用机场条目名。

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

- 每 profile 仅留最新一份。公共端点在 `PUBLIC_REFRESH_MIN_SECONDS`(默认 30 秒)下限内复用最近缓存,
  下限外回源重新生成;本缓存也作机场拉取失败兜底。`CACHE_TTL_MINUTES`(默认 15,按 `generated_at`)
  仅管理端 `preview`。
- `subscription_userinfo`:机场响应头原文,随缓存保存并在公共端点透传(无则 NULL)。
  `content_hash`:对「输入 + 机场内容」的哈希,跳过无变化重复生成。

## 迁移

- `sqlx::migrate!`,文件 `migrations/NNNN_description.sql`,启动自动执行;已应用文件不可改,只追加。
- `0001` 直接建全表(无生产数据,丢弃原型 `subscriptions`)。SQLite `ALTER` 受限,改列用
  「建新表 → 拷贝 → 改名」。
