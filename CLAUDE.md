# CLAUDE.md

Guidance for Claude Code (claude.ai/code) working in this repository.

## Project Status — Read This First

The design under `docs/` is **implemented** (Rust/Axum backend + `web/` SPA);
docs remain the source of truth. The `0.1.12` 1Panel package is complete
(`apps/mihomo-subscription/0.1.12/`). Ship a new version via `docs/release.md`
(multi-arch `docker buildx ... --push` to Docker Hub
`quinlanhoo/mihomo-subscription`, tag `vX.Y.Z`; on-host build is the
offline/intranet fallback). This file is the authoritative guide for change
rules, security defaults, and 1Panel packaging.

## Change Rules

- Branch off `main` and open a PR; **never push to `main`**. Keep changes small
  and passing CI (backend gates + frontend build + 1Panel YAML validation).
- Every notable code/behavior/packaging/security/doc change updates
  `docs/changelog.md` under `[Unreleased]` (never delete old entries) plus any
  affected doc (`api-design`/`data-model`/`security-design`/`1panel-app`), then
  re-aligns this file.
- Never delete changelog history, user data, or generated app-package files
  unless asked. On release: roll `[Unreleased]` into a dated version, tag
  `vX.Y.Z`, and keep `Cargo.toml`, `web/package.json`, and the app-package
  version dir / image tag in sync (current `v0.1.12`).

## Commands

```bash
# CI gates (.github/workflows/ci.yml), runnable locally:
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit                          # needs `cargo install cargo-audit`; ignores in .cargo/audit.toml

# Local Dockerfile sanity check (not a CI gate):
docker build -t mihomo-subscription:0.1.12 .

# One test / one file:
cargo test --lib ssrf::tests::url_validation_rules
cargo test --test profiles

# Frontend, from web/ (CI runs `npm ci` + `npm run build`):
npm install        # first time
npm run dev        # Vite dev server; proxies /api and /health to :8080
npm run build      # tsc --noEmit + vite build -> web/dist (served by Axum)

# Validate 1Panel YAML after editing anything under apps/:
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f) }' \
  apps/mihomo-subscription/data.yml apps/mihomo-subscription/0.1.12/{data,docker-compose}.yml
```

## Architecture

**What this is:** a self-hosted Mihomo subscription converter for 1Panel
("Sub-Store Lite"). Admin registers a provider subscription and custom
rules/nodes/groups via the Web UI; the service fetches provider YAML, appends
custom nodes, **replaces** `rules` and `proxy-groups` with the admin's custom
ones (provider rules/groups are imported on demand, not passed through), and
serves the result at a permanent link:
`https://<host>/<public-path-prefix>/api/sub/<profile-token>`.

**Reference docs** (read the relevant one before changing related code):

- `docs/api-design.md` — `/api` management API (session-cookie auth), public
  endpoint (no auth, uniform `404`), converter top-level key passthrough/strip.
- `docs/data-model.md` — SQLite schema: `profiles` 1—1 `rulesets`, 1—\*
  `custom_nodes`/`custom_groups`, 1—1 `generated_cache`, single-row
  `app_settings` (resettable `public_path_prefix`).
- `docs/security-design.md` — SSRF on every fetch, provider URLs masked, tokens
  ≥32 random bytes.
- `docs/1panel-app.md` — packaging + the **authoritative env-var table**.

**Three trust boundaries:** admin browser → management API (authenticated);
public client → subscription endpoint (path prefix + token, generic 404s);
backend → provider URLs (SSRF-protected fetch with timeout/redirect/size limits).

**Non-obvious rules** (costly to retrofit; rationale in `docs/security-design.md`
and `docs/data-model.md`):

- SSRF check pins the validated IP at connect time (DNS-rebinding safe) and
  unwraps IPv4-embedded IPv6 (IPv4-mapped, NAT64, 6to4).
- Refresh provider subscriptions behind a per-profile single-flight lock.
- Apply `foreign_keys`/`busy_timeout`/`journal_mode` to every pooled SQLite
  connection via an after-connect hook, not once.
- Derive the client IP from a trusted reverse-proxy hop (`TRUSTED_PROXY_HOPS`),
  never a client-spoofable header.
- Public token lookup runs unconditionally and compares in constant time; rate
  limiting is keyed by client IP (not path) to throttle token enumeration.
- Parse untrusted/admin YAML via `yaml::parse_limited` (caps anchors/aliases
  *before* `serde_yaml` — billion-laughs — then depth/nodes).
- Management API is same-origin with no CORS layer (enforced in `build_router`);
  keep it so and verify `Origin` on state-changing requests.
- The converter **replaces** provider `rules` and `proxy-groups` (proxies are
  still appended), so the UI imports the provider's own rules/groups via separate
  live fetches — `GET /api/profiles/:id/provider-rules` (seeds the rule editor)
  and `POST /api/profiles/:id/import-provider-groups` (inserts provider groups as
  editable `custom_groups`; `parse_provider_group` maps name/type/proxies→members
  and the rest→options, skipping existing names / unsupported types). Discarded
  provider rules/groups aren't in `generated_cache`. Both freeze on import: a
  provider update never changes rules/groups unless re-imported.
  `GET /api/profiles/:id/proxies` surfaces provider proxies + custom group names
  from the last generated output for editor autocomplete.
