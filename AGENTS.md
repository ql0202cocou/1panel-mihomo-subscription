# AGENTS.md

Concise guidance for coding agents working on this repository.

## Project

`mihomo-subscription` is a Rust/Axum service for self-hosted Mihomo subscription
conversion and distribution on 1Panel.

Key paths:

- `src/`: Rust service — a library crate (`lib.rs` + per-feature modules) plus a
  thin `main.rs`; `app.rs` wires all routes via `build_router`.
- `migrations/`: SQLx SQLite migrations (embedded into the binary at compile time).
- `web/`: Vite + React + TypeScript SPA, built into `web/dist` and served by Axum.
- `Dockerfile`: multi-stage container build (Node SPA stage + Rust stage).
- `apps/mihomo-subscription`: 1Panel app package.
- `docs`: product, technical, security, release, and changelog documents.
- `.github/workflows/ci.yml`: CI gates.

## Read First

- `docs/plan.md`: product scope and MVP requirements.
- `docs/technical-roadmap.md`: architecture and implementation direction.
- `docs/api-design.md`: API contracts and authentication behavior.
- `docs/data-model.md`: SQLite schema and migration strategy.
- `docs/security-design.md`: required security behavior.
- `docs/changelog.md`: change history and changelog rules.

The documented design is implemented (backend + SPA). Keep code and docs
aligned; track implementation trade-offs in `docs/changelog.md`. The 1Panel app
package install form is not yet updated to match (see `docs/1panel-app.md`).

## Commands

Backend, from the repository root — these are the CI gates:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit   # needs `cargo install cargo-audit`; per-advisory ignores in .cargo/audit.toml
```

Frontend, from `web/`:

```bash
npm ci          # or npm install
npm run build   # tsc --noEmit + vite build -> web/dist
```

Build image:

```bash
docker build -t mihomo-subscription:0.1.0 .
```

Validate 1Panel YAML:

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "OK #{f}" }' \
  apps/mihomo-subscription/data.yml \
  apps/mihomo-subscription/0.1.0/data.yml \
  apps/mihomo-subscription/0.1.0/docker-compose.yml
```

## Repository & CI

- Hosted on GitHub (`origin`), default branch `main`. Branch off `main` and
  open a PR; avoid pushing directly to `main`.
- GitHub Actions (`.github/workflows/ci.yml`) runs the backend gates
  (fmt/clippy/test), the frontend build, and 1Panel YAML validation. Changes
  must pass CI.
- Releases: roll the changelog `[Unreleased]` into a dated version and tag
  `vX.Y.Z` (current release: `v0.1.0`); keep `Cargo.toml`, `web/package.json`,
  and the app package version directory/image tag in sync.

## Change Rules

- Keep changes small and focused.
- Do not delete user data, generated app package files, or changelog history
  unless explicitly requested.
- Every notable code, behavior, packaging, security, or documentation change
  must update `docs/changelog.md` under `[Unreleased]`.
- Every change must also update any affected technical/product docs so the
  documentation stays aligned with the actual project state.
- After every change, review `CLAUDE.md` and this `AGENTS.md` and update both
  so agent guidance stays aligned with the actual project state.
- Never delete old changelog versions. On release, move `[Unreleased]` items into
  a dated version section and create a new empty `[Unreleased]` above it.
- If product scope changes, update `docs/plan.md`.
- If architecture or data model direction changes, update
  `docs/technical-roadmap.md`.
- If auth, public links, SSRF, logging, or sensitive-data handling changes,
  update `docs/security-design.md`.

## Security Defaults

- The management UI and management APIs require login.
- Admin credentials come from 1Panel compose environment variables:
  `ADMIN_USERNAME` and `ADMIN_PASSWORD`.
- Public subscription links use both `PUBLIC_PATH_PREFIX` and a per-profile
  token.
- Invalid public path, invalid token, and disabled profile should all return
  `404 Not Found`.
- Do not log full provider subscription URLs.
- Apply SSRF protection, timeout, redirect, and response-size limits before
  fetching user-provided URLs; pin validated IPs (DNS-rebinding safe) and
  unwrap IPv4-embedded IPv6 addresses per `docs/security-design.md`.
- Never enable permissive CORS on the management API; the SPA is served
  same-origin (no CORS layer). Verify `Origin` on state-changing requests.
- Refresh provider subscriptions behind a per-profile single-flight lock to
  avoid stale-cache stampedes; derive the client IP from a trusted reverse
  proxy hop, not a client-spoofable header (see `docs/security-design.md`).
- Set `foreign_keys`, `busy_timeout`, and `journal_mode` on every pooled
  SQLite connection via an after-connect hook (see `docs/data-model.md`).
- Parse all untrusted/admin YAML via `yaml::parse_limited` (anchor/alias cap
  before parse, depth/node limits after). Rate-limit public downloads by client
  IP (not path) so token enumeration is throttled. Keep `/health` minimal (no
  version). The reverse proxy must preserve `Host` (`docs/1panel-app.md`).

## 1Panel Notes

- Keep the app package at `apps/mihomo-subscription`.
- Use `${CONTAINER_NAME}` in `docker-compose.yml`.
- Use `PANEL_APP_PORT_HTTP` for the web port.
- Expose `ADMIN_USERNAME` and `ADMIN_PASSWORD` as install form fields and pass
  them into the service environment.
- Join the external `1panel-network`.
- Persist app data through `./data`.
- Images are built locally on the 1Panel host as
  `mihomo-subscription:<version>`; keep the compose image tag in sync with the
  release version (see `docs/release.md`).
- `apps/mihomo-subscription/logo.png` is a generated placeholder; replace it
  with a real design before public distribution.
