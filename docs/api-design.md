# API 设计

> **状态:已实现。** 本文档描述的管理 API、认证流程、生成/预览与公开订阅端点
> 均已实现;实现中的细微取舍记录在 `docs/changelog.md`。

相关文档:`security-design.md`、`data-model.md`、`1panel-app.md`。

## 设计原则

- 管理 API 统一挂载在 `/api` 下,要求登录会话。
- 公开订阅端点不要求登录,但要求随机路径前缀和 per-profile token。
- 管理 API 使用 JSON;公开订阅端点输出 Mihomo YAML。
- 公开端点的任何校验失败统一返回 `404`,不泄露失败原因。
- 管理 API 响应默认对原始机场订阅 URL 脱敏。
- 管理 API 仅限同源访问:SPA 由 Axum 同源提供,不启用 CORS 层
  (见 `security-design.md` 的 CORS and CSRF)。
- 所有管理请求体都有大小上限(默认 1 MB),超限返回 `413`。管理员提交的
  节点/分组 YAML 与机场内容用相同的别名/嵌套限制解析(见 `security-design.md`)。

## 通用约定

- 时间格式:RFC 3339 UTC,如 `2026-06-12T08:00:00Z`。
- 标识符:UUID v4 字符串。
- 请求与响应编码:`application/json; charset=utf-8`(公开端点为 `text/yaml`)。

错误响应格式:

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

状态码:

| Code | 含义 |
|------|------|
| 200 / 201 / 204 | 成功(读取 / 创建 / 无内容) |
| 400 | 请求体格式错误或校验失败 |
| 401 | 未登录或会话过期 |
| 404 | 资源不存在;公开端点的统一失败响应 |
| 409 | 名称冲突(如分组重名) |
| 413 | 请求体超过大小上限 |
| 429 | 触发限流 |
| 500 | 服务内部错误,不含敏感信息 |

## 认证

管理员凭据来自环境变量 `ADMIN_USERNAME` 和 `ADMIN_PASSWORD`(由 1Panel 安装表单
写入 compose)。登录成功后签发会话 Cookie:`HttpOnly`、`SameSite=Lax`,HTTPS 部署
时附加 `Secure`。登录失败按 IP 和账户限流。

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

## 端点总览

| Method | Path | 鉴权 | 说明 |
|--------|------|------|------|
| GET | `/health` | 否 | 健康检查 |
| POST | `/api/auth/login` | 否 | 登录 |
| POST | `/api/auth/logout` | 是 | 登出 |
| GET | `/api/auth/session` | 是 | 当前会话 |
| GET | `/api/profiles` | 是 | 配置列表 |
| POST | `/api/profiles` | 是 | 创建配置 |
| GET | `/api/profiles/:id` | 是 | 配置详情 |
| PUT | `/api/profiles/:id` | 是 | 更新基础信息 |
| DELETE | `/api/profiles/:id` | 是 | 删除配置 |
| PUT | `/api/profiles/:id/rules` | 是 | 替换自定义规则 |
| GET | `/api/profiles/:id/provider-rules` | 是 | 拉取机场原始 `rules`(用于规则预览预填,实时拉取,不缓存) |
| GET | `/api/profiles/:id/proxies` | 是 | 节点/分组预览:生成输出中的全部代理与分组(name+type,机场+自定义,只读) |
| PUT | `/api/profiles/:id/node-order` | 是 | 保存**自定义块**内的节点顺序(自定义节点名数组) |
| PUT | `/api/profiles/:id/node-section-order` | 是 | 保存两个节点块的先后(`["provider","custom"]` 的排列) |
| PUT | `/api/profiles/:id/group-order` | 是 | 保存手动分组排序(分组名数组),决定生成 `proxy-groups` 与预览的顺序 |
| GET / POST | `/api/profiles/:id/nodes` | 是 | 自定义节点 |
| PUT / DELETE | `/api/profiles/:id/nodes/:node_id` | 是 | 单个节点 |
| GET / POST | `/api/profiles/:id/groups` | 是 | 自定义分组 |
| PUT / DELETE | `/api/profiles/:id/groups/:group_id` | 是 | 单个分组 |
| POST | `/api/profiles/:id/import-provider-groups` | 是 | 导入机场 `proxy-groups` 为可编辑自定义分组(实时拉取,跳过同名/不支持类型) |
| GET | `/api/profiles/:id/preview` | 是 | 预览生成的 YAML |
| POST | `/api/profiles/:id/generate` | 是 | 校验并生成托管链接 |
| POST | `/api/profiles/:id/reset-token` | 是 | 重置该配置 token |
| GET | `/api/settings` | 是 | 查看应用设置 |
| POST | `/api/settings/reset-public-path` | 是 | 重置公共路径前缀 |
| GET | `/:public_path_prefix/api/sub/:token` | 否 | 公开订阅下载 |

