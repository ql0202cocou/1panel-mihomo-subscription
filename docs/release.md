# 发布流程 / Release Process

> **状态:0.1.7 应用包已就绪。** 服务、多阶段镜像构建与 1Panel 应用包安装表单
> 均已完成(`apps/mihomo-subscription/0.1.7/`)。镜像策略:**发布到 Docker Hub**
> (`quinlanhoo/mihomo-subscription`,多架构 amd64+arm64),1Panel 主机直接
> `docker pull`,无需在主机上同步源码或本地构建;离线/内网环境可改用文末的
> 本地构建备选流程。
>
> **Status: 0.1.7 app package ready.** The service, the multi-stage image build,
> and the 1Panel app package install form are all complete
> (`apps/mihomo-subscription/0.1.7/`). Image strategy: **published to Docker Hub**
> (`quinlanhoo/mihomo-subscription`, multi-arch amd64+arm64), so the 1Panel host
> just `docker pull`s it — no source sync or on-host build. Use the local-build
> fallback at the end for offline/intranet environments.

相关文档 / Related documents: `changelog.md`(发布时滚动 / rolled at release)、
`1panel-app.md`(应用包结构 / app package layout)。

## 版本规则 / Versioning

- 遵循语义化版本 / Semantic versioning: `MAJOR.MINOR.PATCH`。
- `0.x` 阶段允许破坏性变更,但必须记入 changelog 的 `Changed`。
- 镜像 tag、`Cargo.toml` 的 `version`、1Panel 应用包版本目录三者保持一致。

&nbsp;

- `0.x` releases may contain breaking changes, but each must be recorded under
  `Changed` in the changelog.
- The image tag, `Cargo.toml` `version`, and the 1Panel app package version
  directory must always match.

## 发布前检查 / Pre-release Checklist

```bash
cargo fmt --check
cargo check
cargo test

# 校验 1Panel YAML / validate 1Panel YAML
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "OK #{f}" }' \
  apps/mihomo-subscription/data.yml \
  apps/mihomo-subscription/0.1.7/data.yml \
  apps/mihomo-subscription/0.1.7/docker-compose.yml
```

人工确认 / Manual checks:

- `docs/changelog.md` 的 `[Unreleased]` 包含本次发布的全部变更。
- 受影响的产品/技术/安全文档已与实现对齐(见 `AGENTS.md` Change Rules)。
- compose 中镜像名与本次版本一致(`mihomo-subscription:X.Y.Z`)。
- 如计划公开分发:将占位 `logo.png` 替换为正式设计。

&nbsp;

- `[Unreleased]` in `docs/changelog.md` covers everything in this release.
- All affected product/technical/security docs match the implementation (see
  Change Rules in `AGENTS.md`).
- The compose image name matches this release (`mihomo-subscription:X.Y.Z`).
- If distributing publicly: replace the placeholder `logo.png` with a real
  design.

## 滚动 Changelog / Roll the Changelog

按 `changelog.md` 维护规则 / Per the maintenance rules in `changelog.md`:

1. 将 `[Unreleased]` 重命名为 `[X.Y.Z] - YYYY-MM-DD`。
   Rename `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD`.
2. 在其上方新建空的 `[Unreleased]` 段。
   Create a new empty `[Unreleased]` section above it.
3. 不删除任何历史版本条目。
   Never delete historical entries.

## 构建并推送镜像 / Build and Push the Image

采用 Docker Hub 策略:在任意装有 docker buildx 的机器上多架构构建并推送,镜像名
与 compose 中的 `image` 字段一致(`quinlanhoo/mihomo-subscription:<version>`)。
1Panel 主机安装时直接 `docker pull`,无需同步源码或本地构建。

Docker Hub strategy: multi-arch build and push from any machine with docker
buildx; the image name matches the `image` field in compose
(`quinlanhoo/mihomo-subscription:<version>`). The 1Panel host just `docker pull`s
it at install time — no source sync or on-host build.

