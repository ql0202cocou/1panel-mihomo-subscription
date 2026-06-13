# AGENTS.md

Concise guidance for coding agents. `mihomo-subscription` is a Rust/Axum service
for self-hosted Mihomo subscription conversion/distribution on 1Panel.

## Layout

- `src/`: library crate (`lib.rs` + per-feature modules) + thin `main.rs`;
  `app.rs` wires all routes via `build_router`.
- `migrations/`: SQLx SQLite migrations (embedded at compile time).
- `web/`: Vite + React + TS SPA, built to `web/dist`, served by Axum.
- `Dockerfile`: multi-stage (Node SPA + Rust). `apps/mihomo-subscription`: 1Panel package.
- `docs/`: `api-design` (API, auth, converter top-level keys), `data-model`,
  `security-design`, `1panel-app` (packaging + authoritative env-var table),
  `release`, `changelog`. Read these before changing related code.

The design is implemented (backend + SPA); the 1Panel install form is not yet
updated to match (see `docs/1panel-app.md`).

## Commands

```bash
# Backend, from repo root — the CI gates:
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit            # needs `cargo install cargo-audit`; ignores in .cargo/audit.toml
# Frontend, from web/:
npm ci && npm run build # tsc --noEmit + vite build -> web/dist
# Image / 1Panel YAML:
docker build -t mihomo-subscription:0.1.2 .
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f) }' \
  apps/mihomo-subscription/data.yml apps/mihomo-subscription/0.1.2/{data,docker-compose}.yml
```

## Change Rules

- Branch off `main` and open a PR; never push to `main`. Changes must pass CI
  (backend gates + frontend build + 1Panel YAML validation). Keep changes small.
- Every notable code/behavior/packaging/security/doc change updates
  `docs/changelog.md` under `[Unreleased]`, and any affected doc
  (`api-design`/`data-model`/`security-design`/`1panel-app`). Then review and
  align both `CLAUDE.md` and this file.
- Never delete changelog history, user data, or generated app-package files
  unless explicitly asked. On release: roll `[Unreleased]` into a dated version,
  tag `vX.Y.Z`, and keep `Cargo.toml`, `web/package.json`, and the app-package
  version dir / image tag in sync (current release `v0.1.2`).

## Security Defaults (see `docs/security-design.md` for rationale)

- Management UI/API require login; admin creds come from `ADMIN_USERNAME` /
  `ADMIN_PASSWORD` env. Verify `Origin` on state-changing requests; the SPA is
  same-origin — never add a permissive CORS layer.
- Public links use `PUBLIC_PATH_PREFIX` + per-profile token; invalid path,
  invalid token, and disabled profile all return a uniform `404`. Rate-limit
  public downloads by client IP (not path) to throttle enumeration. Keep
  `/health` minimal (no version).
- Provider fetch: SSRF protection with timeout/redirect/size limits before
  fetching user URLs; pin the validated IP (DNS-rebinding safe) and unwrap
  IPv4-embedded IPv6; never log full provider URLs; coalesce refreshes behind a
  per-profile single-flight lock.
- Derive client IP from a trusted reverse-proxy hop, not a spoofable header.
  Parse all untrusted/admin YAML via `yaml::parse_limited` (anchor/alias cap
  before parse, depth/node limits after). Set `foreign_keys`/`busy_timeout`/
  `journal_mode` on every pooled SQLite connection (see `docs/data-model.md`).

## 1Panel Notes

- Package lives at `apps/mihomo-subscription`; follow official layout. Use
  `${CONTAINER_NAME}`, `PANEL_APP_PORT_HTTP` for the web port, join the external
  `1panel-network`, persist data via `./data`, and expose `ADMIN_USERNAME` /
  `ADMIN_PASSWORD` as install fields passed into the service environment.
- Images are published to Docker Hub as `quinlanhoo/mihomo-subscription:<version>`
  (multi-arch amd64+arm64) and pulled by the 1Panel host; on-host local build is
  the offline/intranet fallback. Keep the compose image tag in sync with the
  release (see `docs/release.md`). The reverse proxy must preserve `Host` (else
  CSRF `403`).
- `apps/mihomo-subscription/logo.png` is a placeholder; replace before public
  distribution.
