# 变更日志

此项目的所有显著变更都应记录在此文件中。

使用倒序。保持条目简洁，尽可能面向用户，并按变更类型分组。

## 维护规则

- 永远不要删除旧版本条目。
- 在 `[Unreleased]` 下添加新工作。
- 每次变更都更新受影响的项目文档，使文档和实现/设计保持一致。
- 发布时，将当前 `[Unreleased]` 部分重命名为发布的版本和日期，然后在其上方创建新的空 `[Unreleased]` 部分。
- 保持新版本在旧版本上方。
- 保留历史条目，即使后来的发布更改或取代它们。

## 模板

```markdown
## [Unreleased]

### 新增

- 

### 变更

- 

### 修复

- 

### 安全

- 

### 文档

- 

## [0.1.0] - YYYY-MM-DD

### 新增

- 首次发布变更。
```

## [Unreleased]

### 变更

- 规则编辑器(`RulesCard`)易用性增强,从「逐条手敲」转向「批量 + 可管理」:
  - **批量编辑**:新增「批量编辑」开关,在 textarea 中整段粘贴/修改规则(每行
    `类型,匹配内容,策略`),保存时按行解析整体替换列表。
  - **固定兜底策略**:把 `MATCH` 从普通列表抽出为独立的「兜底策略」下拉,始终位于
    规则末尾、不可被拖拽越过;列表区只放命中即停的具体规则。
  - **列表可管理**:新增搜索过滤、每条规则「置顶/置底」、多选后「批量删除/批量改
    策略」;搜索过滤时禁用拖拽(改用置顶/置底)。
  - **行内编辑**:点击「编辑」时该行就地变为编辑器,不再跳到顶部。
  - 仅前端改动,规则仍存为 `rulesets.content` 文本,后端契约不变。

### 文档

- 将剩余技术文档(`api-design.md`、`data-model.md`、`release.md`)由中英双语
  转为纯中文,与已转换的 `security-design.md`/`1panel-app.md`/`README.md`/
  `changelog.md` 保持一致。
- 将文档版本头对齐到 `0.2.0`:`1panel-app.md`(状态/结构示例/验证清单)、
  `release.md`(状态/YAML 校验路径/构建示例)、`docs/README.md`;`release.md` 的
  「新增版本目录」示例改为从 `0.2.0` 升到 `0.2.1`,并移除已删除的 `README_en.md`
  引用。
- 清理 `changelog.md` 中的重复中文段落:0.1.4—0.1.16 各条目此前在原始中文之后
  附了一份机翻自英文的冗余中文行,现移除该重复行,仅保留原始中文条目(共 29 行)。
- 修复根 `README.md` 版本滞后:状态横幅与一键部署 Compose 镜像 tag 由 `0.1.8`
  更新到 `0.2.0`(此前照 README 部署会拉到旧镜像)。
- 修复根 `README.md`「文档与开发」段的两个失效内部链接,改为指向
  `.github/workflows/ci.yml`(CI 门禁)与 `docs/release.md`(发布/变更规则);
  `docs/release.md` 发布前检查相应改为引用 `docs/changelog.md` 维护规则。
- 从 changelog 历史中移除对本地专属 agent 指导文件的提及与内容描述:该文件不纳入
  版本控制,公开 changelog 不应泄露其内容。

## [0.2.0] - 2026-06-19

### 新增

- 规则集 (rule-providers) 管理：配置文件详情页新增 `RuleProvidersCard`，通过 schema 驱动的结构化表单对自定义规则集（`http`/`file`/`inline` 类型；`domain`/`ipcidr`/`classical` 行为）进行完整 CRUD。新增 `rule_providers` 表（每个配置文件 1—*）和管理端点 `GET/POST /api/profiles/:id/rule-providers` 和 `PUT/DELETE /api/profiles/:id/rule-providers/:rp_id`；配置文件详情响应现在包含 `rule_providers`。

### 变更

- 转换器现在将自定义规则集**合并**到输出 `rule-providers:` 映射中，覆盖机场的同名条目（自定义条目覆盖机场同名条目），而不是仅透传机场的。这是累加性的，因此导入的机场 `RULE-SET` 规则继续解析；变更在下次生成时生效。
- 将规则编辑器（`RulesCard`）重构为内联的 Clash-Verge 风格编辑器（不再使用模态框）：单行类型 · 内容 · no-resolve · 策略，其中**内容输入适应规则类型**（`RULE-SET` 选择定义的规则集名称，`NETWORK` 选择 tcp/udp，其他显示每类型示例占位符 `RULE_EXAMPLES`）。“编辑”将规则加载回编辑器原位；规则列表保持单一可拖拽排序列表（转换器拥有整个 `rules` 块，因此没有前置/追加分割）。

### 文档

- 记录 `rule_providers` 表（`data-model.md`）、规则集管理端点和合并语义（`api-design.md`）。

## [0.1.16] - 2026-06-17

### 变更

- 规则编辑器「规则类型」下拉补全更多 Mihomo 规则类型（`IP-SUFFIX`、`SRC-GEOIP`、`SRC-IP-ASN`、`SRC-IP-SUFFIX`、`IN-PORT`、`IN-TYPE`、`IN-USER`、`IN-NAME`、`UID`、`NETWORK`、`DSCP`、`PROCESS-NAME-REGEX`、`PROCESS-PATH-REGEX`、逻辑规则 `AND`/`OR`/`NOT`/`SUB-RULE` 等），并把 `no-resolve` 选项扩展到全部基于 IP 的类型（含 `RULE-SET`）；选择器仍允许手输任意类型。

### 修复

- 规则编辑器「匹配内容」现可正确填写含逗号/括号的复杂匹配内容（逻辑/嵌套规则 `AND`/`OR`/`NOT`/`SUB-RULE`，如 `AND,((DOMAIN,x.com),(NETWORK,udp)),Proxy`）：解析改为从末尾剥离 `no-resolve` 与策略、其余整体作为匹配内容，不再按固定逗号位置切分，编辑/回显不再错位。

## [0.1.15] - 2026-06-16

### 变更

