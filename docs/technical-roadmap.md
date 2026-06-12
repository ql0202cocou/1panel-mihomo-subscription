# Technical Roadmap

This project aims to become a self-hosted Mihomo subscription conversion and
distribution service for 1Panel, with functionality similar to a lightweight
Sub-Store.

## Recommended Direction

Build this as a single-user/self-hosted "Sub-Store Lite" first, then gradually
expand conversion formats and rule-management capabilities.

The current Rust + Axum + SQLite + Docker + 1Panel app package foundation is a
good fit for this direction. Do not rewrite the project unless the requirements
change substantially.

## Target Architecture

```text
Web UI
  |
  | REST API
  v
Rust/Axum Service
  |
  |-- SQLite: profiles, rules, custom nodes, custom groups, tokens, access records
  |-- Converter: fetch source subscription, parse, append nodes/groups, replace rules
  |-- Security: token, access control, URL validation, SSRF protection
  |
  v
Permanent Subscription Link
/api/sub/:token
```

## Recommended Stack

### Frontend

Use a lightweight SPA:

```text
Vite + React + TypeScript
```

The UI will need interactive forms, rule lists, preview, and copy-link actions.
A small SPA will be easier to maintain than plain server-rendered HTML once the
feature set grows.

### Backend

Continue with the current backend stack:

```text
Rust + Axum + SQLx + SQLite
```

SQLite is appropriate for a 1Panel self-hosted application. Consider PostgreSQL
only if the project later adds multi-user or high-concurrency requirements.

### YAML Processing

Use structured YAML parsing, such as `serde_yaml`, instead of string replacement.
Mihomo, Surge, and Loon differ in rule and proxy structures, so structured
parsing will reduce conversion bugs.

## Core Data Model

Initial tables:

```text
profiles
- id
- name
- source_type        # mihomo / loon / surge / clash
- source_url
- output_type        # initially fixed to mihomo
- public_path        # random public path prefix
- token              # permanent subscription token
- enabled
- created_at
- updated_at

rulesets
- id
- profile_id
- name
- content            # custom rule text
- priority
- enabled

custom_nodes
- id
- profile_id
- name
- node_type          # ss / vmess / vless / trojan / hysteria2 / etc.
- content            # structured node config or raw Mihomo YAML fragment
- enabled
- created_at
- updated_at

custom_groups
- id
- profile_id
- name
- group_type         # select / url-test / fallback / load-balance / relay
- members            # ordered node/group names, stored as JSON or normalized rows
- options            # group-specific options, stored as JSON
- enabled
- created_at
- updated_at

generated_cache
- profile_id
- content_hash
- output_yaml
- generated_at
```

Permanent download links should not expose database IDs. Use a random public path
prefix plus a long random token:

```text
/s/7fKp9mQx/api/sub/3w7s9xQm.../mihomo.yaml
```

## Phase 1: MVP

Goal: deliver a usable 1Panel-hosted subscription converter.

### Web UI

- Subscription type selector: `mihomo`, `clash`, `surge`, `loon`.
- Source subscription URL input.
- Custom rule editor or editable rule list.
- Custom node editor.
- Custom proxy group editor.
- Save profile.
- Generate permanent subscription link.
- Copy generated link.

### Backend API

Suggested endpoints:

```text
POST   /api/profiles
GET    /api/profiles
GET    /api/profiles/:id
PUT    /api/profiles/:id
DELETE /api/profiles/:id
POST   /api/profiles/:id/generate
GET    /api/sub/:token
```

### Conversion Logic

Start with:

```text
mihomo/clash -> mihomo
```

Initial behavior:

- Fetch remote subscription YAML.
- Preserve `proxies`.
- Preserve `proxy-groups`.
- Append enabled custom nodes into `proxies`.
- Append enabled custom groups into `proxy-groups`.
- Validate that custom groups reference existing provider nodes, provider groups,
  or enabled custom nodes/groups.
- Replace `rules` with user-defined rules.
- Return a valid Mihomo YAML document.

## Phase 2: Security and Stability

Goal: make the app safe enough for long-running personal deployment.

### SSRF Protection

- Allow only `http` and `https` URLs.
- Block localhost and loopback addresses.
- Block private LAN ranges.
- Block link-local ranges.
- Block Docker/internal network ranges where practical.
- Set outbound request timeout.
- Set maximum subscription response size.

### Permanent Link Security

- Use at least 32 random bytes for tokens.
- Add a random public path prefix before the tokenized subscription endpoint.
- Do not expose the original source subscription URL in generated links.
- Support token reset/regeneration.
- Support public path reset/regeneration.

### Management Access Control

- Add an administrator password for the Web UI.
- Protect management APIs with a session or bearer token.
- Keep generated subscription links public but token-protected.

## Phase 3: Format Expansion

Add source format support gradually.

Recommended order:

```text
1. mihomo/clash -> mihomo
2. surge -> mihomo
3. loon -> mihomo
4. sing-box -> mihomo, if needed
```

Avoid building full multi-format compatibility in the first version. Mihomo
output should remain the main product path.

## 1Panel Delivery Plan

Keep the app package at:

```text
apps/mihomo-subscription
```

Continue following official 1Panel app package conventions:

```text
apps/mihomo-subscription/
  data.yml
  README.md
  README_en.md
  logo.png
  0.1.0/
    data.yml
    docker-compose.yml
    data/
```

Recommended install parameters:

- `PANEL_APP_PORT_HTTP`
- `ADMIN_USERNAME`
- `ADMIN_PASSWORD`
- `PUBLIC_BASE_URL`
- `PUBLIC_PATH_PREFIX`
- `RUST_LOG`
- `FETCH_TIMEOUT_SECONDS`
- `MAX_SUBSCRIPTION_SIZE_MB`

`PUBLIC_BASE_URL` should only store the externally reachable origin. Add a
random `PUBLIC_PATH_PREFIX` before the tokenized subscription endpoint when
generating permanent links:

```text
https://sub.example.com/<public-path-prefix>/api/sub/<token>
```

## Immediate Next Steps

Recommended implementation order:

1. Add static Web UI serving to the existing Rust service.
2. Replace the current subscription CRUD model with profile CRUD.
3. Implement `mihomo/clash -> mihomo` YAML parsing and rule replacement.
4. Add custom node and custom proxy group append support.
5. Generate permanent subscription links with a random public path prefix and token.
6. Add administrator username/password login using 1Panel compose environment
   variables.
7. Add SSRF protection and fetch limits.
8. Update the 1Panel app package install form.
