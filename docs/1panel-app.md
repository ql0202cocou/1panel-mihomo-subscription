# 1Panel 部署(Docker Compose)与环境变量

> 部署方式:**用 docker compose 在 1Panel 上部署**(不再提供 1Panel 应用包)。镜像取 Docker Hub
> `quinlanhoo/mihomo-subscription:<version>` / `:latest`(多架构 amd64+arm64)。**下方 env 表是权威
> 参考**——compose 与代码必须一致。

## Compose 示例

在 1Panel「容器 → 编排」新建,或主机上 `docker compose up -d`:

```yaml
services:
  mihomo-subscription:
    image: quinlanhoo/mihomo-subscription:latest
    container_name: mihomo-subscription
    restart: unless-stopped
    ports:
      - "8080:8080"            # 主机端口:容器端口(容器固定 8080)
    environment:
      ADMIN_USERNAME: admin
      ADMIN_PASSWORD: change-me
      PUBLIC_BASE_URL: https://sub.example.com
      # 其余可选项见下表(日志、获取/缓存调优、SECURE_COOKIES 等)
    volumes:
      - ./data:/data           # 持久化 SQLite;容器以 root 启动后 chown 再降权
    networks:
      - 1panel-network
networks:
  1panel-network:
    external: true             # 接入 1Panel 的共享网络,供 OpenResty 反代
```

## 环境变量(权威表)

compose 的 `environment:` 与代码必须一致。

| 变量 | 默认值 | 必需 | 用途 |
|------|--------|------|------|
| `ADMIN_USERNAME` | —— | 是 | 管理登录账户 |
| `ADMIN_PASSWORD` | —— | 是 | 管理登录密码 |
| `PUBLIC_BASE_URL` | 空 | 建议 | 生成托管链接的外部可达源(`https://sub.example.com`)。为空则链接缺少 scheme/host |
| `PUBLIC_PATH_PREFIX` | 随机 | 否 | 公共路径前缀的种子;运行时值存于 `app_settings` 并可重置(见 `data-model.md`)。空/空白被忽略并随机生成 |
| `RUST_LOG` | `info` | 否 | 日志级别 |
| `FETCH_TIMEOUT_SECONDS` | `15` | 否 | 机场获取总超时 |
| `FETCH_USER_AGENT` | `clash.meta/1.0` | 否 | 机场获取的 `User-Agent`。许多机场按 Clash 家族 UA 限制订阅、对未知客户端返回 `403`/`401`;默认匹配常见 `/clash/i` 检查。仅在面板需要特定客户端 UA 时覆盖(如 Shadowrocket/Stash) |
| `MAX_SUBSCRIPTION_SIZE_MB` | `8` | 否 | 机场响应大小上限 |
| `CACHE_TTL_MINUTES` | `15` | 否 | 仅管理员**预览**缓存 TTL;公共订阅端点始终按拉取重取机场(见 `api-design.md` / `security-design.md`) |
| `TRUSTED_PROXY_HOPS` | `1` | 否 | 派生客户端 IP 时信任的反向代理跳数(见 `security-design.md`) |
| `SECURE_COOKIES` | `auto` | 否 | 强制 `Secure` 会话 cookie。`auto`(及无法识别值)从 `https://` 的 `PUBLIC_BASE_URL` 推断;经 TLS 终止反代(应用走纯 HTTP)对外 HTTPS 时设 `true`;cookie 最终无 `Secure` 时记 warn |
| `PORT` | `8080` | 否 | 容器监听端口(一般不改,改主机映射即可) |
| `DATA_DIR` | `/data` | 否 | SQLite 数据目录(对应挂载卷) |
| `WEB_DIR` | `/app/web/dist` | —— | Axum 提供的构建 SPA 目录;烘焙进镜像,非用户字段,仅列出 |

`PUBLIC_BASE_URL` 仅存外部可达源;随机 `PUBLIC_PATH_PREFIX` 前置在 token 化端点前:
`https://sub.example.com/<public-path-prefix>/api/sub/<token>`。

## 反向代理(1Panel 兼容性要点)

- 管理 API 在状态变更请求校验 `Origin` 与 `Host`(CSRF 纵深防御),故反代**必须保留原始 Host**
  (nginx/OpenResty:`proxy_set_header Host $host;`)。若代理把 `Host` 改写为后端地址,`Origin`/`Host`
  不一致,每个登录/POST/PUT/DELETE 返回 `403`。1Panel 的网站反代默认即保留 Host。
- 凭据经 `ADMIN_USERNAME`/`ADMIN_PASSWORD` 仅保护管理 UI/API;公共订阅链接仍靠随机路径前缀 +
  per-profile token。
- 容器以 root 启动以便 `docker-entrypoint.sh` chown `${DATA_DIR}`(`./data` 挂载会覆盖构建期 chown,
  否则 SQLite `code 14 (CANTOPEN)`),随后用 `gosu` 降权到 `appuser` 再 exec。
