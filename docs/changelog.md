# 变更日志

此项目的所有显著变更都应记录在此文件中。

使用倒序。保持条目简洁，尽可能面向用户，并按变更类型分组。

## 维护规则

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

### 文档

- 合并文档：`api-design.md` + `data-model.md` + `security-design.md` → `architecture.md`；`1panel-app.md` + `release.md` → `deploy.md`。保留 `changelog.md`，删除 `docs/README.md`。
- 精简 `architecture.md`：压缩表外重复说明、合并 API 伪代码块、精简安全设计冗长描述（604→550 行）。
- 精简 `changelog.md`：合并 0.3.0-a1/a2/a3 进 0.3.0，删除 0.2.5 及以下历史版本（762→144 行）。

## [0.5.1] - 2026-07-02

### 变更

- 公共订阅端点新增回源刷新下限 `PUBLIC_REFRESH_MIN_SECONDS`(默认 30 秒):同一订阅在下限内复用最近
  生成缓存,下限外仍回源刷新并保留 single-flight 合并与失败兜底,降低公开 token 泄露后的上游拉取放大。

### 安全

- 客户端 IP 派生默认不再信任 `X-Forwarded-For`: `TRUSTED_PROXY_HOPS` 默认改为 `0`,并新增
  `TRUSTED_PROXY_CIDRS`;只有 TCP 对端落在显式可信反代网段内时才读取 XFF,避免后端端口被直连时
  伪造 XFF 绕过登录/下载限流。
- 管理端状态变更请求现在必须带同源 `Origin`,并按 `PUBLIC_BASE_URL` 的完整 origin(scheme +
  host + port)校验;缺失 `Origin` 也返回 `403`,减少 CSRF 防护对浏览器 `SameSite` 行为的依赖。
- 移除项目对 `anyhow` 的直接依赖,并将 lockfile 中的 `anyhow` 升至 `1.0.103`,清除
  `RUSTSEC-2026-0190` 审计告警。
- 公开规则集托管端点对齐订阅端点的前缀处理:前缀使用恒定时间比较,且无论前缀是否匹配都先执行
  token 查找,避免规则集路径泄露额外的公共前缀时序信号。
- HTTP trace span 不再记录完整 URI,改为记录脱敏 path;公开订阅/规则集路径中的
  `PUBLIC_PATH_PREFIX` 与 per-profile token 均替换为占位值,避免调试日志泄露长期有效链接秘密。
- 1Panel compose 示例显式设置 `SECURE_COOKIES=true`,减少 HTTPS 反代部署误发非 Secure 会话 cookie
  的配置风险。
- 升级前端开发依赖 Vite / React 插件,消除 `npm audit` 报告的 Vite/esbuild 开发服务器漏洞;CI 与
  Docker 前端构建切到 Node 22,并声明本地构建需 Node `^20.19.0 || >=22.12.0`。
- 修复 Vite dev proxy 与新 Origin 策略不一致的问题:本地代理保留浏览器的 `Host`/`Origin`,避免
  `npm run dev` 登录和写操作因缺失 `Origin` 返回 `403`。

## [0.5.0] - 2026-06-28

### 变更

- 「分组预览」名称 + 成员列加宽,长分组名(含 emoji / 中英混排)更易在一行完整显示。

### 移除

- 弃用 1Panel 应用包,改为用 docker compose 在 1Panel 上部署。删除整个 `apps/mihomo-subscription/`
  应用包目录及 CI 的「1Panel package YAML」校验 job;`docs/release.md` 去掉应用包步骤;
  `docs/1panel-app.md` 重写为「compose 部署 + 环境变量」(保留权威 env 表与反代 `Host` 透传要求);
  README / docs/README 相应更新。仅保留 1Panel 兼容性(compose、`1panel-network`、`./data`、保留
  `Host` 的反代),不再维护应用商店格式。

## [0.4.0] - 2026-06-28

### 新增

- **订阅自包含的规则库(③ 托管规则库)**:每个订阅现在持有自己的规则集定义,随订阅自包含。在订阅
  「规则」里编辑 RULE-SET 规则时可直接内联定义其 rule-provider(规则集名 / behavior / format /
  手动 payload 或远程 URL+更新间隔),保存进该订阅。新增 per-profile API
  `GET/POST /api/profiles/:id/rule-sets`、`PUT/DELETE /api/profiles/:id/rule-sets/:rsid`、
  `POST /api/profiles/:id/rule-sets/import`(从全局库复制),以及按订阅 token 隔离的公开托管端点
  `GET /:prefix/api/sub/:token/r/:name/:behavior.:format`(无鉴权、统一 404、IP 限流;manual 渲染、
  remote 懒刷新二次托管、支持 mrs)。新增迁移 `0011_profile_rule_sets.sql`。

