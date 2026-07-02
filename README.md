# Mihomo Subscription Manager

面向 1Panel 自托管的轻量级 Mihomo 订阅转换/分发服务(类 Sub-Store Lite):录入机场
订阅,配置自定义规则、节点、代理分组,生成长期有效的 Mihomo 订阅链接。带 Web 管理
页面、管理员登录、SSRF 防护与订阅 URL 脱敏。

A lightweight self-hosted Mihomo subscription converter/distributor for 1Panel
(a "Sub-Store Lite"): register provider subscriptions, configure custom rules,
nodes, and proxy groups, and generate long-lived Mihomo links. Ships a web admin
UI with login, SSRF protection, and provider-URL masking.

> 状态:已实现并经安全审计加固。镜像发布在 Docker Hub
> (`quinlanhoo/mihomo-subscription:latest`,多架构 amd64+arm64),用 docker compose 部署。
> Status: implemented and security-hardened. The image is published on Docker Hub
> (`quinlanhoo/mihomo-subscription:latest`, multi-arch amd64+arm64); deploy with
> docker compose.

## 在 1Panel 中部署 / Deploy in 1Panel

镜像发布在 **Docker Hub**(`quinlanhoo/mihomo-subscription`,多架构 amd64+arm64),
1Panel 主机直接拉取,无需本地构建。离线/内网请参见 `docs/deploy.md` 的本地构建备选。
The image is published on **Docker Hub** (`quinlanhoo/mihomo-subscription`,
multi-arch amd64+arm64); the 1Panel host pulls it directly, no local build
needed. For offline/intranet, see the local-build fallback in `docs/deploy.md`.

最简单的方式是手写 Compose:在 1Panel「容器 → 编排」新建,把下面整段粘贴进去,
**只改 4 个标注 `← 修改` 的值**,创建即部署。
The simplest path is a hand-written Compose: in 1Panel **Containers → Compose**,
paste the whole block below, change only the four values marked `← edit`, and
create:

**1. 部署容器 / Deploy the container**

```yaml
services:
  mihomo-subscription:
    image: quinlanhoo/mihomo-subscription:latest
    container_name: mihomo-subscription
    restart: unless-stopped
    networks: [1panel-network]
    ports:
      - "8080:8080"                                  # 宿主:容器 / host:container
    volumes:
      - ./data:/data                                 # 持久化 SQLite / persistent SQLite
    environment:
      # ── 必填 / required ───────────────────────────────────────────────
      - ADMIN_USERNAME=admin                         # ← 修改 / edit: 管理员账号
      - ADMIN_PASSWORD=change-me-to-a-strong-secret  # ← 修改 / edit: 用强密码
      - PUBLIC_BASE_URL=https://sub.example.com      # ← 修改 / edit: 对外访问地址(含 https)
      - SECURE_COOKIES=true                          # ← 修改 / edit: HTTPS 反代填 true,纯 HTTP 填 false
      # ── 固定,勿改 / fixed, do not change ────────────────────────────
      - PORT=8080
      - DATA_DIR=/data
      # ── 可选,留默认即可 / optional, defaults are fine ───────────────
      - RUST_LOG=info                                # 日志级别 / log level
      - PUBLIC_PATH_PREFIX=                          # 留空自动生成随机前缀 / blank = random
      - FETCH_TIMEOUT_SECONDS=15                     # 机场拉取超时 / provider fetch timeout
      - MAX_SUBSCRIPTION_SIZE_MB=8                   # 机场响应大小上限 / provider size cap
      - CACHE_TTL_MINUTES=15                         # 生成结果缓存 / generated cache TTL
      - PUBLIC_REFRESH_MIN_SECONDS=30                # 公开订阅回源刷新下限 / public refresh floor
      - TRUSTED_PROXY_HOPS=0                          # 默认不信任 XFF / do not trust XFF by default
      - TRUSTED_PROXY_CIDRS=                          # 可信反代 CIDR,留空忽略 XFF / trusted proxy CIDRs
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:8080/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
networks:
  1panel-network:
    external: true
```

缺少 `ADMIN_USERNAME`/`ADMIN_PASSWORD` 时服务会拒绝启动;每个变量的含义见
[docs/deploy.md](docs/deploy.md) 的环境变量表。 The service refuses to
start without `ADMIN_USERNAME`/`ADMIN_PASSWORD`; see the env-var table in
[docs/deploy.md](docs/deploy.md) for every variable.

**2. 反向代理 / Reverse proxy** — `PUBLIC_BASE_URL` 必须等于浏览器访问的外部 origin,
并在 1Panel「网站 → 反向代理」指向该容器时**保留原始 Host 头**,否则管理 API 的 `Origin`
同源校验会让登录/写操作返回 `403`。
In 1Panel **Websites → Reverse Proxy**, point a site at the container and
**preserve the original Host header**, and make `PUBLIC_BASE_URL` match the
browser-visible origin, or the management API's `Origin`
check rejects every login/write with `403`:

```nginx
proxy_set_header Host $host;
```

默认配置会忽略 `X-Forwarded-For`,按 TCP 对端做登录/下载限流。若后端端口不会被公网直连,且需要按
真实客户端 IP 限流,同时设置 `TRUSTED_PROXY_HOPS=1` 与反向代理所在的 `TRUSTED_PROXY_CIDRS`。

部署后管理界面在站点根路径。 After deploy, the admin UI is at the site root.

## 使用 / Usage

1. 用 compose 里的管理员账户登录。 Log in with the admin account from compose.
2. 新建订阅配置，填写机场订阅 URL。
   Create a profile and enter the provider subscription URL.
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

参考文档见 [docs/](docs/)（[architecture](docs/architecture.md)、
[deploy](docs/deploy.md)、[changelog](docs/changelog.md)）。CI 门禁见
[.github/workflows/ci.yml](.github/workflows/ci.yml)，发布与变更规则见
[docs/deploy.md](docs/deploy.md)。

Reference docs are in [docs/](docs/); CI gates are in
[.github/workflows/ci.yml](.github/workflows/ci.yml), and release/change rules
are in [docs/deploy.md](docs/deploy.md).

前端本地构建需 Node `^20.19.0 || >=22.12.0`;CI 与 Docker 镜像构建使用 Node 22。
Local frontend builds require Node `^20.19.0 || >=22.12.0`;CI and Docker image
builds use Node 22.

## 许可证 / License

[MIT](LICENSE)
