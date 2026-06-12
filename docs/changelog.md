# Changelog

All notable changes to this project should be documented in this file.

Use reverse chronological order. Keep entries concise, user-facing when
possible, and grouped by change type.

## Maintenance Rules

- Never delete old version entries.
- Add new work under `[Unreleased]`.
- Keep affected project documents updated with every change so documentation and
  implementation/design stay aligned.
- When releasing, rename the current `[Unreleased]` section to the released
  version and date, then create a new empty `[Unreleased]` section above it.
- Keep newer versions above older versions.
- Preserve historical entries even if later releases change or supersede them.

## Template

```markdown
## [Unreleased]

### Added

- 

### Changed

- 

### Fixed

- 

### Security

- 

### Documentation

- 

## [0.1.0] - YYYY-MM-DD

### Added

- First released changes.
```

## [Unreleased]

### Added

- Refreshed `CLAUDE.md` to match the active implementation: status now lists
  what is built vs pending (no longer "planning stage"), commands cover the CI
  gates and single-test invocation, and architecture describes the lib/bin
  split, the per-feature module layout, and the `ServiceExt`/`TempDb` test
  pattern (replacing the obsolete `src/main.rs` prototype description).
- Implemented the SSRF-protected provider fetch (MVP release gate): `src/ssrf.rs`
  with network-free, table-tested URL/IP validation covering every blocked
  IPv4/IPv6 range plus the IPv4-mapped/NAT64/6to4 unwrap bypasses; `src/fetch.rs`
  performing per-hop validation, host resolution with validated-IP pinning
  (DNS-rebinding safe), manual redirect re-validation (max 3), connect/total
  timeouts, a streamed response-size cap (not `Content-Length`), binary
  content-type rejection, and `subscription-userinfo` sanitization. `FetchError`
  maps to `last_fetch_status` labels for reuse by the generate step.
- Finalized the integration-test baseline and CI (Skeleton step 4): added
  `.github/workflows/ci.yml` running `cargo fmt --check`, `cargo clippy
  --all-targets -D warnings`, `cargo test`, and the 1Panel app-package YAML
  validation; deduplicated `tests/db_cascade.rs` onto the shared
  `tests/common` helpers. The `ServiceExt`-based auth and profile suites
  (21 tests) stand as the regression baseline.
- Implemented profile CRUD and sub-resources (Skeleton step 3): profiles
  (create/list/detail/update/delete) plus rules (replace), custom nodes and
  groups (CRUD), reset-token, settings read, and reset-public-path, all under
  session auth. Provider URLs are write-only and masked deterministically
  (`src/mask.rs`); the hosted link is assembled from the live public path
  prefix (now an `RwLock` in `AppState`, updated by reset-public-path) plus the
  per-profile token. Added `src/error.rs` (error envelope, UNIQUE→409 mapping),
  `src/yaml.rs` (depth/node-count-bounded parsing for admin node content),
  `src/util.rs` (timestamps, random token/prefix), `src/profiles.rs`,
  `src/settings.rs`, and `tests/profiles.rs`. Conversion endpoints
  (generate/preview/public) remain for a later step.
- Implemented session authentication and same-origin static serving
  (Skeleton step 2): `src/auth.rs` with constant-time credential verification
  (digest-based, no length leak), an in-memory session store (256-bit IDs,
  7-day idle expiry), login/logout/session handlers, a `require_session`
  middleware (`401` otherwise), and an `Origin` check on state-changing
  requests; `src/app.rs` assembling the router with no CORS layer, a 1 MB
  body limit, and an SPA `ServeDir` fallback; `main.rs` now refuses to start
  without `ADMIN_USERNAME`/`ADMIN_PASSWORD` and enables `Secure` cookies under
  an HTTPS `PUBLIC_BASE_URL`. Added `tests/auth.rs` and a shared
  `tests/common` helper. Login-failure rate limiting is deferred to the
  rate-limit task.
