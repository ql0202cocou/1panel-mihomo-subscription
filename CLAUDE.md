# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Status — Read This First

This project is in the **planning stage**. The design documents under `docs/`
are the source of truth; `src/main.rs` is an early prototype scaffold that does
NOT implement the documented design (no auth, no SSRF protection, a
`subscriptions` CRUD instead of the documented `profiles` model). Doc/code
mismatches are expected — the code will be rewritten to match the docs, not the
other way around. Mark design docs with a "状态:规划阶段 / Status: planning"
banner until implemented.

`AGENTS.md` is the authoritative guide for change rules, security defaults, and
1Panel packaging notes. Its most important rule: **every notable change must
update `docs/changelog.md` under `[Unreleased]`**, and affected docs must stay
aligned with the change. Never delete old changelog entries.

**After every change, review and update both `CLAUDE.md` and `AGENTS.md`** so
agent guidance stays aligned with the actual project state (e.g. when the
prototype starts implementing the documented design, rewrite the status
section above).

## Commands

```bash
cargo check
cargo fmt
cargo test
docker build -t mihomo-subscription:0.1.0 .
```

Validate 1Panel YAML after editing anything under `apps/`:

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "OK #{f}" }' \
  apps/mihomo-subscription/data.yml \
  apps/mihomo-subscription/0.1.0/data.yml \
  apps/mihomo-subscription/0.1.0/docker-compose.yml
```

There are no tests yet.

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
  (no timing disclosure of the path prefix).
- The management API is same-origin with no CORS layer; remove the prototype's
  `CorsLayer::permissive()` when session auth lands.

**Current prototype:** everything lives in `src/main.rs` — Axum routes
(`/api/v1/subscriptions*`, `/api/v1/merged`, `/health`) over a single SQLite
table, DB created at `${DATA_DIR:-/data}/mihomo-subscription.db`. These routes
will be replaced with no compatibility shim.

## Conventions

- New technical docs are bilingual: Chinese first, English following, in the
  same file with shared code blocks.
- The 1Panel app package (`apps/mihomo-subscription/`) mirrors the official
  layout; each release adds a new version directory and keeps old ones. Image
  strategy is local-only: built on the 1Panel host as
  `mihomo-subscription:<version>`, no remote registry (see `docs/release.md`).
  `logo.png` is a generated placeholder — replace before public distribution.