### 变更

- **规则集托管彻底解耦为「三规则库」模型**:① 机场原始规则、② 全局用户规则库、③ 每订阅托管规则库。
  下发只读 ③;①②仅作导入源。生成时 `RULE-SET` 引用按名注入 ③ 自己的定义(URL 指向按订阅 token
  隔离的托管链接,remote+cache 关闭则直注上游),不再读取全局库。
- **全局「规则托管」页(②)降级为用户规则库 / 导入源**:不再公开托管、不再参与生成。移除全局托管
  路由 `/:prefix/r/:name/:file` 与 ② 的远程镜像/缓存代码路径;`/api/rule-sets` 响应去掉托管链接
  `url` 字段(保留 `remote_url_masked` 模板来源)。页面去掉托管链接展示与「复制链接」,文案改为
  「用户规则库」。「导入托管规则」改为把所选 ② 条目**复制**进当前订阅 ③ 并追加 `RULE-SET` 规则行。
- 规则编辑弹窗:RULE-SET 类型改为内联 rule-provider 定义表单(手动 / 远程双来源),替代原先的纯名称
  输入;并对齐设计稿细节——规则类型旁显示所属分类徽标、匹配内容下显示示例、切换类型时按示例预填
  匹配内容、NETWORK 用分段切换、MATCH 用说明框、策略下拉按「节点 / 分组 / 内置」分组。
- 规则卡恢复「添加规则」按钮(0.3.0 曾移除手动单条新增):打开规则编辑器以新增一条规则,追加到列表
  末尾(MATCH 仍钉底);与「导入机场规则」「导入托管规则」并列。

## [0.3.0] - 2026-06-27

### 新增

- **全局规则集托管（#0008）**：跨订阅复用的全局规则集库，面板托管为永久链接 `/<prefix>/r/<name>/<behavior>.<format>`，订阅以 `RULE-SET,<name>` 引用即可套用——这是用「面板自托管」模型重新引入 0.2.3 移除的规则集功能。两种来源：手动（管理员录入 payload）与远程镜像（按间隔懒拉取并以稳定链接二次托管，支持 yaml/text/mrs）。后端新增 `rule_sets` 表 + `src/rule_sets.rs` + 公开托管端点（无鉴权、IP 限流、统一 404）；转换器在 `RULE-SET` 引用时注入对应 `rule-providers:`（同名覆盖机场条目）。前端新增「规则托管」页面（卡片列表 + 拖拽排序 + 新建/编辑弹窗）+「导入托管规则」弹窗。远程镜像复用 SSRF 安全拉取器（`fetch_bytes` 字节路径）+ single-flight 懒刷新 + 缓存兜底。

### 变更

- **自定义节点改为全局共享池（#0007）**：自定义节点不再隶属单个配置，而是一份全局集合自动追加到每条配置输出。后端新增 `global_nodes` 表 + 迁移合并原 `custom_nodes` 去重 + `DROP TABLE custom_nodes`；新增 `GET/POST /api/global-nodes`、`PUT/DELETE /api/global-nodes/:id`、`PUT /api/global-nodes/order`（全局排序立即重排所有缓存）；移除 per-profile 节点端点。`profiles.node_order` 弃用，节点顺序由 `global_nodes.position` 决定。
- **Web 后台全面重构**：新增左侧栏 App Shell + 陶土色主题（亮/暗手动切换、持久化）+「节点配置」全局页 +「规则托管」页；登录页、订阅列表、配置详情（hero + 标签页）、系统设置按设计稿重排。节点 tab 改为只读两块；规则 MATCH 钉底。节点/分组录入弹窗收尾（chip 行类型选择 + 嵌套选项子区块 + 窄屏降级）。修复 vite dev 代理 `Origin` 头问题。
- 移除「原始订阅类型」概念，删 `profiles.source_type` 列（#0009）。
- 新建订阅后自动拉取一次，移除「未拉取」中间态。
- 移除订阅「启用/禁用」概念，删 `profiles.enabled` 列（#0010）。
- 规则托管撞名检测：生成响应新增 `ruleset_conflicts`，详情页告警 banner 提示。
- 详情页规则卡移除「添加规则」按钮，规则仅从导入入口来。
- 统一各导航页容器尺寸（`max-width 1180`）。
- 登录页副标题与标语微调（a2）。

### 文档

- `api-design.md` / `data-model.md` 同步全局节点模型；精简全部技术文档（约 1123→600 行）；全库源码注释统一中文化。