- **公共订阅每次拉取都实时拉机场，客户端永远用最新节点：** 公共订阅端点（`GET /<prefix>/api/sub/<token>`）不再“缓存新鲜就返回”，而是每次拉取都重拉机场并重新生成；`generated_cache` 仅作机场拉取失败时的兜底。并发拉取仍由 per-profile single-flight 合并为一次机场拉取（`serve_or_refresh` + `generated_since`）。`CACHE_TTL_MINUTES` 现仅影响管理端 `preview`。
- **移除「生成配置」按钮：** 既然公共链接始终实时，手动「生成」已多余，移除详情页底部的「生成配置」按钮及「未生成的修改」告警；管理端预览里的机场节点改由「原始订阅源 → 刷新」更新（自定义节点/分组/规则编辑仍即时反映）。
- 界面文案微调：应用标题「Mihomo 订阅管理」改为「管理后台」；「原始订阅源」卡片标题改为「原始订阅」；「托管订阅链接」卡片标题改为「托管订阅」。

## [0.1.14] - 2026-06-16

### 变更

- **节点预览重构为「可展开式分组」：** 节点预览不再是机场+自定义混排的扁平列表，而是两个**可折叠、可拖动先后**的分组——「机场」分组（名称为机场名，机场节点**只读**上游顺序）与「自定义」分组（组内节点可拖动排序）。点击分组名展开/折叠；拖动分组改两块先后，拖动自定义节点改组内顺序。**分组先后 × 自定义组内顺序决定生成订阅 `proxies` 的实际顺序**，且即时生效（就地重写缓存，无需重拉机场）。这样机场更新只在机场块内变化，不再打乱用户的自定义顺序。
  - 数据/接口：`node_order` 语义改为**仅自定义块**顺序；新增 `profiles.node_section_order`（迁移 `0004_node_section_order.sql`）与 `PUT /api/profiles/:id/node-section-order`；`GET /proxies` 新增 `node_section_order` 字段并直接返回缓存（排序改动已就地重写缓存）；生成时仅快照自定义节点顺序（机场块始终上游序、不快照）。

## [0.1.13] - 2026-06-15

### 变更

- 分组预览不再区分「机场/自定义」：机场分组已被替换（需导入为自定义分组），因此预览里的所有分组现在一律可编辑/排序/删除，去掉了只读的「机场」分组行与「自定义/机场」标签；排序顺序仍取自上次生成的输出。

## [0.1.12] - 2026-06-15

### 变更

- **（破坏性）机场分组改为「导入才生效」模型（同规则）：** 转换器不再透传机场原生 `proxy-groups`，而是整体替换为自定义分组——机场更新分组不会自动进入输出。**升级提示：** 已依赖机场原生分组的配置，需在「分组预览」点击「导入机场分组」后重新「生成」，否则下次生成的输出将不含这些分组（不做自动迁移）。

### 新增

- **导入机场分组 + 机场分组可编辑：** 「分组预览」新增「导入机场分组」按钮，经新端点 `POST /api/profiles/:id/import-provider-groups`（SSRF 保护的实时拉取，同 `provider-rules`）把机场 `proxy-groups` 解析为**可编辑的自定义分组**（name/type/proxies→成员，其余键→options；跳过同名与不支持类型），返回 `{ imported, skipped }`；导入后即可像普通自定义分组一样编辑/排序/删除，重新生成后进入订阅。

## [0.1.11] - 2026-06-15

### 修复

- 修复节点/分组预览拖动后卡片又「弹回」原位的问题：拖动时已乐观更新顺序，但行列表会在每次从服务端重新派生时直接覆盖该顺序；当重新派生的列表暂时无法体现刚拖动的顺序（配置尚未生成，或新增但未重新生成的自定义节点——按 `created_at` 重建）时，卡片会弹回旧位置（顺序其实已持久化）。现改为按成员合并：对仍存在的行保留当前屏幕顺序（数据刷新），新增项追加、移除项删除，从而保住拖动顺序。规则不受影响（其内容本身即顺序，重载即返回新序）。仅前端可视修复。

## [0.1.10] - 2026-06-15

### 变更

- 拖拽排序与规则编辑**立即生效**：此前调整节点/分组顺序或编辑规则需重新「生成配置」才会应用到公共订阅链接，现在保存后后端会就地重写已生成缓存（`generated_cache.output_yaml`）对应的 `proxies`/`proxy-groups`/`rules`、**无需重新拉取机场**，节点/分组/规则预览与公共订阅链接随即返回新内容。`node-order`/`group-order`/`rules` 端点保存后调用 `generate::resync_cache`（best-effort，保留 `generated_at` 不影响刷新节奏）；从未生成过的配置仍在首次生成时生效，新增节点/分组仍需生成。前端三张卡片的提示文案同步改为「立即生效」。

## [0.1.9] - 2026-06-15

### 新增

- 节点 / 分组 / 规则预览支持拖拽排序：在三张预览卡片均可直接拖动列表调整顺序，松手即保存。节点 / 分组：新增 `profiles.node_order` / `group_order`（JSON 名字数组，迁移 `0002_node_order.sql` / `0003_group_order.sql`）与 `PUT /api/profiles/:id/node-order` / `group-order` 端点；转换器在组装 `proxies` / `proxy-groups` 后按对应顺序重排，`GET /api/profiles/:id/proxies` 也按其返回，使预览在重新生成前即反映新顺序；未列出的新条目回退到末尾默认位置。规则：规则顺序本就是有序文本且具语义（命中即止），前端拖动后经现有 `PUT /api/profiles/:id/rules` 整体保存，无需新增列/端点。所有排序于下一次「生成配置」时应用到订阅输出。前端引入 `@dnd-kit`。
- 订阅更新时的节点/分组排序保持稳定：每次生成都会把输出的节点/分组实际顺序快照回写到 `node_order` / `group_order`（`persist_cache` → `snapshot_orders`）。因此后续自动拉取机场订阅时，已存在的节点/分组按名字保留原位置（其信息按名从新机场 YAML 刷新），新增的节点/分组排到列表末尾；管理员的手动拖拽顺序仍会被保留并在新增时向后追加。

### 文档

- 文档校对：修正 `docs/README.md` 中过时的「发布前待办」段落（0.1.8 已发布、安装表单已完成）；在 `docs/1panel-app.md` 环境变量表补充容器内部变量 `WEB_DIR`（Dockerfile 内置，非安装项），使该「权威清单」与代码一致。

## [0.1.8] - 2026-06-14

