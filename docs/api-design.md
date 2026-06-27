# API 设计

> 管理 API、认证、生成/预览、公开订阅端点均已实现;实现取舍见 `changelog.md`。

相关:`security-design.md`、`data-model.md`、`1panel-app.md`。

## 原则

- 管理 API 挂 `/api`、需登录会话、用 JSON;公开订阅端点无需登录(需随机路径前缀 +
  per-profile token)、输出 Mihomo YAML。
- 公开端点任何校验失败统一 `404`(不泄因);管理 API 默认对机场 URL 脱敏。
- 管理 API **同源**(SPA 由 Axum 同源提供,不启用 CORS;见 `security-design.md`)。
- 管理请求体 ≤1MB(超限 `413`);管理员提交的节点/分组 YAML 与机场内容用相同别名/嵌套限制解析。

## 约定

- 时间 RFC 3339 UTC;标识符 UUID v4;编码 `application/json`(公开端点 `text/yaml`)。

错误响应:`{ "error": { "code", "message", "details": [...] } }`。

| Code | 含义 |
|------|------|
| 200/201/204 | 成功(读/建/无内容) |
| 400 | 请求体格式错或校验失败 |
| 401 | 未登录/会话过期 |
| 404 | 资源不存在;公开端点统一失败响应 |
| 409 | 名称冲突(如重名) |
| 413 | 请求体超限 |
| 429 | 限流 |
| 500 | 内部错误(不含敏感信息) |

## 认证

凭据来自 `ADMIN_USERNAME` / `ADMIN_PASSWORD`(compose 环境变量)。登录签发会话 Cookie
(`HttpOnly`、`SameSite=Lax`,HTTPS 加 `Secure`);登录失败按 IP + 账户限流。除 `/health`、
登录、公开端点外,所有路由需有效会话(否则 `401`)。

```text
POST /api/auth/login  {username,password} -> 204+Set-Cookie | 401 | 429
POST /api/auth/logout -> 204 清除 cookie
GET  /api/auth/session -> 200 {username} | 401
```

## 端点总览

| Method | Path | 鉴权 | 说明 |
|--------|------|------|------|
| GET | `/health` | 否 | 健康检查 |
| POST | `/api/auth/login` | 否 | 登录 |
| POST | `/api/auth/logout` | 是 | 登出 |
| GET | `/api/auth/session` | 是 | 当前会话 |
| GET / POST | `/api/profiles` | 是 | 配置列表 / 创建 |
| GET / PUT / DELETE | `/api/profiles/:id` | 是 | 配置详情 / 更新 / 删除 |
| PUT | `/api/profiles/:id/rules` | 是 | 替换自定义规则 |
| GET | `/api/profiles/:id/provider-rules` | 是 | 拉取机场原始 `rules`(实时,不缓存) |
| GET | `/api/profiles/:id/proxies` | 是 | 节点/分组预览(生成输出中的代理与分组,只读) |
| PUT | `/api/profiles/:id/node-section-order` | 是 | 两个节点块的先后(`["provider","custom"]` 排列) |
| PUT | `/api/profiles/:id/group-order` | 是 | 分组顺序(分组名数组) |
| GET / POST | `/api/global-nodes` | 是 | 全局自定义节点池(跨订阅共享,自动追加到每条配置) |
| PUT / DELETE | `/api/global-nodes/:id` | 是 | 单个全局节点 |
| PUT | `/api/global-nodes/order` | 是 | 全局自定义块顺序;立即重排所有配置缓存 |
| GET / POST | `/api/rule-sets` | 是 | 全局用户规则库(② 模板 / 导入源;不托管、不参与生成) |
| PUT / DELETE | `/api/rule-sets/:id` | 是 | 单个全局规则集 |
| PUT | `/api/rule-sets/order` | 是 | 全局库显示顺序(仅展示序) |
| GET / POST | `/api/profiles/:id/rule-sets` | 是 | 订阅自有规则库(③);生成只读此处 |
| PUT / DELETE | `/api/profiles/:id/rule-sets/:rsid` | 是 | 单个订阅规则集 |
| POST | `/api/profiles/:id/rule-sets/import` | 是 | 从全局 ② 复制规则集进本订阅 ③ + 追加 `RULE-SET` 规则行 |
| GET / POST | `/api/profiles/:id/groups` | 是 | 自定义分组 |
| PUT / DELETE | `/api/profiles/:id/groups/:group_id` | 是 | 单个分组 |
| POST | `/api/profiles/:id/import-provider-groups` | 是 | 导入机场 `proxy-groups` 为可编辑自定义分组(实时,跳过同名/不支持类型) |
| GET | `/api/profiles/:id/preview` | 是 | 预览生成的 YAML |
| POST | `/api/profiles/:id/generate` | 是 | 校验并生成 |
| POST | `/api/profiles/:id/reset-token` | 是 | 重置该配置 token |
| GET | `/api/settings` | 是 | 应用设置 |
| POST | `/api/settings/reset-public-path` | 是 | 重置公共路径前缀 |
| GET | `/:public_path_prefix/api/sub/:token` | 否 | 公开订阅下载 |
| GET | `/:public_path_prefix/api/sub/:token/r/:name/:file` | 否 | 公开规则集托管(③,按订阅 token 隔离;`:file` = `<behavior>.<format>`) |

