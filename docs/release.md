# 发布流程

> 镜像策略:**发布到 Docker Hub**(`quinlanhoo/mihomo-subscription`,多架构 amd64+arm64),
> 1Panel 主机直接 `docker pull`,无需同步源码;离线/内网用文末本地构建。

相关:`changelog.md`(发布时滚动)、`1panel-app.md`(应用包结构)。

## 版本规则

- 语义化版本 `MAJOR.MINOR.PATCH`;`0.x` 允许破坏性变更,但每项须记入 changelog。
- 镜像 tag、`Cargo.toml`、`web/package.json` 与 1Panel 应用包版本目录保持一致。

## 发布前检查

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test

# 校验 1Panel YAML(替换 <version>)
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "OK #{f}" }' \
  apps/mihomo-subscription/data.yml \
  apps/mihomo-subscription/<version>/{data,docker-compose}.yml
```

人工确认:`changelog.md` 的 `[Unreleased]` 已含本次全部变更;受影响文档已对齐;compose 镜像名
与版本一致;如公开分发,替换占位 `logo.png`。

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
`docker run --rm -p 8080:8080 -v "$(pwd)/tmp-data:/data" mihomo-subscription:${VERSION}` →
`curl -fsS http://localhost:8080/health`。

## 更新 1Panel 应用包

```bash
cp -R apps/mihomo-subscription/<prev> apps/mihomo-subscription/${VERSION}
```

然后:1. 更新该目录 `docker-compose.yml` 的镜像 tag;2. 有新增安装参数则更新 `data.yml` 的
`formFields`;3. 应用元数据变化则更新根 `data.yml` 与 `README.md`。本地验证:复制目录到
`/opt/1panel/resource/apps/local/mihomo-subscription`,在应用商店刷新、安装并验证(登录、建配置、
生成链接、客户端可拉取 YAML;详见 `1panel-app.md`)。

## 打标签 + GitHub Release

```bash
git tag -a v${VERSION} -m "Release v${VERSION}" && git push origin v${VERSION}
gh release create v${VERSION} --verify-tag --title "v${VERSION}" --notes "..."  # notes 取自 changelog 对应版本
```

## 发布后

- 确认 `[Unreleased]` 为空且在最新版本之上;GitHub Release 指向正确 tag;在 1Panel 实环境安装验证。
- 发布缺陷走新 PATCH 版本,不覆盖已发布的镜像 tag。

## 可选:本地构建(离线/内网)

主机无法访问 Docker Hub 时,把仓库同步到主机本地构建,并把该版本 compose 的 `image` 临时改为
本地 tag(镜像名须与 compose `image` 一致):

```bash
docker build -t mihomo-subscription:${VERSION} .
```
