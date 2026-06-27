# 发布流程

> 镜像策略:**发布到 Docker Hub**(`quinlanhoo/mihomo-subscription`,多架构 amd64+arm64),
> 1Panel 主机用 docker compose 直接 `docker pull`,无需同步源码;离线/内网用文末本地构建。

相关:`changelog.md`(发布时滚动)、`1panel-app.md`(compose 部署 + 环境变量)。

## 版本规则

- 语义化版本 `MAJOR.MINOR.PATCH`;`0.x` 允许破坏性变更,但每项须记入 changelog。
- 镜像 tag、`Cargo.toml`、`web/package.json`(及其锁文件)保持一致。

## 发布前检查

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
( cd web && npm ci && npm run build )
```

人工确认:`changelog.md` 的 `[Unreleased]` 已含本次全部变更;受影响文档已对齐;版本号在各处一致。

## 滚动 Changelog

1. `[Unreleased]` 改名为 `[X.Y.Z] - YYYY-MM-DD`;2. 其上方新建空 `[Unreleased]`;3. 不删历史条目。

## 构建并推送镜像

```bash
VERSION=0.0.0; NS=quinlanhoo

# 登录(建议 PAT;非 TTY 用 --password-stdin)
echo "<token>" | docker login -u ${NS} --password-stdin

# 多架构需 docker-container driver 的 builder(默认 docker driver 不支持)
docker buildx create --name multiarch --driver docker-container --use --bootstrap 2>/dev/null \
  || docker buildx use multiarch

docker buildx build --platform linux/amd64,linux/arm64 \
  -t ${NS}/mihomo-subscription:${VERSION} -t ${NS}/mihomo-subscription:latest --push .
```

推送前可单架构冒烟:`docker build -t mihomo-subscription:${VERSION} .` →
`docker run --rm -p 8080:8080 -e ADMIN_USERNAME=admin -e ADMIN_PASSWORD=test -v "$(pwd)/tmp-data:/data" mihomo-subscription:${VERSION}` →
`curl -fsS http://localhost:8080/health`。

## 打标签 + GitHub Release

```bash
git tag -a v${VERSION} -m "Release v${VERSION}" && git push origin v${VERSION}
gh release create v${VERSION} --verify-tag --title "v${VERSION}" --notes "..."  # notes 取自 changelog 对应版本
```

## 发布后

- 确认 `[Unreleased]` 为空且在最新版本之上;GitHub Release 指向正确 tag;在 1Panel 用 compose
  拉取新镜像部署验证(登录、建配置、生成链接、客户端可拉取 YAML;部署细节见 `1panel-app.md`)。
- 发布缺陷走新 PATCH 版本,不覆盖已发布的镜像 tag。

## 可选:本地构建(离线/内网)

主机无法访问 Docker Hub 时,把仓库同步到主机本地构建,并把 compose 的 `image` 临时改为本地 tag
(镜像名须与 compose `image` 一致):

```bash
docker build -t mihomo-subscription:${VERSION} .
```