### 新增

- 规则预览支持「导入机场规则」：由于转换时自定义规则会整体替换机场规则，新增 `GET /api/profiles/:id/provider-rules`（实时 SSRF 拉取机场订阅并解析 `rules`），前端「导入机场规则」按钮把机场规则追加到列表末尾（跳过重复），便于以机场规则为起点再做定制。

### 文档

- 新增 `apps/mihomo-subscription/0.1.8/` 应用包（镜像 `quinlanhoo/mihomo-subscription:0.1.8`），将 `Cargo.toml`/`Cargo.lock` 与 `web/package.json` 升到 `0.1.8`。

## [0.1.7] - 2026-06-14

### 变更

- 「自定义分组」卡片改名为「分组预览」，交互对齐「节点预览」：在自定义分组（可编辑）之外只读列出机场分组（解析自最近一次生成的输出），沿用相同的标签、计数与未生成提示。`GET /api/profiles/:id/proxies` 的 `groups` 由名称数组改为 `name`+`type` 对象数组（机场分组预览也显示类型）。
- 「分流规则」卡片改名为「规则预览」，交互对齐「节点预览」：整块 YAML 文本编辑器改为逐条规则的列表，单条规则用结构化表单增/改/删（规则类型 / 匹配内容 / 策略下拉 / no-resolve），策略候选来自机场节点/分组、自定义节点/分组与内置策略。注释与不常见规则（如逻辑 AND/OR）按原文保留。随之移除已无用的 CodeMirror YAML 编辑器组件与依赖（前端构建产物显著减小）。

### 文档

- 新增 `apps/mihomo-subscription/0.1.7/` 应用包（镜像 `quinlanhoo/mihomo-subscription:0.1.7`），将 `Cargo.toml`/`Cargo.lock` 与 `web/package.json` 升到 `0.1.7`。

## [0.1.6] - 2026-06-14

### 新增

- 节点编辑器扩充图形化字段：VLESS 新增 REALITY（`reality-opts`：public-key / short-id）、传输层（`ws-opts` 含 path 与 Host、`grpc-opts`）、`flow`、`client-fingerprint`、`alpn`、`udp`、`skip-cert-verify` 等；vmess/trojan/hysteria2/tuic 也补齐常用项。字段按 TLS/传输协议条件显示，嵌套选项以结构化子表单编辑，无需再手写 YAML。

### 文档

- 新增 `apps/mihomo-subscription/0.1.6/` 应用包（镜像 `quinlanhoo/mihomo-subscription:0.1.6`），将 `Cargo.toml`/`Cargo.lock` 与 `web/package.json` 升到 `0.1.6`。

## [0.1.5] - 2026-06-14

### 修复

- 修复机场订阅拉取返回 `http_error:403/401` 导致无法生成的问题：此前拉取请求未带 `User-Agent`，而大量机场后端（SSPanel/V2board 等）会校验 UA 是否为 Clash 家族，否则拒绝或返回非 YAML 页面。现默认发送 `clash.meta/1.0`（可用环境变量 `FETCH_USER_AGENT` 覆盖）。

### 文档

- 将根 `README.md` 的版本号与镜像 tag 同步到 `0.1.4`（0.1.4 发布时遗漏）。
- 新增 `apps/mihomo-subscription/0.1.5/` 应用包（镜像 `quinlanhoo/mihomo-subscription:0.1.5`），将 `Cargo.toml`/`Cargo.lock` 与 `web/package.json` 升到 `0.1.5`；在 `1panel-app.md` 环境变量表登记 `FETCH_USER_AGENT`（可选，代码内置默认，不在安装表单中）。

## [0.1.4] - 2026-06-14

### 新增

- 节点预览：订阅详情页的「自定义节点」卡片改名为「节点预览」，在自定义节点之外同时只读列出机场节点（解析自最近一次生成的输出）。新增只读接口 `GET /api/profiles/:id/proxies`。

### 变更

- 自定义节点改用结构化 UI 表单编辑（按类型给出 server/port/密码/加密/uuid/tls/sni 等常用字段，其余字段以高级键值行补充），不再要求管理员手写 Mihomo proxy YAML；保存时由前端序列化为 `content`。前端新增 `yaml` 依赖。
- 自定义分组改用结构化 UI 表单编辑：按分组类型给出选项字段（`url`/`interval`/`tolerance`/`lazy`/`strategy`）+ 高级键值行，取代原先的选项 JSON 文本框；成员选择改为从机场节点/分组、自定义节点/分组与内置策略中下拉候选（仍可手动输入）。节点与分组共用一套结构化字段组件。

### 文档

- 新增 `apps/mihomo-subscription/0.1.4/` 应用包（镜像 `quinlanhoo/mihomo-subscription:0.1.4`），将 `Cargo.toml`/`Cargo.lock` 与 `web/package.json` 升到 `0.1.4`。

## [0.1.3] - 2026-06-14

### 修复

- 修复容器在 1Panel / 任何 `./data:/data` 绑定挂载上启动失败，出现 `unable to open database file`（SQLite 代码 14）的问题。镜像以非特权 `appuser` 运行，但绑定挂载用主机目录的（通常 root 拥有的）所有权覆盖了构建时的 `chown appuser /data`，因此进程无法创建 SQLite 文件。容器现在以 root 启动，新的 `docker-entrypoint.sh` `chown`s `DATA_DIR` 并通过 `gosu`（添加到运行时镜像）以 `appuser` 重新执行应用，在使绑定挂载数据目录开箱即用的同时保持最小特权运行时。添加了 `apps/mihomo-subscription/0.1.3/` 包（镜像 `quinlanhoo/mihomo-subscription:0.1.3`）并将 `Cargo.toml`/`Cargo.lock` 升级到 `0.1.3`。

### 文档

- 在 `docs/release.md` 中添加了“创建 GitHub Release”步骤（`gh release create vX.Y.Z --verify-tag`，说明从变更日志版本部分提取），并在发布后检查清单中添加了确认 Release 已发布的项目——发布流程之前在 git 标签处停止。

## [0.1.2] - 2026-06-14

### 新增