- Node/group ordering: `profiles.node_order` / `group_order` (JSON name arrays,
  `NULL`=default) drive a unified manual order of all proxies / proxy-groups.
  `converter::reorder_by_name` reorders the assembled `proxies` / `proxy-groups`
  by them (unlisted/new entries fall to the end); both `generate` and
  `list_proxies` apply them (so the preview reflects a saved order before
  regeneration). `PUT /api/profiles/:id/node-order` and `.../group-order` persist
  them (shared `set_order`/`load_order` over an `OrderKind` column allowlist);
  the `NodesCard`/`GroupsCard` previews drag via `@dnd-kit` and submit the full
  name list. Every generation snapshots the output's proxy/group name order back
  into `node_order`/`group_order` (`persist_cache` → `snapshot_orders`), so a
  provider refresh keeps existing entries in place by name (info refreshed by
  name) and appends new ones at the end; a manual drag overwrites the column. The
  `RulesCard` preview is also `@dnd-kit`-sortable, but rule order is already
  semantic ordered text, so it reorders the lines and saves via the existing
  `PUT /api/profiles/:id/rules` — no `*_order` column.
- Order/rule edits apply to the served subscription **immediately**, without a
  provider re-fetch: the `node-order`/`group-order`/`rules` write handlers call
  `generate::resync_cache`, which re-stitches the existing
  `generated_cache.output_yaml` in place (reorder `proxies`/`proxy-groups` by the
  saved orders; rebuild the `rules` block from the ruleset) and keeps
  `generated_at` so the refetch cadence is unchanged. Best-effort — a failure
  just defers the change to the next full generate. (Adding a *new* node/group
  still needs a generate, since it isn't in the cached output yet.)
- Outbound fetches send `FETCH_USER_AGENT` (default `clash.meta/1.0`); many
  airport panels 403 a non-clash UA, so don't blank it.
- Keep `/health` minimal (no version). Admin creds from
  `ADMIN_USERNAME`/`ADMIN_PASSWORD`; management UI/API require login.

**Code layout:** library (`src/lib.rs`) + thin binary (`src/main.rs`) so tests
drive the app directly. `src/app.rs` holds `AppState` and `build_router` (single
source of route wiring); `main.rs` only loads env config, builds state, serves.
Per-feature modules: `db`, `auth`, `profiles`, `settings`, `ssrf`, `fetch`
(`SubscriptionFetcher` trait + `HttpFetcher`, injected into `AppState` so
generate/public paths test without network), `converter`, `generate`
(generate/preview + public endpoint), `single_flight`, `rate_limit`, `net`;
helpers `error` (API envelope + `sqlx` UNIQUE→409), `mask`, `yaml`, `util`. DB at
`${DATA_DIR:-/data}/mihomo-subscription.db`; migrations in `migrations/`. SPA in
`web/` (Vite + React + Ant Design + react-i18next) → `web/dist`, served by Axum
with an `index.html` fallback (`WEB_DIR`).

**Frontend editors** (`web/src/pages/detail/`): nodes/groups/rules each render a
read-only **preview** card (`NodesCard`/`GroupsCard`/`RulesCard`) with
**schema-driven structured forms** — admins never edit raw YAML.
`nodeSchema.ts`/`groupSchema.ts` declare per-type field sets (`showWhen` toggles
fields like REALITY/ws/grpc by transport/TLS); `fields.tsx` holds `FieldInput`
plus dotted-path `getPath`/`setPath` that prune empties (so `alpn: []` never
serializes). Model↔YAML conversion lives in each card. All copy goes through
`i18n.ts` keys (zh-only). Adding a field/rule type means extending the
schema/serializer **and** the i18n table — no free-text YAML escape hatch.

**Test pattern:** integration tests in `tests/` build the router via
`build_router` and drive it with `tower::util::ServiceExt::oneshot` against a
throwaway SQLite file (`tests/common::TempDb` / `test_state`). Pure logic (SSRF
ranges, masking, bounded YAML) is unit-tested inline. Converter and SSRF suites
are release gates.

## Conventions

- New technical docs are bilingual: Chinese first, English following, shared
  code blocks.
- The 1Panel package mirrors the official layout; each release adds a new version
  dir and keeps old ones. Images publish to Docker Hub as
  `quinlanhoo/mihomo-subscription:<version>` (multi-arch amd64+arm64); on-host
  build is the offline fallback. `logo.png` is a placeholder.
- 1Panel packaging: use `${CONTAINER_NAME}` and `PANEL_APP_PORT_HTTP`, join the
  external `1panel-network`, persist via `./data`, expose
  `ADMIN_USERNAME`/`ADMIN_PASSWORD` as install fields. The reverse proxy must
  preserve `Host` or the same-origin check yields CSRF `403`. Env-var table in
  `docs/1panel-app.md`.
- Container runtime: the image **starts as root** so `docker-entrypoint.sh` can
  `chown ${DATA_DIR}` (a `./data:/data` bind mount overrides the build-time
  `chown`, else SQLite `code 14 (CANTOPEN)`), then drops to unprivileged
  `appuser` via `gosu` before exec. Keep this drop — the business process must
  not run as root.