## Profile 资源

列表返回摘要,详情返回完整对象。`source_url` 默认脱敏,仅创建/更新接受完整值。详情的
`nodes` 是**全局节点池的只读快照**(各配置一致,编辑/排序走 `/api/global-nodes`),供详情
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

- 创建:`{name, source_url}`。创建成功后**同步触发一次生成/拉取**(尽力而为),故新订阅
  立即带有真实 `last_fetch_status`,不存在「未拉取」中间态。
- `source_url` 写时静态校验(http/https、无内嵌凭据、非本地/私有地址),否则 `400`;真正 SSRF
  在拉取时按 DNS 解析 + IP 固定(见 `security-design.md`)。
- `last_fetch_status`:`success` / `http_error:<code>` / `ssrf_rejected` / `timeout` / `too_large`,
  从未拉取为 `null`。

请求体(节点走全局池,分组按配置):

```text
POST /api/global-nodes { name, node_type, content, enabled? }  # name 全局唯一(重名 409);content 为完整 Mihomo proxy 映射 YAML
POST /api/profiles/:id/groups { name, group_type, members, options?, enabled? }
```

- 节点 `content` 保存时结构校验,生成时原样并入**每条配置**输出的 `proxies`;`PUT` 同体整体替换。
- 全局节点为单一共享池:新建落末尾、`name` 全局唯一;增删改在下次生成(公共链接每拉取即重生)
  进入各配置输出,排序见下立即生效。

## 规则集库(三规则库模型)

规则下发与订阅**解耦**为三个库:① 机场原始规则(`GET .../provider-rules`,「导入机场规则」)、
② 全局用户规则库(`/api/rule-sets`)、③ 每订阅托管规则库(`/api/profiles/:id/rule-sets`)。**下发只读
③**;①② 仅作导入源。

### ② 全局用户规则库(模板 / 导入源)

```text
GET    /api/rule-sets                                  # 列表;每项含 count、source、remote_url_masked、cache(无托管链接)
POST   /api/rule-sets   { name, behavior, source?, format, ... }
   # name 全局唯一(重名 409)且限 [A-Za-z0-9._-];behavior∈domain/ipcidr/classical;source∈manual(默认)/remote
   # source=manual: { content }                 format∈yaml/text
   # source=remote: { url, interval_hours?=24, cache?=true }   format∈yaml/text/mrs;url 须 http(s)
PUT    /api/rule-sets/:id   { ...同上 }                 # remote 编辑 url 留空则沿用原值(已脱敏不回显)
DELETE /api/rule-sets/:id
PUT    /api/rule-sets/order { order: [规则集名] }        # 仅展示序,未列出落末尾
```

② 不再公开托管、不再参与生成,仅是可复用模板。通过 ③ 的导入端点复制进某订阅后才生效。

