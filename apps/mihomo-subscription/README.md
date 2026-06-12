# Mihomo 订阅管理

轻量级的 [Mihomo (Clash Meta)](https://github.com/MetaCubeX/mihomo) 代理订阅管理服务，基于 Rust 构建，提供 REST API 用于管理多个代理订阅链接。

## 功能特性

- **订阅管理**：增删改查代理订阅链接
- **启用/禁用**：灵活控制每条订阅的状态
- **合并输出**：聚合所有已启用订阅，供 Mihomo 外部提供商使用
- **持久化存储**：使用 SQLite 存储订阅数据
- **健康检查**：内置 `/health` 端点

## API 接口

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/api/v1/subscriptions` | 获取所有订阅 |
| POST | `/api/v1/subscriptions` | 创建订阅 |
| GET | `/api/v1/subscriptions/:id` | 获取单条订阅 |
| PUT | `/api/v1/subscriptions/:id` | 更新订阅 |
| DELETE | `/api/v1/subscriptions/:id` | 删除订阅 |
| GET | `/api/v1/merged` | 获取所有已启用订阅的合并列表 |

## 快速开始

### 添加订阅

```bash
curl -X POST http://localhost:8080/api/v1/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"name": "机场A", "url": "https://example.com/subscribe?token=xxx"}'
```

### 获取合并配置

```bash
curl http://localhost:8080/api/v1/merged
```

## 环境变量

| 变量 | 默认值 | 描述 |
|------|--------|------|
| `PORT` | `8080` | 监听端口 |
| `DATA_DIR` | `/data` | 数据存储目录 |
| `RUST_LOG` | `info` | 日志级别 (`debug`/`info`/`warn`/`error`) |