## Profile 资源

列表返回摘要,详情返回完整对象(含规则、节点、分组)。`source_url` 默认脱敏,
仅在创建/更新请求中接受完整值。

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

创建请求:

```json
{
  "name": "My Profile",
  "source_type": "clash",
  "source_url": "https://example.com/api/sub?token=abcdef",
  "enabled": true
}
```

- `source_type` ∈ `mihomo | clash | surge | loon`(MVP 仅实现 `mihomo`/`clash`)。
- `source_url` 在写入时即做静态校验:必须是 http/https、不得内嵌凭据、不得指向
  本地/私有地址(回环主机名或被封锁的字面 IP),否则返回 `400`。这是纵深防御,
  真正的 SSRF 校验仍在拉取时按 DNS 解析并钉死 IP(见 `security-design.md`)。
- 创建时立即生成 `token`;`subscription_url` 由
  `PUBLIC_BASE_URL + public_path_prefix + token` 拼装,见 `security-design.md`。
- `last_fetch_status`:最近一次机场拉取的结果分类,取值
  `success` / `http_error:<code>` / `ssrf_rejected` / `timeout` / `too_large`,
  供"原始订阅源"卡片展示;从未拉取时为 `null`。

自定义节点/分组请求体:

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
- `PUT` 使用与 `POST` 相同的请求体,整体替换。

节点预览:

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
- 规则预览同样支持拖拽排序,但规则顺序本身具有语义(自上而下命中即止),且
  `rulesets.content` 本就是有序文本,因此无需新增列:前端拖动后直接把重排后的规则
  行经 `PUT /api/profiles/:id/rules` 整体保存。该端点保存后同样会就地重写已生成缓存
  的 `rules` 块(规则与机场无关,可独立重建),因此排序(以及增删改)**立即生效**,
  无需重新拉取机场。
- 编辑自定义节点与自定义分组均通过结构化表单完成:节点给出常用字段 + 高级键值;
  分组按类型给出选项(`url`/`interval`/`tolerance`/`lazy`/`strategy`)+ 高级键值,
  成员从候选下拉中选择。前端保存时分别序列化为节点 `content` 的 Mihomo proxy YAML
  与分组 `options` 的 JSON 对象。

## 生成与校验

`POST /api/profiles/:id/generate` 执行完整校验,成功后刷新缓存并返回托管链接;
失败返回 `400` 和逐条错误,与 Web 弹窗文案一一对应。

详情页"原始订阅源"卡片的手动刷新按钮**复用本端点**,不另设 refresh 端点。

`GET /api/profiles/:id/preview` 是 generate 的只读版本:有新鲜缓存时返回缓存,
否则实时拉取生成;**不**写入缓存,不影响托管链接和 `last_*` 字段。

校验规则:

- 规则引用的分组必须存在于已启用的自定义分组中(机场分组不在输出里,除非已导入为
  自定义分组)。
- 自定义分组名称可与机场分组重名(机场分组被整体替换,「导入机场分组」正是这样生成
  同名自定义分组)。
- 自定义分组成员必须引用存在的机场节点(仍透传)、已启用的自定义节点,或已启用的
  自定义分组。
- 输出必须是合法的 Mihomo YAML。

顶层键处理:

转换器对拉取到的机场配置的每个顶层键都显式处理(实现见 `src/converter.rs`):