### ③ 每订阅托管规则库(下发来源)

```text
GET    /api/profiles/:id/rule-sets                     # 列表;每项含 url(按订阅 token 隔离的托管链接)、count、source、remote_url_masked、cache、last_fetch_status
POST   /api/profiles/:id/rule-sets   { name, behavior, source?, format, ... }   # name 在本订阅内唯一(重名 409);字段同 ②
PUT    /api/profiles/:id/rule-sets/:rsid   { ...同上 }
DELETE /api/profiles/:id/rule-sets/:rsid
POST   /api/profiles/:id/rule-sets/import  { names: [②规则集名], policy }   # 复制 ② 定义进 ③(含真实远程 URL)+ 为未引用名追加 RULE-SET,<name>,<policy> 行;返回 { imported }
```

- manual / remote 行为与 ② 一致(校验/渲染/镜像同一套逻辑)。托管在按订阅 token 隔离的链接
  `/<prefix>/api/sub/<token>/r/<name>/<behavior>.<format>`;remote 关缓存则转换时直接注入上游 URL。
- 被本订阅 `RULE-SET,<name>` 规则引用时注入指向该托管链接的 `rule-providers:` 条目(公共链接每拉取即
  重生,故定义改动即时生效);未被引用不注入。
- 注入时若名与机场 `rule-providers` 已有条目**撞名**,用本订阅托管版**覆盖**机场版;生成端点
  (`POST .../generate`)在响应 `ruleset_conflicts` 列出撞名的名字,详情页据此告警,避免静默替换。

## 节点/分组预览与排序

```text
GET /api/profiles/:id/proxies
{ "generated": true, "generated_at": "...",
  "proxies": [{ "name":"hk-1","type":"ss" }, { "name":"my-ss","type":"ss" }],
  "node_section_order": ["provider","custom"],
  "groups": [{ "name":"Proxy","type":"select" }] }
```

只读,解析自 `generated_cache.output_yaml`,直接返回缓存当前内容(排序改动会就地重写缓存);
未生成返回 `generated:false` + 空数组。`proxies` = 机场块 + 自定义块按 `node_section_order` 拼接,
前端据全局节点名集合拆成两块渲染(机场只读,自定义在「节点配置」排序);`groups` 全为自定义
分组。两者也作分组成员候选。

```text
PUT /api/global-nodes/order               { order: [节点名] }              # 全局,未列出落末尾;重写为 0..n-1
PUT /api/profiles/:id/node-section-order  { order: ["custom","provider"] } # per-profile,必须是排列
PUT /api/profiles/:id/group-order         { order: [分组名] }              # per-profile
-> 均 204
```

- 自定义块顺序由全局 `global-nodes/order` 决定(作用所有配置);两块先后由 per-profile
  `node-section-order`;分组顺序由 per-profile `group-order`。名字超长/数组过大 `400`。
- 这些端点保存后**就地重写已生成缓存、无需重拉机场**,改动**立即生效**(预览与公共链接随即
  反映);全局排序重排**每条配置**缓存,无缓存者首次生成时生效。
- 规则拖拽同理:规则顺序即语义(命中即止),存为 `rulesets.content` 有序文本,前端经
  `PUT .../rules` 整体保存,同样就地重写缓存 `rules` 块、立即生效。
- 每次生成把输出的分组顺序快照回写 `group_order`(新增分组落末尾);节点顺序为全局
  `global_nodes.position`,不 per-profile 快照,机场块恒上游序。
- 节点/分组均结构化表单录入(节点常用字段 + 高级 KV;分组按类型给选项 + 高级 KV;成员从候选
  下拉选),前端分别序列化为节点 `content` YAML 与分组 `options` JSON。

## 生成与校验

- `POST .../generate` 完整校验,成功刷新缓存并返回托管链接,失败 `400` + 逐条错误(对应弹窗
  文案)。详情页「原始订阅源」手动刷新复用本端点。
