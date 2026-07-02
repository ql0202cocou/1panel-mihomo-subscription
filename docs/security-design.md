# 安全设计

自托管于 1Panel,默认安全:不泄露机场秘密、不被当内网扫描器、链接不易枚举、管理面全程
鉴权、错误/日志不含秘密。

**信任边界(区别对待):** ① 管理员浏览器 → Web UI / 管理 API:需认证;② 公共客户端 →
订阅端点:无登录,需随机路径前缀 + per-profile token;③ 后端 → 机场 URL:每次出站获取受
SSRF 保护。

## 公共链接

```text
https://<PUBLIC_BASE_URL>/<PUBLIC_PATH_PREFIX>/api/sub/<profile_token>
```

- `PUBLIC_PATH_PREFIX` 随机 16-24 字符;`profile_token` ≥32 随机字节、每配置独立;链接不含
  库 ID 或机场 URL。
- 放行 = 前缀匹配 **且** token 存在 **且** 配置启用;否则一律 `404`(不透露哪步失败)。
- **防时序侧信道**:无论前缀是否匹配都执行 token 查找,前缀恒定时间比较;规则集托管端点同样先
  查 token 再判定前缀。
- **规则集托管** `…/<PUBLIC_PATH_PREFIX>/api/sub/<profile_token>/r/<name>/<behavior>.<format>`
  共用同一前缀与 profile token(重置任一秘密均使其失效)。规则集内容是规则清单、非私密,按名可枚举
  可接受;仍按源 IP 限流。`name` 限 `[A-Za-z0-9._-]`,杜绝路径穿越。

## Token 轮换

重置单配置 token、重置全局 `PUBLIC_PATH_PREFIX`(使所有链接失效)均支持;机场变化时链接保持
稳定,除非显式重置。

## 管理员认证

- 凭据来自 `ADMIN_USERNAME` / `ADMIN_PASSWORD`(compose 环境变量),未设置拒绝启动;
  恒定时间比较;登录失败按 IP + 账户限流。
- 会话 Cookie:≥128 位 CSPRNG ID、`HttpOnly` + `SameSite=Lax`、HTTPS 加 `Secure`;存内存
  (重启失效)、空闲超时默认 7 天。`Secure` 由 `https://` 的 `PUBLIC_BASE_URL` 推断;TLS 终止
  代理后(应用走 HTTP)需显式 `SECURE_COOKIES=true`,否则告警。
- **不启用 CORS 层**(SPA 同源;宽松 CORS 会破坏 cookie 同源保护);状态变更请求必须带同源
  `Origin`,生产环境按 `PUBLIC_BASE_URL` 的完整 origin(scheme + host + port)校验(缺失或不匹配
  均 `403`)。公共链接不需会话。

## SSRF 保护

**所有**出站获取(generate / preview / 公共端点 + provider-rules / import-provider-groups +
规则集远程镜像)走单一保护获取器。规则集远程镜像复用同一获取器的字节路径(`fetch_bytes`,为二进制
`mrs` 不强制 UTF-8),享受同样的 IP 钉定 / 重定向逐跳重查 / 超时 / 大小限制。

- 仅 `http` / `https`;拒空主机、内嵌凭据、`localhost` 回环名、阻止段裸 IP。
- 解析域名 → 检查解析 IP → **连接时固定该 IP**(防 DNS 重绑定 TOCTOU),非请求时重解析;
  每个重定向同规则重查,上限 3。
- IPv6 内嵌 IPv4(映射 `::ffff:0:0/96`、NAT64 `64:ff9b::/96`、6to4 `2002::/16`)须解包出
  IPv4 再按 IPv4 段查(经典绕过,如 `http://[::ffff:127.0.0.1]/`)。
- 出站限制:连接超时 5-10s、总超时 10-20s、最大响应 5-10MB(按流字节计,不信
  `Content-Length`)、重定向 ≤3、仅取文本/YAML。

阻止 IPv4:`0.0.0.0/8 10.0.0.0/8 100.64.0.0/10 127.0.0.0/8 169.254.0.0/16 172.16.0.0/12
192.0.0.0/24 192.0.2.0/24 192.88.99.0/24 192.168.0.0/16 198.18.0.0/15 198.51.100.0/24
203.0.113.0/24 224.0.0.0/4 240.0.0.0/4`
阻止 IPv6:`::/128 ::1/128 ::ffff:0:0/96 64:ff9b::/96 2002::/16 fc00::/7 fe80::/10 ff00::/8`

## 不受信任内容(机场响应即使过 SSRF 也不可信)

- 解析 YAML 用资源限制:**先**扫原文限锚点/别名数(防「十亿笑」),**再**限嵌套深度/节点数;
  管理员提交的节点/分组 YAML 同等限制;请求体 ≤1MB(超限 `413`)。
- `subscription-userinfo` 存/回显前校验格式(仅 `key=value; ...`,拒 CR/LF,防头注入)。
- 机场节点/分组名视为纯数据,渲染时转义,绝不拼入 HTML / shell。

## 敏感数据(机场 URL 含秘密)

不写完整 URL 进日志 / 公共输出 / 错误;管理 API 默认脱敏;Web UI 仅持脱敏值。脱敏规则
(确定性,处处一致):留 scheme/host/path,每个查询值 → `***`(`?token=abcdef` → `?token=***`)。
HTTP trace 只记录脱敏 path,公开订阅/规则集路径中的 `PUBLIC_PATH_PREFIX` 与 profile token 均替换为
占位值。

## 限流与客户端 IP

- 登录按 IP + 账户;公共下载按**源 IP**(独立于 token,`404` 也计数)限流,使枚举共享单一
  预算;首版内存限流。
- 默认不信任 `X-Forwarded-For`: `TRUSTED_PROXY_HOPS=0`,按 TCP 对端限流。需要按真实客户端 IP
  限流时,必须同时设置受信跳数与 `TRUSTED_PROXY_CIDRS`(逗号分隔的直接反代网段);只有 TCP 对端
  落在该网段内时才读取 `X-Forwarded-For`,并取**最右**不受信跳(最左可伪造)。头缺失、过短、
  或 peer 不可信时回退 TCP 对端。

## 缓存与刷新

- 公共端点以 `PUBLIC_REFRESH_MIN_SECONDS`(默认 30 秒)作为每配置最小回源间隔:间隔内复用最近
  `generated_cache`,间隔外回源拉取并重新生成;拉取失败时用旧缓存兜底(无则 `503`)。
  `CACHE_TTL_MINUTES`(默认 15)仅管理端 `preview`。
- **single-flight**:同配置并发刷新在 per-profile 锁后合并为一次上游获取(后到者等待或拿陈旧
  缓存),防踩踏扇出。

## 错误处理

- 公共端点:无效路径 / token → 通用 `404`;有效但无缓存且拉取失败 → 通用 `503`
  (体内无上游细节);不透露 token 是否存在。
- 管理 API:返回有用校验错误但不含机场秘密;内部细节脱敏后入日志。