| 键 | 处理 |
|-----|------|
| `proxies` | 机场块(机场代理,上游序)+ 自定义块(启用的自定义节点,按 `node_order` 排),按 `node_section_order` 拼接 |
| `proxy-groups` | 整体替换为启用的自定义分组(机场分组不透传,需「导入机场分组」),再按 `group_order` 重排 |
| `rules` | 整体替换为用户规则 |
| `rule-providers` | 机场的**原样透传**(不托管自定义规则集);用户规则仍可用 `RULE-SET` 引用机场自带条目名 |
| `proxy-providers` | **MVP 阶段剥离**:远程节点提供者会让客户端拉取绕过本服务 SSRF 防护与缓存的 URL,并可能暴露机场 URL |
| 其余(`port`、`dns`、`tun`、`sniffer`…) | 原样透传 |

未知键透传而非丢弃,使新的 Mihomo 选项无需改转换器即可继续工作。

`proxy-groups` 与 `rules` 同为「替换」模型:机场原生分组不再进入输出,机场更新分组
也不会自动生效。`POST /api/profiles/:id/import-provider-groups` 实时拉取机场订阅,把
其 `proxy-groups` 解析为**可编辑的自定义分组**写入(`name`/`type`/`proxies`→成员,其余
键→`options`;跳过同名与不支持类型),返回 `{ imported, skipped }`。导入只改
`custom_groups`,与新增自定义分组一样需重新「生成」才进入输出。鉴权与 SSRF 防护同
`provider-rules`。

规则集(`rule-providers`):本项目**不托管/管理自定义规则集**,转换器只把机场自带的
`rule-providers:` 原样透传给客户端。用户规则里仍可用 `RULE-SET,<name>,<policy>` 引用
机场自带规则集的名称,这些 `RULE-SET` 规则因透传而继续解析。(早期版本的自定义规则集
CRUD 与 `rule_providers` 表已移除,见 `data-model.md`。)

成功响应:

```json
{
  "subscription_url": "https://sub.example.com/7fKp9mQx/api/sub/3w7s9xQm...",
  "generated_at": "2026-06-12T08:00:00Z"
}
```

## 公开订阅端点

```text
GET /:public_path_prefix/api/sub/:token
  -> 200 text/yaml         有效路径 + 有效 token + 配置启用
  -> 503                   请求有效,但无任何缓存且上游拉取失败(通用响应)
  -> 404 Not Found         其余一切情况(统一响应)
```

- 缓存过期且重新拉取失败时:返回过期缓存并记录告警(见 `security-design.md`)。
- 完全无缓存且拉取失败时:返回通用 `503`,响应体不含任何上游信息。

成功响应头:

```text
content-type: text/yaml; charset=utf-8
content-disposition: attachment; filename="<profile-name>.yaml"
subscription-userinfo: upload=...; download=...; total=...; expire=...
profile-update-interval: 24
```

- `subscription-userinfo` 从原始订阅响应透传,随生成缓存一起保存,使客户端能
  显示流量和到期信息;上游未提供时省略该头。
- `profile-update-interval`(小时)提示客户端自动更新周期,MVP 固定为 `24`。

行为:

- **每次拉取都实时重拉机场并重新生成**,使客户端永远拿到机场最新节点;不再有
  「缓存新鲜就直接返回」的 TTL 短路。并发拉取由 per-profile single-flight 合并为
  一次机场拉取(后到的请求若发现缓存已在本批刷新过,直接复用)。`generated_cache`
  现仅作**机场拉取失败时的兜底**(返回上一份缓存,无则 `503`);`CACHE_TTL_MINUTES`
  现只影响管理端 `preview`,不影响本端点。
- 「生成配置」按钮已移除——公共链接始终实时,无需手动生成;管理端预览里的机场节点
  通过「原始订阅源 → 刷新」(`POST /generate`)更新。
- 响应和错误中绝不包含原始机场 URL。
- 按 token 和来源 IP 限流(配合 single-flight 合并,约束机场侧负载)。

## 兼容性说明

- 早期原型的 `/api/v1/subscriptions*` 与 `/api/v1/merged` 路由已移除,
  无兼容层。