- 完成 1Panel 应用包并发布 `0.1.2`。新的 `apps/mihomo-subscription/0.1.2/` 包暴露了完整的安装表单——`ADMIN_USERNAME`/`ADMIN_PASSWORD`、`PUBLIC_BASE_URL`、`PUBLIC_PATH_PREFIX`、`RUST_LOG`、`FETCH_TIMEOUT_SECONDS`、`MAX_SUBSCRIPTION_SIZE_MB`、`CACHE_TTL_MINUTES`、`TRUSTED_PROXY_HOPS` 和 `SECURE_COOKIES` `auto`/`true`/`false` 选择器——其 `docker-compose.yml` 将每个变量传递到容器（镜像 `mihomo-subscription:0.1.2`）。将 `Cargo.toml`/`Cargo.lock` 升级到 `0.1.2`，将 README/CLAUDE/AGENTS 构建命令和 CI 1Panel-YAML 门指向新包，并清除了 `docs/1panel-app.md` 中的“包更新待定”标记。不完整的 `0.1.0/` 目录保留用于历史。
- 新的 `SECURE_COOKIES` 环境变量，用于强制 `Secure` 会话 cookie 属性。它默认从 `https://` 的 `PUBLIC_BASE_URL` 推断，因此在 TLS 终止反向代理后面（应用使用纯 HTTP，`PUBLIC_BASE_URL` 可能未设置或为 `http`），操作员现在可以明确选择加入。服务还在会话 cookie 最终没有 `Secure` 时记录启动警告（见 `src/main.rs`、`docs/technical-roadmap.md` 环境变量表）。

### 变更

- 将 1Panel 镜像策略从主机本地构建切换到 Docker Hub 镜像。`apps/mihomo-subscription/0.1.2/docker-compose.yml` 现在引用 `quinlanhoo/mihomo-subscription:0.1.2`（多架构 amd64+arm64），因此 1Panel 主机在安装时拉取镜像，而不是同步源代码并在本地构建。
- 移除了未使用的 `tokio-cron-scheduler` 依赖（`src/` 中未连接调度器），精简了依赖图和供应链面。
- 将 `sqlx` 从 0.7 升级到 0.8，并将其功能集从 `runtime-tokio-rustls` 切换到 `runtime-tokio`（SQLite 不需要 TLS）。这修复了 RUSTSEC-2024-0363 并删除了未使用的 rustls 堆栈，清除了三个 `rustls-webpki` 公告和 `rustls-pemfile`/`paste` 未维护警告。无需代码更改。
- 将固定窗口限流器替换为令牌桶（`src/rate_limit.rs`）。相同的 `max`/`window` 旋钮，但令牌连续补充，消除了固定窗口在其边界允许的约 2 倍突发，同时仍允许最多 `max` 的合法突发。

### 安全

- 加固了会话 cookie 签发：以前 `Secure` 属性仅在 `PUBLIC_BASE_URL` 以 `https://` 开头时设置，因此在 HTTPS 反向代理后面（`PUBLIC_BASE_URL` 未设置）的部署会静默签发没有 `Secure` 的会话 cookie（暴露于明文传输）。新的 `SECURE_COOKIES` 覆盖加启动警告关闭了该缺口。
- 添加了 `cargo audit` CI 门（`.github/workflows/ci.yml`），在 `.cargo/audit.toml` 中记录了每公告忽略（仅 `rsa`——由功能门控、从未编译的 `sqlx-mysql` 驱动程序拉取，无上游修复——和信息性的 `rustls-pemfile` 未维护通知）。新公告现在使构建失败。
- 在写入时（配置文件创建/更新）验证机场 `source_url`：预先拒绝非 http(s) 方案、嵌入凭据、环回主机名和阻止的字面 IP，返回通用 `400`。这是纵深防御和更清晰的错误——权威 SSRF 检查仍在获取时运行，具有 DNS 解析和 IP 固定（`src/fetch.rs`）。
- 在创建新会话时扫描过期会话（`src/auth.rs`），限制内存中的会话映射，使废弃/过期条目无法再累积（创建是唯一点）。
- 审计了数据库错误日志行（`src/error.rs`），确认 `sqlx` 错误显示从不包含绑定参数值（仅驱动程序/约束文本），因此机场 URL 和 token 不能通过它泄露；添加了注释以保持这种方式。

### 文档

- 重新设计了 `docs/release.md`，使多架构 `docker buildx ... --push` 到 Docker Hub 成为主要构建步骤（带有个人访问令牌登录说明和所需的 `docker-container` 构建器），并将主机 `docker build` 降级为离线/内网回退附录。相应更新了 `docs/1panel-app.md` 验证检查清单中的镜像引用项。
- 再次更新 `README.md` 用于 Docker Hub 部署：“在 1Panel 中部署”部分现在拉取发布的镜像，并将手写 Compose 作为主要的、可复制粘贴的路径——包含每个变量的完整环境块（必需/固定/可选分组，仅四个值标记为 `← edit`）加健康检查，1Panel 应用包安装降级为单行指针。状态横幅反映发布的 `0.1.2` 镜像和完整的 1Panel 安装表单。
- 将 `README.md` 重写为简洁的面向用户指南（约 185 行 → 约 109 行）：简短介绍，重点突出的“在 1Panel 中部署”部分（本地镜像构建、包含必需环境变量（包括 `SECURE_COOKIES`）的 Compose，以及主机头反向代理要求），以及简要使用演练。架构图、功能列表和完整开发部分被删除，改为指向 `docs/`。
- 在设计实现后删除了开发阶段规划文档 `docs/plan.md` 和 `docs/technical-roadmap.md`。其持久内容已并入维护文档：权威环境变量表移至 `docs/1panel-app.md`，转换器的顶级键处理移至 `docs/api-design.md`。`README.md`、`docs/` 和 `src/converter.rs` 中的所有交叉引用均已相应重定向。

## [0.1.1] - 2026-06-13

### 安全

- 加固了 YAML 别名扩展（“十亿笑”）：`src/yaml.rs` 现在在原始文本中计数 `&anchor`/`*alias` 令牌，并在 `serde_yaml` 解析前拒绝超过小上限的文档（炸弹很小且在解析器内扩展，因此大小/深度/节点检查无济于事）。适用于管理员节点/分组内容和获取的机场 YAML。
- 使公共下载限流节制 token 枚举：限流器现在仅按客户端 IP 键控（不是 IP + 路径），因此从一个 IP 猜测许多不同 token 共享单一预算，产生 `404` 的扫描被节流。
- 从未经认证的 `/health` 响应中移除了版本号（现在仅返回 `{"status":"ok"}`），以避免版本泄露。

