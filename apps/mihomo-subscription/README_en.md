# Mihomo Subscription Manager

A lightweight [Mihomo (Clash Meta)](https://github.com/MetaCubeX/mihomo) proxy subscription management service built with Rust, providing a REST API to manage multiple proxy subscription URLs.

## Features

- **Subscription CRUD**: Create, read, update, and delete proxy subscription URLs
- **Enable/Disable**: Flexibly toggle each subscription's active state
- **Merged Output**: Aggregate all enabled subscriptions for use with Mihomo external providers
- **Persistent Storage**: SQLite-backed subscription data
- **Health Check**: Built-in `/health` endpoint

## API Reference

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/api/v1/subscriptions` | List all subscriptions |
| POST | `/api/v1/subscriptions` | Create a subscription |
| GET | `/api/v1/subscriptions/:id` | Get a subscription |
| PUT | `/api/v1/subscriptions/:id` | Update a subscription |
| DELETE | `/api/v1/subscriptions/:id` | Delete a subscription |
| GET | `/api/v1/merged` | Get merged list of all enabled subscriptions |

## Quick Start

### Add a Subscription

```bash
curl -X POST http://localhost:8080/api/v1/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"name": "My Provider", "url": "https://example.com/subscribe?token=xxx"}'
```

### Get Merged Config

```bash
curl http://localhost:8080/api/v1/merged
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | Listening port |
| `DATA_DIR` | `/data` | Data storage directory |
| `RUST_LOG` | `info` | Log level (`debug`/`info`/`warn`/`error`) |
