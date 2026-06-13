# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Status — Read This First

The documented design under `docs/` is **implemented** (backend + `web/` SPA),
and the docs remain the source of truth. Implemented: SQLite migration and pool
(`src/db.rs`), session auth and same-origin static serving (`src/auth.rs`,
`src/app.rs`), profile + sub-resource CRUD and settings (`src/profiles.rs`,
`src/settings.rs`), SSRF-protected fetch (`src/ssrf.rs`, `src/fetch.rs`), the
`mihomo`/`clash` converter (`src/converter.rs`), generate/preview/public
endpoint with caching and single-flight (`src/generate.rs`,
`src/single_flight.rs`), rate limiting and client-IP derivation
(`src/rate_limit.rs`, `src/net.rs`), and the SPA under `web/`. Remaining before
a formal release: updating the 1Panel app package install form
(`docs/1panel-app.md` pending markers) and cutting the release
(`docs/release.md`). Implementation trade-offs are tracked in
`docs/changelog.md`.

`AGENTS.md` is the authoritative guide for change rules, security defaults, and
1Panel packaging notes. Its most important rule: **every notable change must
update `docs/changelog.md` under `[Unreleased]`**, and affected docs must stay
aligned with the change. Never delete old changelog entries.

**After every change, review and update both `CLAUDE.md` and `AGENTS.md`** so
agent guidance stays aligned with the actual project state.

## Commands

The gates CI enforces (`.github/workflows/ci.yml`), runnable locally:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit                       # needs `cargo install cargo-audit`; ignores live in .cargo/audit.toml
docker build -t mihomo-subscription:0.1.0 .
```

Run one test or one file's tests:

```bash
cargo test --lib ssrf::tests::url_validation_rules   # a single unit test
cargo test --test profiles                           # one integration test file
```

Frontend (run inside `web/`; CI runs `npm ci` + `npm run build`):

```bash
npm install        # first time
npm run dev        # Vite dev server; proxies /api and /health to :8080
npm run build      # tsc --noEmit + vite build -> web/dist (served by Axum)
npm run typecheck  # tsc --noEmit only
```

Validate 1Panel YAML after editing anything under `apps/`:

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "OK #{f}" }' \
  apps/mihomo-subscription/data.yml \
  apps/mihomo-subscription/0.1.0/data.yml \
  apps/mihomo-subscription/0.1.0/docker-compose.yml
```

## Architecture

**What this is:** a self-hosted Mihomo subscription converter for 1Panel
("Sub-Store Lite"). Admin registers provider subscriptions and custom
rules/nodes/groups via a Web UI; the service fetches provider YAML, appends
custom nodes/groups, replaces `rules`, and serves the result at a permanent
public link: `https://<host>/<public-path-prefix>/api/sub/<profile-token>`.

**Target design** (spread across docs — read together):

- `docs/plan.md` — product scope, MVP boundaries, Web UI structure (Chinese).
- `docs/technical-roadmap.md` — stack (Rust/Axum/SQLx/SQLite + Vite/React),
  phased plan, conversion pipeline (including top-level key
  passthrough/stripping rules), frontend build pipeline (`web/` SPA served by
  Axum with fallback, Node Docker stage), testing strategy (converter and
  SSRF suites are MVP release gates), and the **authoritative environment
  variable table** (install form, compose, and code must stay consistent
  with it).
- `docs/api-design.md` — management API under `/api` (session-cookie auth),
  public subscription endpoint (no auth, uniform `404` on any failure).
- `docs/data-model.md` — SQLite schema: `profiles` 1—1 `rulesets`,
  1—\* `custom_nodes`/`custom_groups`, 1—1 `generated_cache`, plus a
  single-row `app_settings` holding the resettable `public_path_prefix`.
- `docs/security-design.md` — required behavior: SSRF protection on every
  provider fetch, provider URLs masked everywhere (logs, responses, errors),
  tokens ≥32 random bytes.

**Three trust boundaries** drive the design: admin browser → management API
(authenticated), public client → subscription endpoint (path prefix + token,
generic 404s), backend → provider URLs (SSRF-protected fetch with timeout,
redirect, and size limits).

**Non-obvious implementation rules** (scattered across docs — easy to miss,
costly to retrofit; full rationale in `docs/security-design.md` and
`docs/data-model.md`):

- SSRF check must pin the validated IP at connect time (DNS-rebinding safe)
  and unwrap IPv4-embedded IPv6 (IPv4-mapped, NAT64, 6to4).
- Refresh provider subscriptions behind a per-profile single-flight lock to
  prevent stale-cache stampedes.
- Apply `foreign_keys`, `busy_timeout`, and `journal_mode` to every pooled
  SQLite connection via an after-connect hook, not once.
- Derive the client IP from a trusted reverse-proxy hop (`TRUSTED_PROXY_HOPS`),
  never a client-spoofable header.
- Public token lookup runs unconditionally and compares in constant time
  (no timing disclosure of the path prefix); public-download rate limiting is
  keyed by client IP (not path) so it throttles token enumeration.
- Parse untrusted/admin YAML through `yaml::parse_limited`, which caps
  anchors/aliases *before* `serde_yaml` runs (billion-laughs), then depth/nodes.
- The management API is same-origin with no CORS layer (enforced in
  `build_router`); keep it that way and verify `Origin` on state-changing
  requests.

**Code layout:** the crate is split into a library (`src/lib.rs`) plus a thin
binary (`src/main.rs`) so integration tests can drive the app directly.
`src/app.rs` holds `AppState` and `build_router` (the single source of route
wiring); `src/main.rs` only loads env config, builds state, and serves.
Per-feature modules: `db`, `auth`, `profiles`, `settings`, `ssrf`, `fetch`
(`SubscriptionFetcher` trait + `HttpFetcher`, injected into `AppState` so
generate/public paths are testable without network), `converter`, `generate`
(generate/preview + public endpoint), `single_flight`, `rate_limit`, `net`
(client-IP derivation); plus helpers `error` (API error envelope + `sqlx`
UNIQUE→409 mapping), `mask` (provider-URL masking), `yaml` (bounded parsing),
`util`. The DB lives at `${DATA_DIR:-/data}/mihomo-subscription.db`; migrations
are in `migrations/`. The SPA lives in `web/` (Vite + React + Ant Design +
react-i18next) and is built into `web/dist`, which Axum serves with an
`index.html` fallback (`WEB_DIR`).

**Test pattern:** integration tests in `tests/` build the router with
`build_router` and drive it via `tower::util::ServiceExt::oneshot` against a
throwaway SQLite file from `tests/common::TempDb` / `test_state`. Pure logic
(SSRF ranges, URL masking, bounded YAML) is unit-tested inline. The converter
and SSRF suites are MVP release gates.

## Conventions

- New technical docs are bilingual: Chinese first, English following, in the
  same file with shared code blocks.
- The 1Panel app package (`apps/mihomo-subscription/`) mirrors the official
  layout; each release adds a new version directory and keeps old ones. Image
  strategy is local-only: built on the 1Panel host as
  `mihomo-subscription:<version>`, no remote registry (see `docs/release.md`).
  `logo.png` is a generated placeholder — replace before public distribution.