### 文档

- 在 `docs/1panel-app.md` 中记录了反向代理 `Host` 透传要求（如果代理重写 `Host`，`Origin` 检查会对状态更改请求返回 403），并更新了 `docs/security-design.md` 的预解析锚点/别名上限和每 IP 下载限制。

## [0.1.0] - 2026-06-13

### 新增

- 将 `Dockerfile` 重新设计为三阶段构建：`node:20-slim` 阶段构建 SPA（`web/dist`），Rust 阶段编译二进制文件（现在复制 `migrations/` 以便 `sqlx::migrate!` 可以嵌入它们），运行时镜像仅包含二进制文件加构建资产（通过 `WEB_DIR` 提供）。将构建镜像升级到 `rust:1.90-slim`（传递依赖现在需要 edition2024）。扩展了 `.dockerignore`。冒烟测试：`/health` 正常，SPA 提供，未经认证的 `/api` 返回 `401`。1Panel 应用包更新有意推迟。
- 构建了配置文件详情页和编辑器（前端步骤 2）：托管链接头（复制、QR、重置 token 带确认，以及从最新子资源修改与 `last_generated_at` 客户端派生的“已修改但未生成”横幅）；六个配置卡（基本信息、源带屏蔽 URL/最后获取状态/只写 URL 替换/手动刷新、自定义节点和分组 CRUD、CodeMirror 规则编辑器和输出预览）；以及生成页脚，将逐项 `400` 验证错误映射回编辑器——规则行错误获得点击跳转到 CodeMirror 编辑器。在配置文件 API 响应中添加了 `last_generated_at`（`src/profiles.rs` 中的相关子查询）以驱动横幅，与 `docs/api-design.md` 对齐。新 Web 依赖：`@uiw/react-codemirror`、`@codemirror/lang-yaml`、`@codemirror/state`。
- 搭建了 `web/` SPA（Vite + React + TypeScript + Ant Design + react-i18next）：`/login`、`/`（配置文件列表）、`/profiles/:id`（带托管链接复制 + QR 的骨架详情）和 `/settings`（带类型确认的公共路径重置）的路由；一个获取客户端，其 `401` 处理器清除会话，使 `RequireAuth` 重定向到 `/login` 保留返回路径；Vite 开发服务器将 `/api` 和 `/health` 代理到后端。添加了前端 CI 作业（`npm ci` + `npm run build`）。完整配置卡和编辑器在下一步。
- 实现了客户端 IP 派生和限流：`src/net.rs` 从 `X-Forwarded-For` 从右边计数 `TRUSTED_PROXY_HOPS` 派生客户端 IP（忽略伪造的左边条目；回退到 TCP 对等），完全单元测试；`src/rate_limit.rs` 添加了内存中固定窗口限流器加登录（每 IP）和公共下载（每 IP+路径）中间件。`main` 提供 connect-info 以便 TCP 对等可用，读取 `TRUSTED_PROXY_HOPS`，并配置限流器。这也提供了从认证步骤推迟的登录失败限流。每配置文件刷新限流由单发锁加缓存 TTL 结构性提供。
- 实现了生成、预览和公共订阅端点（`src/generate.rs`）：`generate`（和源卡手动刷新）通过注入的 `SubscriptionFetcher` 获取，转换，持久化 `generated_cache`，并更新 `last_fetch_*`；`preview` 是只读的（无缓存写入，无 `last_fetch_*` 更改）；公共端点提供新鲜缓存，在每配置文件单发锁下刷新（`src/single_flight.rs`），在刷新失败时回退到陈旧缓存，当无缓存且获取失败时返回通用 `503`，以及对错误前缀/未知 token/禁用配置文件的统一 `404`（恒定时间前缀比较，始终运行 token 查找）。添加了记录的响应头（`subscription-userinfo` 透传、`profile-update-interval`、`content-disposition`）。获取抽象在 `SubscriptionFetcher` 后面（生产中的真实 `HttpFetcher`），因此路径在没有网络的情况下测试；`tests/generate.rs` 覆盖缓存命中、单发合并、陈旧回退、`503` 和统一 `404`。新环境变量连接：`FETCH_TIMEOUT_SECONDS`、`MAX_SUBSCRIPTION_SIZE_MB`、`CACHE_TTL_MINUTES`。
- 实现了 `mihomo`/`clash` -> `mihomo` 转换器（MVP 发布门）：`src/converter.rs` 解析机场 YAML（有界），追加启用的自定义节点/分组，替换 `rules`，剥离 `proxy-providers`，并透传 `rule-providers` 和所有其他顶级键。生成时验证返回逐项错误列表：自定义分组/机场分组名称冲突、自定义节点/机场代理冲突、规则策略目标和分组成员未解析到已知代理/分组/内置。逻辑/嵌套规则（带括号）无目标验证通过。九个夹具单元测试覆盖追加/替换/透传/剥离/冲突/悬挂引用。
- 实现了 SSRF 保护的机场获取（MVP 发布门）：`src/ssrf.rs` 无网络、表测试 URL/IP 验证覆盖每个阻止的 IPv4/IPv6 范围加 IPv4 映射/NAT64/6to4 解包绕过；`src/fetch.rs` 执行每跳验证、主机解析带验证 IP 固定（DNS 重绑定安全）、手动重定向重新验证（最多 3）、连接/总超时、流式响应大小限制（不是 `Content-Length`）、二进制内容类型拒绝和 `subscription-userinfo` 清理。`FetchError` 映射到 `last_fetch_status` 标签以供生成步骤重用。
- 确定了集成测试基线和 CI（骨架步骤 4）：添加了 `.github/workflows/ci.yml` 运行 `cargo fmt --check`、`cargo clippy --all-targets -D warnings`、`cargo test` 和 1Panel 应用包 YAML 验证；将 `tests/db_cascade.rs` 去重到共享的 `tests/common` 助手。基于 `ServiceExt` 的认证和配置文件套件（21 个测试）作为回归基线。
- 实现了配置文件 CRUD 和子资源（骨架步骤 3）：配置文件（创建/列表/详情/更新/删除）加规则（替换）、自定义节点和分组（CRUD）、重置 token、设置读取和重置公共路径，全部在会话认证下。机场 URL 是只写的，并确定性屏蔽（`src/mask.rs`）；托管链接从实时公共路径前缀（现在是 `AppState` 中的 `RwLock`，由重置公共路径更新）加每配置文件 token 组装。添加了 `src/error.rs`（错误信封、UNIQUE→409 映射）、`src/yaml.rs`（管理员节点内容的深度/节点计数解析）、`src/util.rs`（时间戳、随机 token/前缀）、`src/profiles.rs`、`src/settings.rs` 和 `tests/profiles.rs`。转换端点（生成/预览/公共）留待后续步骤。
- 实现了会话认证和同源静态服务（骨架步骤 2）：`src/auth.rs` 具有恒定时间凭据验证（基于摘要，无长度泄露）、内存中会话存储（256 位 ID，7 天空闲过期）、登录/注销/会话处理器、`require_session` 中间件（否则 `401`），以及状态更改请求上的 `Origin` 检查；`src/app.rs` 组装路由器，无 CORS 层，1 MB 正文限制，SPA `ServeDir` 回退；`main.rs` 现在在没有 `ADMIN_USERNAME`/`ADMIN_PASSWORD` 时拒绝启动，并在 HTTPS `PUBLIC_BASE_URL` 下启用 `Secure` cookie。添加了 `tests/auth.rs` 和共享的 `tests/common` 助手。登录失败限流推迟到限流任务。
- 开始实现记录的设计（骨架步骤 1）：添加了 `migrations/0001_init.sql` 创建目标模式并删除原型 `subscriptions` 表；添加了 `src/db.rs` 模块，打开 SQLite 池，每连接 `foreign_keys`/`busy_timeout`/WAL pragma，运行迁移，并种子 `app_settings` 公共路径前缀；添加了 `src/lib.rs` 以便集成测试可以使用 crate；添加了 `tests/db_cascade.rs` 证明配置文件删除级联到所有子表（且外键 pragma 在池化连接中保持）。
- 在 `docs` 下初始化了项目文档。
- 添加了 1Panel 应用打包说明。
- 添加了 Mihomo 订阅转换服务的技术路线图。
- 添加了涵盖公共链接、管理员认证、SSRF 保护、敏感数据处理和缓存的安全设计。
- 添加了涵盖 MVP 范围、自定义规则、自定义节点、自定义代理组、永久链接和 1Panel 部署期望的产品计划。

