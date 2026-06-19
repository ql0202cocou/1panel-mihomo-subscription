# 1Panel 应用打包

> **状态：包已完成（0.2.1）。** `0.2.1` 应用包暴露了完整的安装表单——管理员凭据、
> 公共源/路径前缀、获取/缓存/代理调优和 `SECURE_COOKIES` 覆盖——compose 文件将它们
> 全部传递给服务。下面的环境变量表是权威参考；安装表单、compose 和代码保持一致。

1Panel 应用包位于：

```text
apps/mihomo-subscription
```

旨在遵循官方 1Panel 应用包布局，同时保持为个人/本地应用包。

## 结构

```text
apps/mihomo-subscription/
  data.yml
  README.md
  logo.png
  0.1.0/            # 历史版本（不完整；早于完整安装表单）
    data.yml
    docker-compose.yml
    data/
  ...               # 其余历史版本目录,保留不删
  0.2.1/            # 当前版本
    data.yml
    docker-compose.yml
    data/
```

## 本地安装路径

将应用包目录复制到 1Panel 主机：

```bash
/opt/1panel/resource/apps/local/mihomo-subscription
```

然后打开 1Panel 应用商店并刷新应用列表。

## 环境变量

此表是权威列表。1Panel 安装表单（`apps/mihomo-subscription/<version>/data.yml`）、
compose 文件和服务代码必须保持一致。"已打包"标记当前包已暴露的内容。

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

`PUBLIC_BASE_URL` 应仅存储外部可达的源。随机的 `PUBLIC_PATH_PREFIX` 在生成永久链接时
前置在 token 化的订阅端点之前：

```text
https://sub.example.com/<public-path-prefix>/api/sub/<token>
```

## 验证检查清单

此检查清单描述发布所需的包状态。`0.2.1` 包满足以下每一项。

- 根 `data.yml` 包含应用元数据。
- 版本 `data.yml` 包含 `additionalProperties.formFields`。
- 版本 `data.yml` 暴露管理登录的 `ADMIN_USERNAME` 和 `ADMIN_PASSWORD` 安装字段。
- 版本 `data.yml` 暴露上表中的其余安装参数（`PUBLIC_BASE_URL`、`PUBLIC_PATH_PREFIX`、`FETCH_TIMEOUT_SECONDS`、`MAX_SUBSCRIPTION_SIZE_MB`、`CACHE_TTL_MINUTES`、`TRUSTED_PROXY_HOPS`、`SECURE_COOKIES`）。
- `docker-compose.yml` 使用 `${CONTAINER_NAME}`。
- `docker-compose.yml` 将每个安装表单变量作为环境变量传递（管理员凭据、公共源/前缀、获取/缓存/代理调优和 `SECURE_COOKIES`）。
- Web 端口表单字段使用 `PANEL_APP_PORT_HTTP`。
- 服务连接到外部 `1panel-network`。
- 持久数据从 `./data` 挂载。
- 镜像引用匹配发布的 Docker Hub 镜像（`quinlanhoo/mihomo-subscription:<version>`，多架构 amd64+arm64，安装时由 1Panel 主机拉取；见 `docs/release.md` 的离线本地构建回退）。
- `logo.png` 存在（当前是生成的占位符；在公共分发前替换为真实设计）。

## 登录配置

管理 Web UI 必须在用户查看或更改订阅配置之前要求登录。

通过 1Panel 应用安装表单配置凭据，并通过 compose 环境变量传递给服务：

```yaml
environment:
  - ADMIN_USERNAME=${ADMIN_USERNAME}
  - ADMIN_PASSWORD=${ADMIN_PASSWORD}
```

这些凭据仅保护管理 UI 和管理 API。生成的订阅链接保持公共，但仍需随机公共路径前缀和每个配置文件的 token。

## 反向代理主机头

管理 API 在状态更改请求上验证 `Origin` 头与请求 `Host`（CSRF 纵深防御）。
因此，应用前面的反向代理必须保留原始 Host——对于 nginx/OpenResty：

```nginx
proxy_set_header Host $host;
```

如果代理将 `Host` 重写为后端地址，浏览器 `Origin` 和 `Host` 将不一致，
每个登录/POST/PUT/DELETE 都返回 `403`。