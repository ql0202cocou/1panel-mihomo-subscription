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

#### Frontend Build and Serving

The SPA lives in `web/` at the repository root:

```text
web/
  src/
  package.json
  vite.config.ts
```

- Development: `npm run dev` in `web/`; the Vite dev server proxies `/api` and
  `/health` to the Rust service at `http://localhost:8080`.
- Build: `npm run build` outputs static assets to `web/dist`.
- Serving: Axum serves `web/dist` through `tower-http` `ServeDir` with an SPA
  fallback to `index.html`. API routes and the public subscription route take
  precedence over the static fallback.
- Docker: the `Dockerfile` gains a Node build stage (`node:20-slim`) that
  builds `web/dist`; the runtime image copies only the built assets and ships
  no Node runtime.
- The login page is an SPA route. Management API calls without a valid session
  return `401` and the SPA redirects to login.

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

#### Top-Level Key Handling

The converter must treat every top-level key of the fetched provider config
explicitly:

| Key | Handling |
|-----|----------|
| `proxies` | Provider entries preserved; enabled custom nodes appended |
| `proxy-groups` | Provider entries preserved; enabled custom groups appended |
| `rules` | Replaced entirely with the user-defined rules |
| `rule-providers` | Passed through unchanged (user rules may reference provider `RULE-SET`s) |
| `proxy-providers` | **Stripped in the MVP**: remote node providers would make the client fetch URLs that bypass this service's SSRF protection and caching, and may expose provider URLs |
| All others (`port`, `mixed-port`, `dns`, `tun`, `sniffer`, ...) | Passed through unchanged |

Unknown keys are passed through rather than dropped, so newer Mihomo options
keep working without converter updates. Clients typically override local
general settings anyway; passthrough keeps the output predictable.

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

## Testing Strategy

Tests are written alongside each implementation phase, not deferred.

- **Converter** (required before MVP release): fixture-based unit tests —
  input provider YAML plus profile config, asserted against expected output
  YAML. Cover node/group appending, rule replacement, top-level key
  passthrough/stripping, group name collisions, and missing-group reference
  errors.
- **SSRF validator** (required before MVP release): table-driven tests over
  every blocked range listed in `security-design.md`, plus scheme rejection,
  credential-in-URL rejection, DNS resolution checks, and redirect re-checks.
- **API**: integration tests using `sqlite::memory:` and `tower::ServiceExt`
  request injection. Cover auth (`401` without session), profile CRUD,
  generate-time validation errors, and the public endpoint's uniform `404`
  behavior for invalid path/token/disabled profiles.
- **Frontend**: `tsc --noEmit` and a production `npm run build` are the
  minimum gates; component tests are optional in the MVP.

`cargo test` must pass in the pre-release checklist (`docs/release.md`); the
converter and SSRF suites are hard gates for the MVP release.

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

### Environment Variables

This table is the authoritative list. The 1Panel install form
(`apps/mihomo-subscription/<version>/data.yml`), compose file, and service
code must stay consistent with it. "In package" marks what the current
prototype package already exposes.

| Variable | Source | Default | In package | Purpose |
|----------|--------|---------|------------|---------|
| `PANEL_APP_PORT_HTTP` | Install form | `8080` | Yes | Host web port mapping |
| `RUST_LOG` | Install form | `info` | Yes | Log level |
| `ADMIN_USERNAME` | Install form | — (required) | No | Management login account |
| `ADMIN_PASSWORD` | Install form | — (required) | No | Management login password |
| `PUBLIC_BASE_URL` | Install form | — (required) | No | Externally reachable origin for generated links |
| `PUBLIC_PATH_PREFIX` | Install form (optional) | random | No | Seed for the public path prefix; runtime value lives in `app_settings` and is resettable (see `data-model.md`) |
| `FETCH_TIMEOUT_SECONDS` | Install form | `15` | No | Provider fetch total timeout |
| `MAX_SUBSCRIPTION_SIZE_MB` | Install form | `8` | No | Provider response size limit |
| `CACHE_TTL_MINUTES` | Install form | `15` | No | Generated YAML cache TTL (see `security-design.md`) |
| `PORT` | Compose (fixed) | `8080` | Yes | Container listen port |
| `DATA_DIR` | Compose (fixed) | `/data` | Yes | SQLite data directory |

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
