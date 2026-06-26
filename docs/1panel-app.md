# 1Panel 应用打包

> 应用包在 `apps/mihomo-subscription`,遵循官方 1Panel 布局。安装表单暴露完整参数(管理员
> 凭据、公共源/前缀、获取/缓存/代理调优、`SECURE_COOKIES`),compose 全部传给服务。**下方
> env 表是权威参考**——安装表单、compose、代码三者必须一致。

## 结构

```text
apps/mihomo-subscription/
  data.yml  README.md  logo.png            # logo 当前为占位符,公开分发前替换
  0.1.0/ ...                               # 历史版本目录,保留不删
  <当前版本>/ { data.yml, docker-compose.yml, data/ }
```

每版本新增一个版本目录(保留旧的)。本地安装:复制目录到 1Panel 主机
`/opt/1panel/resource/apps/local/mihomo-subscription`,再在应用商店刷新列表。

## 环境变量(权威表)

安装表单(`<version>/data.yml` 的 `formFields`)、compose、代码必须一致。「已打包」= 当前包已暴露。

| 变量 | 来源 | 默认值 | 已打包 | 用途 |
|------|------|--------|--------|------|
| `PANEL_APP_PORT_HTTP` | 安装表单 | `8080` | 是 | 主机 Web 端口映射 |
| `RUST_LOG` | 安装表单 | `info` | 是 | 日志级别 |
| `ADMIN_USERNAME` | 安装表单 | —（必需） | 是 | 管理登录账户 |
| `ADMIN_PASSWORD` | 安装表单 | —（必需） | 是 | 管理登录密码 |
| `PUBLIC_BASE_URL` | 安装表单 | —（必需） | 是 | 生成链接的外部可达源 |
| `PUBLIC_PATH_PREFIX` | 安装表单（可选） | 随机 | 是 | 公共路径前缀的种子；运行时值存储在 `app_settings` 中并可重置（见 `data-model.md`）。空/空白被忽略并生成随机前缀 |
| `FETCH_TIMEOUT_SECONDS` | 安装表单 | `15` | 是 | 机场获取总超时 |
| `FETCH_USER_AGENT` | 环境变量（可选） | `clash.meta/1.0` | 否 | 机场获取的 `User-Agent`。许多机场面板基于 Clash 家族 UA 限制订阅，对未知客户端返回 `403`/`401`；默认匹配常见的 `/clash/i` 检查并表示 Meta 支持。仅覆盖需要特定客户端 UA 的面板（如 Shadowrocket/Stash） |
| `MAX_SUBSCRIPTION_SIZE_MB` | 安装表单 | `8` | 是 | 机场响应大小限制 |
| `CACHE_TTL_MINUTES` | 安装表单 | `15` | 是 | 仅管理员**预览**缓存 TTL；公共订阅端点始终按拉取重新获取机场（见 `api-design.md` / `security-design.md`） |
| `TRUSTED_PROXY_HOPS` | 安装表单 | `1` | 是 | 派生客户端 IP 时信任的反向代理跳数（见 `security-design.md`） |
| `SECURE_COOKIES` | 安装表单（可选） | `auto`（从 `https://` 的 `PUBLIC_BASE_URL` 推断） | 是 | 强制 `Secure` 会话 cookie 属性。安装表单提供 `auto`/`true`/`false`；`auto`（和任何无法识别的值）回退到推断。当通过 TLS 终止反向代理（应用本身使用纯 HTTP）通过 HTTPS 提供服务时设置 `true`；当 cookie 最终没有 `Secure` 时服务记录警告 |
| `PORT` | Compose（固定） | `8080` | 是 | 容器监听端口 |
| `DATA_DIR` | Compose（固定） | `/data` | 是 | SQLite 数据目录 |
| `WEB_DIR` | Dockerfile（内部） | `/app/web/dist` | 不适用 | Axum 提供的构建 SPA 资产目录。烘焙到镜像中；不是用户/安装字段——为完整性列出 |

`PUBLIC_BASE_URL` 仅存外部可达源;随机 `PUBLIC_PATH_PREFIX` 前置在 token 化端点前:
`https://sub.example.com/<public-path-prefix>/api/sub/<token>`。

## 打包要求

- 根 `data.yml` 含应用元数据;版本 `data.yml` 含 `additionalProperties.formFields`,暴露
  `ADMIN_USERNAME`/`ADMIN_PASSWORD` 及上表其余安装参数。
- `docker-compose.yml`:用 `${CONTAINER_NAME}`;Web 端口表单字段用 `PANEL_APP_PORT_HTTP`;每个
  表单变量作 env 传入;连外部 `1panel-network`;数据从 `./data` 挂载;镜像引用
  `quinlanhoo/mihomo-subscription:<version>`(多架构 amd64+arm64,安装时主机拉取;离线本地构建
  见 `release.md`)。

## 登录与反向代理

- 凭据经表单写入 compose env(`ADMIN_USERNAME`/`ADMIN_PASSWORD`),仅保护管理 UI/API;公共
  链接仍需随机路径前缀 + per-profile token。
- 管理 API 在状态变更请求校验 `Origin` 与 `Host`(CSRF 纵深防御),故代理须保留原始 Host
  (nginx/OpenResty:`proxy_set_header Host $host;`)。若代理把 `Host` 改写为后端地址,
  `Origin`/`Host` 不一致,每个登录/POST/PUT/DELETE 返回 `403`。
