# 发布流程

> **状态:0.2.1 应用包已就绪。** 服务、多阶段镜像构建与 1Panel 应用包安装表单
> 均已完成(`apps/mihomo-subscription/0.2.1/`)。镜像策略:**发布到 Docker Hub**
> (`quinlanhoo/mihomo-subscription`,多架构 amd64+arm64),1Panel 主机直接
> `docker pull`,无需在主机上同步源码或本地构建;离线/内网环境可改用文末的
> 本地构建备选流程。

相关文档:`changelog.md`(发布时滚动)、`1panel-app.md`(应用包结构)。

## 版本规则

- 遵循语义化版本:`MAJOR.MINOR.PATCH`。
- `0.x` 阶段允许破坏性变更,但每项必须记入 changelog 的 `Changed`。
- 镜像 tag、`Cargo.toml` 的 `version`、`web/package.json` 的 `version` 与
  1Panel 应用包版本目录保持一致。

## 发布前检查

```bash
cargo fmt --check
cargo check
cargo test

# 校验 1Panel YAML
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "OK #{f}" }' \
  apps/mihomo-subscription/data.yml \
  apps/mihomo-subscription/0.2.1/data.yml \
  apps/mihomo-subscription/0.2.1/docker-compose.yml
```

人工确认:

- `docs/changelog.md` 的 `[Unreleased]` 包含本次发布的全部变更。
- 受影响的产品/技术/安全文档已与实现对齐(见 `docs/changelog.md` 维护规则)。
- compose 中镜像名与本次版本一致(`mihomo-subscription:X.Y.Z`)。
- 如计划公开分发:将占位 `logo.png` 替换为正式设计。

## 滚动 Changelog

按 `changelog.md` 维护规则:

1. 将 `[Unreleased]` 重命名为 `[X.Y.Z] - YYYY-MM-DD`。
2. 在其上方新建空的 `[Unreleased]` 段。
3. 不删除任何历史版本条目。

## 构建并推送镜像

采用 Docker Hub 策略:在任意装有 docker buildx 的机器上多架构构建并推送,镜像名
与 compose 中的 `image` 字段一致(`quinlanhoo/mihomo-subscription:<version>`)。
1Panel 主机安装时直接 `docker pull`,无需同步源码或本地构建。

```bash
VERSION=0.2.1
NS=quinlanhoo

# 登录 Docker Hub(建议用 Personal Access Token,非 TTY 用 --password-stdin)
echo "<token>" | docker login -u ${NS} --password-stdin

# 多架构需要 docker-container driver 的 builder(默认的 docker driver 不支持)
docker buildx create --name multiarch --driver docker-container --use --bootstrap 2>/dev/null || \
  docker buildx use multiarch

# 多架构构建并推送
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t ${NS}/mihomo-subscription:${VERSION} \
  -t ${NS}/mihomo-subscription:latest \
  --push .
```

推送前可先做单架构本地冒烟验证:

```bash
docker build -t mihomo-subscription:${VERSION} .
docker run --rm -p 8080:8080 -v "$(pwd)/tmp-data:/data" \
  mihomo-subscription:${VERSION}
curl -fsS http://localhost:8080/health
```

## 更新 1Panel 应用包

每个版本新增一个版本目录,保留旧版本目录不删除:

```bash
VERSION=0.2.2
PREV=0.2.1
cp -R apps/mihomo-subscription/${PREV} apps/mihomo-subscription/${VERSION}
```

然后:

1. 更新 `apps/mihomo-subscription/${VERSION}/docker-compose.yml` 中的镜像 tag。
2. 如有新增安装参数,更新该目录 `data.yml` 的 `formFields`。
3. 如应用元数据变化,更新根 `data.yml` 与 `README.md`。

本地安装验证:

```bash
# 复制到 1Panel 主机
/opt/1panel/resource/apps/local/mihomo-subscription
```

然后在 1Panel 应用商店刷新列表,安装并验证:登录、创建配置、生成链接、
客户端可拉取 YAML。详见 `1panel-app.md`。

## 打标签

仓库纳入 git 管理后:

```bash
git tag -a v${VERSION} -m "Release v${VERSION}"
git push origin v${VERSION}
```

## 创建 GitHub Release

打完 tag 后,基于该 tag 发布一个 GitHub Release,release notes 取自
`changelog.md` 对应版本段的要点(部署说明 + Added/Changed/Security)。

```bash
gh release create v${VERSION} \
  --verify-tag \
  --title "v${VERSION}" \
  --notes "$(...)"   # 从 changelog [${VERSION}] 整理
```

## 发布后

- 确认 `[Unreleased]` 为空段并位于最新版本之上。
- 确认 GitHub Release 已发布且指向正确的 tag。
- 在 1Panel 实际环境安装新版本做最终验证。
- 如发现发布缺陷,修复走新的 PATCH 版本,不覆盖已发布的镜像 tag。

## 可选:本地构建(离线/内网)

当 1Panel 主机无法访问 Docker Hub(离线或内网)时,把仓库同步到主机上本地构建,
并把该版本 compose 的 `image` 字段临时改成本地 tag。

```bash
VERSION=0.2.1

# 在 1Panel 主机上构建,镜像名需与 compose 的 image 字段一致
docker build -t mihomo-subscription:${VERSION} .
```
