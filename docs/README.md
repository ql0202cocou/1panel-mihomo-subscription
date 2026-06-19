# 文档

此目录存储 Mihomo 订阅管理器的项目文档、打包说明和发布材料。

这些是已实现服务的维护阶段参考文档。（开发阶段规划文档 `plan.md` 和 `technical-roadmap.md` 在设计实现后被移除；其持久内容已并入下面的文档——环境变量表并入 `1panel-app.md`，转换器的顶级键处理并入 `api-design.md`。）

## 文档列表

- `api-design.md`：API 请求/响应契约、认证行为和转换器的顶级键处理。
- `data-model.md`：SQLite 模式、索引和迁移说明。
- `security-design.md`：安全目标、公共链接设计、SSRF 保护、认证和滥用控制说明。
- `1panel-app.md`：1Panel 本地应用打包、权威环境变量表和验证说明。
- `release.md`：镜像构建、标签和发布步骤。
- `changelog.md`：变更日志模板和显著项目变更。

文档化的设计已实现（后端 + SPA）并发布：`0.2.0` 1Panel 应用包及其完整安装表单已完成（`1panel-app.md`），镜像已发布到 Docker Hub（`release.md`）。实现权衡和每版本变更记录在 `changelog.md` 中。