- Began implementing the documented design (Skeleton step 1): added
  `migrations/0001_init.sql` creating the target schema and dropping the
  prototype `subscriptions` table; added a `src/db.rs` module that opens the
  SQLite pool with per-connection `foreign_keys`/`busy_timeout`/WAL pragmas,
  runs migrations, and seeds the `app_settings` public path prefix; added a
  `src/lib.rs` so integration tests can use the crate; added
  `tests/db_cascade.rs` proving profile deletion cascades to all child tables
  (and the foreign-keys pragma holds across pooled connections).
- Initialized project documentation under `docs`.
- Added 1Panel app packaging notes.
- Added technical roadmap for the Mihomo subscription conversion service.
- Added security design covering public links, admin authentication, SSRF
  protection, sensitive data handling, and caching.
- Added product plan covering MVP scope, custom rules, custom nodes, custom
  proxy groups, permanent links, and 1Panel deployment expectations.
- Added `AGENTS.md` for future coding-agent handoff.

### Changed

- Clarified that permanent public subscription links should use both a random
  public path prefix and per-profile token.
- Expanded the planned product scope from subscription URL CRUD to profile-based
  Mihomo subscription conversion and distribution.

### Fixed

- Updated the Axum service startup code for Axum 0.7 compatibility.
- Installed `wget` in the runtime image so health checks can run.

### Security

- Documented SSRF protection requirements for provider subscription fetching.
- Closed a second round of design-review gaps in concurrency, deployment
  topology, and storage correctness: a per-profile single-flight lock to
  prevent stale-cache refresh stampedes; correct client-IP derivation behind
  the 1Panel reverse proxy via `TRUSTED_PROXY_HOPS` (added to the environment
  variable table); always-perform, constant-time public token lookup to avoid
  timing disclosure of the path prefix; management request body size limits
  (`413`) with the same YAML parse limits for admin-submitted content; and
  per-connection SQLite pragmas (`foreign_keys`, `busy_timeout`) applied via
  an after-connect hook so `ON DELETE CASCADE` is not silently disabled.
- Extended the testing strategy with cascade-delete, `503`, `413`, and
  single-flight concurrency cases.

### Documentation

- Added a "Non-obvious implementation rules" section to `CLAUDE.md`
  summarizing the cross-cutting SSRF, single-flight, SQLite pool, client-IP,
  timing, and CORS requirements for future implementing instances.
- Hardened the SSRF design after a security review: blocked IPv4-embedding
  IPv6 ranges (IPv4-mapped, NAT64, 6to4) with embedded-address re-checking,
  required pinning of validated IPs against DNS rebinding, required the
  response size limit to count streamed bytes instead of `Content-Length`,
  and added TEST-NET/6to4-relay IPv4 ranges.
- Added an untrusted content handling section: YAML alias/nesting parse
  limits, `subscription-userinfo` format validation before storage or echo,
  and escaping of provider-supplied names in the Web UI.