```bash
VERSION=0.1.7
NS=quinlanhoo

# 登录 Docker Hub(建议用 Personal Access Token,非 TTY 用 --password-stdin)
# log in to Docker Hub (prefer a Personal Access Token; use --password-stdin in non-TTY)
echo "<token>" | docker login -u ${NS} --password-stdin

# 多架构需要 docker-container driver 的 builder(默认的 docker driver 不支持)
# multi-arch needs a docker-container driver builder (the default docker driver can't)
docker buildx create --name multiarch --driver docker-container --use --bootstrap 2>/dev/null || \
  docker buildx use multiarch

# 多架构构建并推送 / multi-arch build and push
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t ${NS}/mihomo-subscription:${VERSION} \
  -t ${NS}/mihomo-subscription:latest \
  --push .
```

推送前可先做单架构本地冒烟验证 / Single-arch local smoke test before pushing:

```bash
docker build -t mihomo-subscription:${VERSION} .
docker run --rm -p 8080:8080 -v "$(pwd)/tmp-data:/data" \
  mihomo-subscription:${VERSION}
curl -fsS http://localhost:8080/health
```

## 更新 1Panel 应用包 / Update the 1Panel App Package

每个版本新增一个版本目录,保留旧版本目录不删除:

Each release adds a new version directory; old version directories are kept:

```bash
VERSION=0.2.0
PREV=0.1.7
cp -R apps/mihomo-subscription/${PREV} apps/mihomo-subscription/${VERSION}
```

然后 / Then:

1. 更新 `apps/mihomo-subscription/${VERSION}/docker-compose.yml` 中的镜像 tag。
   Update the image tag in the new `docker-compose.yml`.
2. 如有新增安装参数,更新该目录 `data.yml` 的 `formFields`。
   Update `formFields` in the version `data.yml` if install parameters changed.
3. 如应用元数据变化,更新根 `data.yml` 与 `README.md` / `README_en.md`。
   Update the root `data.yml` and READMEs if app metadata changed.

本地安装验证 / Local install validation:

```bash
# 复制到 1Panel 主机 / copy to the 1Panel host
/opt/1panel/resource/apps/local/mihomo-subscription
```

然后在 1Panel 应用商店刷新列表,安装并验证:登录、创建配置、生成链接、
客户端可拉取 YAML。详见 `1panel-app.md`。

Then refresh the 1Panel App Store list, install, and verify: login, create a
profile, generate a link, and confirm a client can fetch the YAML. See
`1panel-app.md`.

## 打标签 / Tag the Release

仓库纳入 git 管理后 / Once the repository is under git:

```bash
git tag -a v${VERSION} -m "Release v${VERSION}"
git push origin v${VERSION}
```

## 创建 GitHub Release / Create the GitHub Release

打完 tag 后,基于该 tag 发布一个 GitHub Release,release notes 取自
`changelog.md` 对应版本段的要点(部署说明 + Added/Changed/Security)。

After tagging, publish a GitHub Release from that tag; draw the notes from the
matching version section of `changelog.md` (deployment notes + Added/Changed/
Security).

```bash
gh release create v${VERSION} \
  --verify-tag \
  --title "v${VERSION}" \
  --notes "$(...)"   # 从 changelog [${VERSION}] 整理 / summarize from changelog
```

## 发布后 / Post-release

- 确认 `[Unreleased]` 为空段并位于最新版本之上。
- 确认 GitHub Release 已发布且指向正确的 tag。
- 在 1Panel 实际环境安装新版本做最终验证。
- 如发现发布缺陷,修复走新的 PATCH 版本,不覆盖已发布的镜像 tag。

&nbsp;

- Confirm `[Unreleased]` is an empty section above the newest version.
- Confirm the GitHub Release is published and points at the correct tag.
- Install the new version in a real 1Panel environment for final validation.
- Fix release defects in a new PATCH version; never overwrite a published
  image tag.

## 可选:本地构建(离线/内网) / Optional: Local Build (Offline/Intranet)

当 1Panel 主机无法访问 Docker Hub(离线或内网)时,把仓库同步到主机上本地构建,
并把该版本 compose 的 `image` 字段临时改成本地 tag。

When the 1Panel host cannot reach Docker Hub (offline or intranet), sync the
repository to the host and build locally, and temporarily change the version's
compose `image` field to the local tag.

```bash
VERSION=0.1.7

# 在 1Panel 主机上构建,镜像名需与 compose 的 image 字段一致
# build on the 1Panel host; the image name must match the compose image field
docker build -t mihomo-subscription:${VERSION} .
```
