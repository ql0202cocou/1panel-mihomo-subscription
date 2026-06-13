# Mihomo Subscription Manager

面向 1Panel 自托管的轻量级 Mihomo 订阅转换/分发服务(类 Sub-Store Lite):录入机场
订阅,配置自定义规则、节点、代理分组,生成长期有效的 Mihomo 订阅链接。带 Web 管理
页面、管理员登录、SSRF 防护与订阅 URL 脱敏。

A lightweight self-hosted Mihomo subscription converter/distributor for 1Panel
(a "Sub-Store Lite"): register provider subscriptions, configure custom rules,
nodes, and proxy groups, and generate long-lived Mihomo links. Ships a web admin
UI with login, SSRF protection, and provider-URL masking.

> 状态:已实现并经安全审计加固,尚未正式发布(1Panel 应用包安装表单待补齐)。
> Status: implemented and security-hardened; not yet formally released (the
> 1Panel app package install form is still pending).

## 在 1Panel 中部署 / Deploy in 1Panel

镜像采用**本地构建**(不使用远程仓库):先在 1Panel 主机构建镜像,再用 1Panel 部署。
The image is **built locally** (no remote registry): build it on the 1Panel host,
then deploy through 1Panel.

**1. 构建镜像 / Build the image** — 在 1Panel 主机上 / on the 1Panel host:

```bash
git clone https://github.com/ql0202cocou/1panel-mihomo-subscription.git
cd 1panel-mihomo-subscription
docker build -t mihomo-subscription:0.1.0 .
```

**2. 部署容器 / Deploy the container** — 用 1Panel「容器 → 编排」部署下面的 compose
(以注入必需的环境变量)。 Use 1Panel **Containers → Compose** with the compose
below so the required environment variables are injected:

```yaml
services:
  mihomo-subscription:
    image: mihomo-subscription:0.1.0
    container_name: mihomo-subscription
    restart: always
    networks: [1panel-network]
    ports:
      - "8080:8080"                              # 宿主:容器 / host:container
    volumes:
      - ./data:/data                             # 持久化 SQLite / persistent SQLite
    environment:
      - PORT=8080
      - DATA_DIR=/data
      - RUST_LOG=info
      - ADMIN_USERNAME=admin                     # 必填 / required
      - ADMIN_PASSWORD=change-me                 # 必填,用强密码 / required, use a strong value
      - PUBLIC_BASE_URL=https://sub.example.com  # 对外访问地址 / externally reachable origin
      - SECURE_COOKIES=true                      # HTTPS 反代时置 true / true behind HTTPS proxy
      # 可选 / optional: PUBLIC_PATH_PREFIX, FETCH_TIMEOUT_SECONDS,
      # MAX_SUBSCRIPTION_SIZE_MB, CACHE_TTL_MINUTES, TRUSTED_PROXY_HOPS
networks:
  1panel-network:
    external: true
```

缺少 `ADMIN_USERNAME`/`ADMIN_PASSWORD` 时服务会拒绝启动;完整环境变量表见
[docs/1panel-app.md](docs/1panel-app.md)。 The service refuses to start without
`ADMIN_USERNAME`/`ADMIN_PASSWORD`; the full env-var table is in
[docs/1panel-app.md](docs/1panel-app.md).

**3. 反向代理 / Reverse proxy** — 在 1Panel「网站 → 反向代理」指向该容器,并**保留
原始 Host 头**,否则管理 API 的 `Origin`/`Host` 同源校验会让登录/写操作返回 `403`。
In 1Panel **Websites → Reverse Proxy**, point a site at the container and
**preserve the original Host header**, or the management API's `Origin`/`Host`
check rejects every login/write with `403`:

```nginx
proxy_set_header Host $host;
```

部署后管理界面在站点根路径。 After deploy, the admin UI is at the site root.

## 使用 / Usage

1. 用 compose 里的管理员账户登录。 Log in with the admin account from compose.
2. 新建订阅配置:选择来源类型(`mihomo`/`clash`)、填写机场订阅 URL。
   Create a profile: pick the source type (`mihomo`/`clash`) and enter the
   provider subscription URL.
3. (可选)编辑分流规则、追加自定义节点和代理分组。
   Optionally edit rules and append custom nodes/proxy groups.
4. 点击生成,得到永久订阅链接,复制到 Mihomo 客户端。 Generate to get the permanent
   link and copy it into a Mihomo client:

```text
https://<host>/<public-path-prefix>/api/sub/<profile-token>
```

链接长期有效;可重置单个配置的 token,或在系统设置重置全局路径前缀以一次失效所有链接。
Links are long-lived; reset a profile's token, or reset the global path prefix in
settings to invalidate all links at once.

## 文档与开发 / Docs & development

参考文档见 [docs/](docs/)([api-design](docs/api-design.md)、
[data-model](docs/data-model.md)、[security-design](docs/security-design.md)、
[1panel-app](docs/1panel-app.md)、[release](docs/release.md)、
[changelog](docs/changelog.md))。本地开发与 CI 门禁见
[CLAUDE.md](CLAUDE.md) / [AGENTS.md](AGENTS.md)。

Reference docs are in [docs/](docs/); local development and CI gates are in
[CLAUDE.md](CLAUDE.md) / [AGENTS.md](AGENTS.md).

## 许可证 / License

[MIT](LICENSE)