### 变更

- 阐明永久公共订阅链接应同时使用随机公共路径前缀和每配置文件 token。
- 将计划产品范围从订阅 URL CRUD 扩展到基于配置文件的 Mihomo 订阅转换和分发。

### 修复

- 更新了 Axum 服务启动代码以兼容 Axum 0.7。
- 在运行时镜像中安装了 `wget` 以便健康检查可以运行。

### 安全

- 记录了机场订阅获取的 SSRF 保护要求。
- 在并发、部署拓扑和存储正确性方面关闭了第二轮设计审查缺口：每配置文件单发锁防止陈旧缓存刷新踩踏；在 1Panel 反向代理后面通过 `TRUSTED_PROXY_HOPS` 正确获取客户端 IP（添加到环境变量表）；始终执行、恒定时间公共 token 查找以避免路径前缀的时序泄露；管理请求正文大小限制（`413`），管理员提交内容使用相同的 YAML 解析限制；以及每连接 SQLite pragma（`foreign_keys`、`busy_timeout`）通过 after-connect 钩子应用，使 `ON DELETE CASCADE` 不被静默禁用。
- 扩展了测试策略，包括级联删除、`503`、`413` 和单发并发案例。

### 文档

- 将项目状态从规划翻转为已实现：从 `api-design.md` 和 `data-model.md` 中删除了“状态：规划阶段”横幅（现在已实现），将 `release.md` 和 `1panel-app.md` 横幅重新措辞为“尚未发布/包更新待定”，刷新了根 `README.md` 和 `docs/README.md` 中的状态部分，并删除了过时的原型路由兼容性说明。变更日志版本滚动到带日期的 `0.1.0` 有意推迟，直到 1Panel 应用包更新且实际发布。
- 在安全审查后加固了 SSRF 设计：阻止了 IPv4 嵌入的 IPv6 范围（IPv4 映射、NAT64、6to4）带嵌入地址重新检查，要求验证 IP 固定以防止 DNS 重绑定，要求响应大小限制计数字节流而不是 `Content-Length`，并添加了 TEST-NET/6to4 中继 IPv4 范围。
- 添加了不受信任内容处理部分：YAML 别名/嵌套解析限制，存储或回显前的 `subscription-userinfo` 格式验证，以及 Web UI 中机场提供名称的转义。
- 加强了认证设计：恒定时间凭据比较，最小会话 ID 熵，管理 API 的同源/无 CORS 策略（原型的宽松 CORS 层必须在认证落地时移除），以及 `Origin` 验证作为 CSRF 纵深防御。
- 记录了原始机场订阅 URL 的屏蔽要求。
- 记录了管理员登录要求和 1Panel 基于 compose 的凭据配置。
- 添加了此变更日志模板和初始未发布条目。
- 添加了文档维护指导，要求受影响的项目文档随每次变更保持对齐。
- 记录了计划的 Web UI 结构：托管链接头、Mihomo 配置卡和生成链接模态框。
- 为登录管理页面要求更新了产品、安全、技术和 1Panel 文档。
- 添加了 `docs/api-design.md` 定义目标管理 API、认证流程、验证规则和公共订阅端点契约（双语）。
- 添加了 `docs/data-model.md` 定义目标 SQLite 模式、索引和迁移策略（双语）。
- 添加了 `docs/release.md` 定义版本控制、发布前检查、镜像构建、1Panel 应用包更新和变更日志滚动步骤（双语）。
- 添加了根 `README.md`，包含项目状态、计划功能、架构概述和文档索引（双语）。
- 更新了 `docs/README.md` 将计划文档移入已发布文档列表。
- 添加了变更规则，要求每次变更后同步更新代理指导文档，使其与实现保持对齐。
- 添加了 MIT `LICENSE`，在 `Cargo.toml` 中声明 `license = "MIT"`，并在根 `README.md` 中添加了许可部分。
- 决定采用本地镜像策略：compose 镜像现在是 `mihomo-subscription:0.1.0`（在 1Panel 主机上构建，无远程注册表）；相应重新设计了 `docs/release.md`，带有可选推送附录。
- 添加了生成的占位符 `apps/mihomo-subscription/logo.png`（180x180）；在公共分发前替换为真实设计。
- 为本地镜像名称和 logo 状态更新了 `docs/1panel-app.md`。
- 在 `docs/1panel-app.md` 中添加了规划状态横幅，并将尚未满足的验证检查清单项标记为待定，修复了与实际应用包内容的不匹配。
- 在 `docs/technical-roadmap.md` 中添加了权威环境变量表，包括先前未定义的 `CACHE_TTL_MINUTES`，并将其缓存 TTL 措辞与 `docs/security-design.md` 和 `docs/data-model.md` 对齐。
- 在 `docs/technical-roadmap.md` 中记录了前端构建管道：`web/` 目录布局、Vite 开发代理、Axum 静态服务带 SPA 回退，以及 Node Docker 构建阶段。
- 在 `docs/technical-roadmap.md` 中记录了转换器顶级键处理：默认透传，MVP 中为 SSRF 和 URL 暴露原因剥离 `proxy-providers`。
- 在 `docs/technical-roadmap.md` 中添加了测试策略，转换器和 SSRF 验证器套件作为 MVP 发布的硬门。
- 在 `docs/api-design.md` 和 `docs/plan.md` 中记录了客户端兼容性行为：`subscription-userinfo` 透传（存储在生成的缓存中；`docs/data-model.md` 中的新列）、`profile-update-interval` 和 `content-disposition` 头。
- 在实现前定义了 `docs/api-design.md` 中的其余 API 边缘语义：源卡的手动刷新重用生成端点，预览是只读的（新鲜缓存或实时获取，永不持久化），公共端点在刷新失败时返回陈旧缓存或无缓存时的通用 `503`，以及自定义节点和分组的请求正文形状。
- 在 `docs/security-design.md` 中指定了会话存储（内存中，7 天空闲过期）和确定性 URL 屏蔽规则（屏蔽每个查询参数值），并将其错误处理部分与实现对齐。
- 在 `docs/security-design.md` 中添加了速率限制和滥用控制部分：每 IP 登录限制、每 IP+路径公共下载限制（枚举/扫描共享预算）、每配置文件刷新限制、客户端 IP 从 `X-Forwarded-For` 从右边派生（`TRUSTED_PROXY_HOPS`），以及内存中限制的首次部署可接受性。
- 将 `docs/security-design.md` 中的缓存建议与实现对齐：每拉取刷新、每配置文件单发锁、`CACHE_TTL_MINUTES` 用于管理员预览、陈旧缓存回退和内容哈希缓存键。
- 在 `docs/data-model.md` 中记录了 `last_fetch_*` 和 `subscription_userinfo` 列，更新了 `docs/api-design.md` 的源卡响应，并添加了 `last_generated_at` 到配置文件响应。
- 在 `docs/security-design.md` 中添加了缓存和刷新策略部分：避免在每个公共下载上获取，每拉取刷新，每配置文件单发锁，以及 `CACHE_TTL_MINUTES` 用于管理员预览。
- 在 `docs/security-design.md` 中添加了错误处理部分：公共端点统一 `404`、陈旧缓存回退、通用 `503`、无上游细节；管理 API 有用验证错误，无秘密。
- 在 `docs/security-design.md` 中添加了安全检查清单：管理 API 认证、`PUBLIC_PATH_PREFIX` 和 token、加密安全 token、SSRF URL/IP 阻止、重定向重新验证、超时/大小限制、YAML 解析限制、恒定时间凭据、无宽松 CORS、机场 URL 屏蔽、公共端点统一 `404`、缓存防止重复获取。
- 在 `docs/api-design.md` 中添加了公共订阅端点：`GET /<prefix>/api/sub/<token>`、响应头（`subscription-userinfo`、`profile-update-interval`、`content-disposition`）、`404` 失败、`503` 回退。
- 在 `docs/api-design.md` 中添加了管理 API 端点：配置文件 CRUD、规则 CRUD、自定义节点/分组 CRUD、生成/预览、重置 token、设置读取、重置公共路径。
- 在 `docs/api-design.md` 中添加了认证：`ADMIN_USERNAME`/`ADMIN_PASSWORD` 环境变量、会话 cookie、`HttpOnly`/`SameSite`/`Secure`、`Origin` 检查、登录/注销/会话端点。
- 在 `docs/api-design.md` 中添加了转换器行为：追加自定义节点，替换规则，剥离 `proxy-providers`，透传 `rule-providers` 和其他键。
- 在 `docs/data-model.md` 中添加了 SQLite 模式：`profiles`、`rulesets`、`custom_nodes`、`custom_groups`、`generated_cache`、`app_settings`。
- 在 `docs/data-model.md` 中添加了索引和迁移策略。
- 在 `docs/release.md` 中添加了版本控制、发布前检查、镜像构建、1Panel 应用包更新和变更日志滚动步骤。
- 在 `docs/1panel-app.md` 中添加了 1Panel 本地应用打包：目录布局、本地安装路径、环境变量表、验证检查清单。
- 添加了 MIT `LICENSE`。
- 添加了 `.gitignore`。
- 添加了 `Cargo.toml`。
- 添加了 `src/main.rs`。
- 添加了 `src/lib.rs`。
- 添加了 `src/db.rs`。
- 添加了 `src/auth.rs`。
- 添加了 `src/app.rs`。
- 添加了 `src/profiles.rs`。
- 添加了 `src/settings.rs`。
- 添加了 `src/generate.rs`。
- 添加了 `src/converter.rs`。
- 添加了 `src/ssrf.rs`。
- 添加了 `src/fetch.rs`。
- 添加了 `src/yaml.rs`。
- 添加了 `src/util.rs`。
- 添加了 `src/error.rs`。
- 添加了 `src/mask.rs`。
- 添加了 `src/net.rs`。
- 添加了 `src/rate_limit.rs`。
- 添加了 `src/single_flight.rs`。
- 添加了 `migrations/0001_init.sql`。
- 添加了 `tests/auth.rs`。
- 添加了 `tests/profiles.rs`。
- 添加了 `tests/db_cascade.rs`。
- 添加了 `tests/generate.rs`。
- 添加了 `tests/common/mod.rs`。
- 添加了 `web/` 目录。
- 添加了 `web/package.json`。
- 添加了 `web/tsconfig.json`。
- 添加了 `web/vite.config.ts`。
- 添加了 `web/index.html`。
- 添加了 `web/src/main.tsx`。
- 添加了 `web/src/App.tsx`。
- 添加了 `web/src/i18n.ts`。
- 添加了 `web/src/fetch.ts`。
- 添加了 `web/src/pages/Login.tsx`。
- 添加了 `web/src/pages/ProfileList.tsx`。
- 添加了 `web/src/pages/ProfileDetail.tsx`。
- 添加了 `web/src/pages/Settings.tsx`。
- 添加了 `web/src/components/RequireAuth.tsx`。
- 添加了 `web/src/components/HostedLinkHeader.tsx`。
- 添加了 `web/src/components/BasicInfoCard.tsx`。
- 添加了 `web/src/components/SourceCard.tsx`。
- 添加了 `web/src/components/CustomNodesCard.tsx`。
- 添加了 `web/src/components/CustomGroupsCard.tsx`。
- 添加了 `web/src/components/RulesCard.tsx`。
- 添加了 `web/src/components/OutputPreviewCard.tsx`。
- 添加了 `web/src/components/GenerateFooter.tsx`。
- 添加了 `.github/workflows/ci.yml`。
- 添加了 `Dockerfile`。
- 添加了 `.dockerignore`。
- 添加了 `docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/` 目录。
- 添加了 `apps/mihomo-subscription/data.yml`。
- 添加了 `apps/mihomo-subscription/README.md`。
- 添加了 `apps/mihomo-subscription/README_en.md`。
- 添加了 `apps/mihomo-subscription/logo.png`。
- 添加了 `apps/mihomo-subscription/0.1.0/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.0/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.0/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.0/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.2/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.2/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.2/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.2/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.3/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.3/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.3/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.3/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.4/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.4/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.4/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.4/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.5/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.5/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.5/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.5/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.6/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.6/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.6/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.6/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.7/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.7/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.7/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.7/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.8/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.8/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.8/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.8/data/` 目录。
- 添加了 `docs/` 目录。
- 添加了 `docs/README.md`。
- 添加了 `docs/api-design.md`。
- 添加了 `docs/data-model.md`。
- 添加了 `docs/security-design.md`。
- 添加了 `docs/1panel-app.md`。
- 添加了 `docs/release.md`。
- 添加了 `docs/changelog.md`。
- 添加了 `README.md`。
- 添加了 `LICENSE`。
- 添加了 `.gitignore`。
- 添加了 `Cargo.toml`。
- 添加了 `Cargo.lock`。
- 添加了 `src/main.rs`。
- 添加了 `src/lib.rs`。
- 添加了 `src/db.rs`。
- 添加了 `src/auth.rs`。
- 添加了 `src/app.rs`。
- 添加了 `src/profiles.rs`。
- 添加了 `src/settings.rs`。
- 添加了 `src/generate.rs`。
- 添加了 `src/converter.rs`。
- 添加了 `src/ssrf.rs`。
- 添加了 `src/fetch.rs`。
- 添加了 `src/yaml.rs`。
- 添加了 `src/util.rs`。
- 添加了 `src/error.rs`。
- 添加了 `src/mask.rs`。
- 添加了 `src/net.rs`。
- 添加了 `src/rate_limit.rs`。
- 添加了 `src/single_flight.rs`。
- 添加了 `migrations/0001_init.sql`。
- 添加了 `tests/auth.rs`。
- 添加了 `tests/profiles.rs`。
- 添加了 `tests/db_cascade.rs`。
- 添加了 `tests/generate.rs`。
- 添加了 `tests/common/mod.rs`。
- 添加了 `web/` 目录。
- 添加了 `web/package.json`。
- 添加了 `web/tsconfig.json`。
- 添加了 `web/vite.config.ts`。
- 添加了 `web/index.html`。
- 添加了 `web/src/main.tsx`。
- 添加了 `web/src/App.tsx`。
- 添加了 `web/src/i18n.ts`。
- 添加了 `web/src/fetch.ts`。
- 添加了 `web/src/pages/Login.tsx`。
- 添加了 `web/src/pages/ProfileList.tsx`。
- 添加了 `web/src/pages/ProfileDetail.tsx`。
- 添加了 `web/src/pages/Settings.tsx`。
- 添加了 `web/src/components/RequireAuth.tsx`。
- 添加了 `web/src/components/HostedLinkHeader.tsx`。
- 添加了 `web/src/components/BasicInfoCard.tsx`。
- 添加了 `web/src/components/SourceCard.tsx`。
- 添加了 `web/src/components/CustomNodesCard.tsx`。
- 添加了 `web/src/components/CustomGroupsCard.tsx`。
- 添加了 `web/src/components/RulesCard.tsx`。
- 添加了 `web/src/components/OutputPreviewCard.tsx`。
- 添加了 `web/src/components/GenerateFooter.tsx`。
- 添加了 `.github/workflows/ci.yml`。
- 添加了 `Dockerfile`。
- 添加了 `.dockerignore`。
- 添加了 `docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/` 目录。
- 添加了 `apps/mihomo-subscription/data.yml`。
- 添加了 `apps/mihomo-subscription/README.md`。
- 添加了 `apps/mihomo-subscription/README_en.md`。
- 添加了 `apps/mihomo-subscription/logo.png`。
- 添加了 `apps/mihomo-subscription/0.1.0/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.0/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.0/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.0/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.2/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.2/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.2/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.2/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.3/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.3/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.3/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.3/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.4/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.4/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.4/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.4/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.5/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.5/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.5/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.5/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.6/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.6/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.6/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.6/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.7/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.7/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.7/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.7/data/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.8/` 目录。
- 添加了 `apps/mihomo-subscription/0.1.8/data.yml`。
- 添加了 `apps/mihomo-subscription/0.1.8/docker-compose.yml`。
- 添加了 `apps/mihomo-subscription/0.1.8/data/` 目录。