- Strengthened the auth design: constant-time credential comparison, minimum
  session-ID entropy, a same-origin/no-CORS policy for the management API
  (the prototype's permissive CORS layer must be removed when auth lands),
  and `Origin` verification as CSRF defense in depth.
- Documented masking requirements for original provider subscription URLs.
- Documented administrator login requirements and 1Panel compose-based
  credential configuration.

### Documentation

- Added this changelog template and initial unreleased entries.
- Simplified `AGENTS.md` into a concise handoff guide.
- Updated `AGENTS.md` with login credential and 1Panel environment guidance.
- Added documentation maintenance guidance requiring affected project docs to
  stay aligned with each change.
- Documented the planned Web UI structure: hosted link header, Mihomo
  configuration cards, and generate-link modal.
- Updated product, security, technical, and 1Panel docs for the login management
  page requirement.
- Added `docs/api-design.md` defining the target management API, authentication
  flow, validation rules, and public subscription endpoint contract (bilingual).
- Added `docs/data-model.md` defining the target SQLite schema, indexes, and
  migration strategy (bilingual).
- Added `docs/release.md` defining versioning, pre-release checks, image build,
  1Panel app package update, and changelog roll steps (bilingual).
- Added a root `README.md` with project status, planned capabilities,
  architecture overview, and documentation index (bilingual).
- Updated `docs/README.md` to move the planned documents into the published
  document list.
- Added `CLAUDE.md` with Claude Code guidance: planning-stage status, commands,
  target architecture summary, and documentation conventions.
- Added a change rule requiring `CLAUDE.md` and `AGENTS.md` to be reviewed and
  updated after every change so agent guidance stays aligned.
- Added the MIT `LICENSE`, declared `license = "MIT"` in `Cargo.toml`, and
  added a License section to the root `README.md`.
- Decided on a local-image strategy: the compose image is now
  `mihomo-subscription:0.1.0` (built on the 1Panel host, no remote registry);
  reworked `docs/release.md` accordingly with an optional push appendix.
- Added a generated placeholder `apps/mihomo-subscription/logo.png` (180x180);
  to be replaced with a real design before public distribution.
- Updated `docs/1panel-app.md`, `AGENTS.md`, and `CLAUDE.md` for the local
  image name and logo status.
- Added a planning-status banner to `docs/1panel-app.md` and marked
  not-yet-satisfied validation checklist items as pending, fixing the mismatch
  with the actual app package contents.
- Added an authoritative environment variable table to
  `docs/technical-roadmap.md`, including the previously undefined
  `CACHE_TTL_MINUTES`, and aligned the cache TTL wording in
  `docs/security-design.md` and `docs/data-model.md` with it.
- Documented the frontend build pipeline in `docs/technical-roadmap.md`:
  `web/` directory layout, Vite dev proxy, Axum static serving with SPA
  fallback, and a Node Docker build stage.
- Documented converter top-level key handling in `docs/technical-roadmap.md`:
  passthrough by default, `proxy-providers` stripped in the MVP for SSRF and
  URL-exposure reasons.
- Added a testing strategy to `docs/technical-roadmap.md` with converter and
  SSRF validator suites as hard gates for the MVP release.
- Documented client compatibility behavior: `subscription-userinfo`
  passthrough (stored with the generated cache; new column in
  `docs/data-model.md`), `profile-update-interval`, and `content-disposition`
  headers in `docs/api-design.md` and `docs/plan.md`.
- Defined the remaining API edge semantics in `docs/api-design.md` ahead of
  implementation: the source card's manual refresh reuses the generate
  endpoint, preview is read-only (fresh cache or live fetch, never persisted),
  the public endpoint returns stale cache on refresh failure or a generic
  `503` when no cache exists, and request body shapes for custom nodes and
  groups.
- Specified session storage (in-memory, 7-day idle expiry) and a
  deterministic URL masking rule (mask every query parameter value) in
  `docs/security-design.md`, and aligned its error handling section with the
  public endpoint `503` behavior.
- Documented the Web UI interaction design in `docs/plan.md`: page routes and
  two-level information architecture (list / detail / settings), a profile
  state model with a "modified but not generated" banner, danger-level
  separation of token vs public path resets, write-only provider URL editing,
  subscription link QR codes, provider fetch status observability with manual
  refresh, and session-expiry redirect behavior.
- Added UI implementation choices to `docs/technical-roadmap.md`: Ant Design,
  CodeMirror 6 with validation-error line mapping, `qrcode.react`,
  `react-i18next` from day one, and editor draft persistence rules.
- Added provider fetch observability fields (`last_fetch_at`,
  `last_fetch_status`) to `docs/data-model.md` and `docs/api-design.md`.
- Forbade persisting provider subscription URLs in browser storage in
  `docs/security-design.md`.
- Reconciled stale sections of `docs/technical-roadmap.md` with the
  authoritative docs: the data model sketch (per-profile `public_path` and an
  outdated link format) now defers to `docs/data-model.md`, the endpoint
  sketch defers to `docs/api-design.md`, and the architecture diagram shows
  the public path prefix.