- `GET .../preview` 是只读版:有新鲜缓存则返回,否则实时拉取生成;不写缓存、不动 `last_*`。
- 校验:规则引用的分组须存在于已启用自定义分组;自定义分组名可与机场分组重名(机场分组整体
  替换);分组成员须引用存在的机场节点(透传)/启用自定义节点/启用自定义分组;输出须合法
  Mihomo YAML。成功响应 `{ subscription_url, generated_at }`。

顶层键处理(转换器逐键显式处理,见 `src/converter.rs`):

| 键 | 处理 |
|-----|------|
| `proxies` | 机场块(上游序)+ 自定义块(启用全局节点按 `global_nodes.position` 排),按 `node_section_order` 拼接 |
| `proxy-groups` | 整体替换为启用的自定义分组(机场分组不透传,需「导入机场分组」),按 `group_order` 重排 |
| `rules` | 整体替换为用户规则 |
| `rule-providers` | 机场原样透传;另把被 `RULE-SET` 引用、本订阅自有(③)规则集合并在上(同名覆盖,指向按订阅 token 隔离的托管链接) |
| `proxy-providers` | **剥离**(会让客户端拉取绕过 SSRF/缓存的 URL,可能暴露机场 URL) |
| 其余(`port`/`dns`/`tun`/`sniffer`…) | 原样透传(新 Mihomo 选项无需改转换器) |

- `import-provider-groups`:实时拉取机场,把 `proxy-groups` 解析为可编辑自定义分组写入
  (`name`/`type`/`proxies`→成员,其余→`options`;跳过同名/不支持类型),返回 `{imported,skipped}`;
  只改 `custom_groups`,需重新生成才进输出;鉴权 + SSRF 同 `provider-rules`。
- 规则集:除透传机场自带 `rule-providers` 外,面板按 `RULE-SET,<name>,<policy>` 注入**本订阅自有规则库
  (③)**中同名条目(见「规则集库」),指向按订阅 token 隔离的托管链接;`<name>` 也可仍引用机场条目名。
  全局 ② 库不参与生成。

## 公开订阅端点

```text
GET /:public_path_prefix/api/sub/:token
  -> 200 text/yaml   有效前缀 + token + 配置启用
  -> 503             有效但无缓存且上游拉取失败(通用)
  -> 404             其余一切(统一)
```

响应头:`content-type: text/yaml`、`content-disposition: attachment; filename="<name>.yaml"`、
`subscription-userinfo`(从机场透传,无则省略)、`profile-update-interval: 24`(小时,MVP 固定)。

- **每次拉取都重拉机场重新生成**(永得最新节点),无 TTL 短路;并发拉取由 per-profile
  single-flight 合并为一次机场拉取。`generated_cache` 仅作拉取失败兜底(返陈旧缓存,无则
  `503`)。无「生成配置」按钮(链接始终实时);响应/错误绝不含机场 URL;按 token + 来源 IP 限流。

兼容:早期 `/api/v1/subscriptions*`、`/api/v1/merged` 已移除,无兼容层。

## 公开规则集托管端点

```text
GET /:public_path_prefix/api/sub/:token/r/:name/:file
  -> 200 text/plain | application/octet-stream(mrs)   前缀+token 匹配 + 名在该订阅存在且启用 + :file=="<behavior>.<format>" + 已托管
  -> 503             remote 镜像首拉失败且无缓存
  -> 404             其余一切(统一,含前缀错/token 错/名不存在/未启用/文件名不符/remote 关缓存未托管)
```

- 托管本订阅自有规则库(③),**按订阅 token 隔离**,与订阅端点共用 `public_path_prefix`(重置公共路径同样
  使其失效)。规则集是规则清单、非私密,按名可枚举可接受;仍按来源 IP 限流。全局 ② 库不再公开托管。
- manual:`yaml`→Mihomo `payload:` 列表、`text`→逐行(均忽略空行与 `#` 注释)。remote(`cache=1`):
  single-flight 懒刷新——缓存超 `interval_hours` 才回源(SSRF 安全字节),失败回退旧缓存、无缓存 `503`;
  字节原样托管(`mrs` 为 `application/octet-stream`)。remote(`cache=0`)不在此托管,统一 `404`。
