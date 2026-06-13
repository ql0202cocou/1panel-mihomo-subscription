# CLAUDE.md

Guidance for Claude Code (claude.ai/code) working in this repository.

## Project Status — Read This First

The documented design under `docs/` is **implemented** (Rust/Axum backend +
`web/` SPA); the docs remain the source of truth. Remaining before a formal
release: updating the 1Panel app package install form (`docs/1panel-app.md`
pending markers) and cutting the release (`docs/release.md`).

**Most important rule:** every notable change updates `docs/changelog.md` under
`[Unreleased]` (never delete old entries), keeps affected docs aligned, and
reviews/updates both `CLAUDE.md` and `AGENTS.md`. `AGENTS.md` is the
authoritative guide for change rules, security defaults, and 1Panel packaging.

## Commands

```bash
# CI gates (.github/workflows/ci.yml), runnable locally:
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit                          # needs `cargo install cargo-audit`; ignores in .cargo/audit.toml
docker build -t mihomo-subscription:0.1.0 .

# One test / one file:
cargo test --lib ssrf::tests::url_validation_rules
cargo test --test profiles

# Frontend, from web/ (CI runs `npm ci` + `npm run build`):
npm install        # first time
npm run dev        # Vite dev server; proxies /api and /health to :8080
npm run build      # tsc --noEmit + vite build -> web/dist (served by Axum)

# Validate 1Panel YAML after editing anything under apps/:
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f) }' \
  apps/mihomo-subscription/data.yml apps/mihomo-subscription/0.1.0/{data,docker-compose}.yml
```

## Architecture

**What this is:** a self-hosted Mihomo subscription converter for 1Panel
("Sub-Store Lite"). Admin registers provider subscriptions and custom
rules/nodes/groups via a Web UI; the service fetches provider YAML, appends
custom nodes/groups, replaces `rules`, and serves the result at a permanent
public link: `https://<host>/<public-path-prefix>/api/sub/<profile-token>`.

**Reference docs** (maintenance-phase; read the relevant one before changing
code). The development-phase planning docs (`plan.md`, `technical-roadmap.md`)
were removed once implemented; their durable content moved here:

- `docs/api-design.md` — `/api` management API (session-cookie auth), public
  endpoint (no auth, uniform `404`), converter top-level key passthrough/strip.
- `docs/data-model.md` — SQLite schema: `profiles` 1—1 `rulesets`,
  1—\* `custom_nodes`/`custom_groups`, 1—1 `generated_cache`, single-row
  `app_settings` (resettable `public_path_prefix`).
- `docs/security-design.md` — SSRF on every fetch, provider URLs masked
  everywhere, tokens ≥32 random bytes.
- `docs/1panel-app.md` — packaging + the **authoritative env-var table**
  (install form, compose, and code must stay consistent with it).

**Three trust boundaries:** admin browser → management API (authenticated);
public client → subscription endpoint (path prefix + token, generic 404s);
backend → provider URLs (SSRF-protected fetch with timeout/redirect/size limits).

**Non-obvious implementation rules** (easy to miss, costly to retrofit; full
rationale in `docs/security-design.md` and `docs/data-model.md`):

- SSRF check pins the validated IP at connect time (DNS-rebinding safe) and
  unwraps IPv4-embedded IPv6 (IPv4-mapped, NAT64, 6to4).
- Refresh provider subscriptions behind a per-profile single-flight lock to
  prevent stale-cache stampedes.
- Apply `foreign_keys`, `busy_timeout`, and `journal_mode` to every pooled
  SQLite connection via an after-connect hook, not once.
- Derive the client IP from a trusted reverse-proxy hop (`TRUSTED_PROXY_HOPS`),
  never a client-spoofable header.
- Public token lookup runs unconditionally and compares in constant time (no
  prefix timing disclosure); public-download rate limiting is keyed by client IP
  (not path) so it throttles token enumeration.
- Parse untrusted/admin YAML through `yaml::parse_limited` (caps anchors/aliases
  *before* `serde_yaml` runs — billion-laughs — then depth/nodes).
- The management API is same-origin with no CORS layer (enforced in
  `build_router`); keep it so and verify `Origin` on state-changing requests.

**Code layout:** library (`src/lib.rs`) + thin binary (`src/main.rs`) so
integration tests drive the app directly. `src/app.rs` holds `AppState` and
`build_router` (single source of route wiring); `main.rs` only loads env config,
builds state, and serves. Per-feature modules: `db`, `auth`, `profiles`,
`settings`, `ssrf`, `fetch` (`SubscriptionFetcher` trait + `HttpFetcher`,
injected into `AppState` so generate/public paths are testable without network),
`converter`, `generate` (generate/preview + public endpoint), `single_flight`,
`rate_limit`, `net`; helpers `error` (API envelope + `sqlx` UNIQUE→409), `mask`,
`yaml` (bounded parsing), `util`. DB at `${DATA_DIR:-/data}/mihomo-subscription.db`;
migrations in `migrations/`. SPA in `web/` (Vite + React + Ant Design +
react-i18next) → `web/dist`, served by Axum with an `index.html` fallback (`WEB_DIR`).

**Test pattern:** integration tests in `tests/` build the router with
`build_router` and drive it via `tower::util::ServiceExt::oneshot` against a
throwaway SQLite file (`tests/common::TempDb` / `test_state`). Pure logic (SSRF
ranges, URL masking, bounded YAML) is unit-tested inline. The converter and SSRF
suites are release gates.

## Conventions

- New technical docs are bilingual: Chinese first, English following, in the
  same file with shared code blocks.
- The 1Panel app package mirrors the official layout; each release adds a new
  version directory and keeps old ones. Images are local-only, built on the
  1Panel host as `mihomo-subscription:<version>`, no remote registry (see
  `docs/release.md`). `logo.png` is a placeholder — replace before distribution.
