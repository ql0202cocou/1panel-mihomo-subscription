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

## [0.1.3] - 2026-06-14

### Fixed

- Fixed the container failing to start with `unable to open database file`
  (SQLite code 14) on 1Panel / any `./data:/data` bind mount. The image ran as
  the unprivileged `appuser`, but a bind mount overrides the build-time
  `chown appuser /data` with the host directory's (usually root-owned)
  ownership, so the process could not create the SQLite file. The container now
  starts as root, a new `docker-entrypoint.sh` `chown`s `DATA_DIR` and re-execs
  the app as `appuser` via `gosu` (added to the runtime image), preserving the
  least-privilege runtime while making bind-mounted data directories work
  out of the box. Added the `apps/mihomo-subscription/0.1.3/` package
  (image `quinlanhoo/mihomo-subscription:0.1.3`) and bumped
  `Cargo.toml`/`Cargo.lock` to `0.1.3`.

### Documentation

- Added a "Create the GitHub Release" step to `docs/release.md` (`gh release
  create vX.Y.Z --verify-tag`, notes drawn from the changelog version section)
  and a post-release checklist item confirming the Release is published — the
  release flow previously stopped at the git tag.

## [0.1.2] - 2026-06-14

### Added

- Completed the 1Panel app package and cut release `0.1.2`. The new
  `apps/mihomo-subscription/0.1.2/` package exposes the full install form —
  `ADMIN_USERNAME`/`ADMIN_PASSWORD`, `PUBLIC_BASE_URL`, `PUBLIC_PATH_PREFIX`,
  `RUST_LOG`, `FETCH_TIMEOUT_SECONDS`, `MAX_SUBSCRIPTION_SIZE_MB`,
  `CACHE_TTL_MINUTES`, `TRUSTED_PROXY_HOPS`, and a `SECURE_COOKIES`
  `auto`/`true`/`false` selector — and its `docker-compose.yml` passes every
  one through to the container (image `mihomo-subscription:0.1.2`). Bumped
  `Cargo.toml`/`Cargo.lock` to `0.1.2`, pointed the README/CLAUDE/AGENTS build
  commands and the CI 1Panel-YAML gate at the new package, and cleared the
  "package update pending" markers in `docs/1panel-app.md`. The incomplete
  `0.1.0/` directory is retained for history.
- New `SECURE_COOKIES` environment variable to force the `Secure` session-cookie
  attribute. It defaults to inferring from an `https://` `PUBLIC_BASE_URL`, so
  behind a TLS-terminating reverse proxy (where the app speaks plain HTTP and
  `PUBLIC_BASE_URL` may be unset or `http`) the operator can now opt in
  explicitly. The service also logs a startup warning whenever session cookies
  end up without `Secure` (see `src/main.rs`, `docs/technical-roadmap.md` env
  table).

### Changed

- Switched the 1Panel image strategy from on-host local builds to a Docker Hub
  image. `apps/mihomo-subscription/0.1.2/docker-compose.yml` now references
  `quinlanhoo/mihomo-subscription:0.1.2` (multi-arch amd64+arm64) so the 1Panel
  host pulls the image at install instead of syncing source and building locally.
- Removed the unused `tokio-cron-scheduler` dependency (no scheduler is wired
  in `src/`), trimming the dependency graph and supply-chain surface.
- Upgraded `sqlx` 0.7 → 0.8 and switched its feature set from
  `runtime-tokio-rustls` to `runtime-tokio` (SQLite needs no TLS). This fixes
  RUSTSEC-2024-0363 and drops the unused rustls stack, clearing three
  `rustls-webpki` advisories and the `rustls-pemfile`/`paste` unmaintained
  warnings. No code changes were required.
- Replaced the fixed-window rate limiter with a token bucket (`src/rate_limit.rs`).
  Same `max`/`window` knobs, but tokens refill continuously, removing the ~2x
  burst a fixed window allows across its boundary while still permitting a
  legitimate burst up to `max`.

### Fixed

### Security

- Hardened session-cookie issuance: previously the `Secure` attribute was set
  only when `PUBLIC_BASE_URL` began with `https://`, so a deployment behind an
  HTTPS reverse proxy that left `PUBLIC_BASE_URL` unset would silently issue
  session cookies without `Secure` (exposing them to plaintext transmission).
  The new `SECURE_COOKIES` override plus startup warning close that gap.
- Added a `cargo audit` CI gate (`.github/workflows/ci.yml`) with documented,
  per-advisory ignores in `.cargo/audit.toml` (only `rsa` — pulled by the
  feature-gated, never-compiled `sqlx-mysql` driver, no upstream fix — and the
  informational `rustls-pemfile` unmaintained notice). New advisories now fail
  the build.
- Validate provider `source_url` at write time (profile create/update): reject
  non-http(s) schemes, embedded credentials, loopback hostnames, and blocked
  literal IPs up front with a generic `400`. This is defense in depth and a
  clearer error — the authoritative SSRF check still runs at fetch time with DNS
  resolution and IP pinning (`src/fetch.rs`).
- Sweep expired sessions when a new one is created (`src/auth.rs`), bounding the
  in-memory session map so abandoned/expired entries can no longer accumulate
  (creation is the only growth point).
- Audited the database-error log line (`src/error.rs`) and confirmed `sqlx`
  error Display never includes bound parameter values (only driver/constraint
  text), so provider URLs and tokens cannot leak through it; added a comment to
  keep it that way.

### Documentation

- Reworked `docs/release.md` to make multi-arch `docker buildx ... --push` to
  Docker Hub the primary build step (with a Personal Access Token login note and
  the required `docker-container` builder), and demoted on-host `docker build` to
  an offline/intranet fallback appendix. Updated the image-reference item in the
  `docs/1panel-app.md` validation checklist accordingly.
- Updated `README.md` again for the Docker Hub deployment: the "Deploy in 1Panel"
  section now pulls the published image and presents a hand-written Compose as
  the primary, copy-paste-ready path — a complete env block with every variable
  (required / fixed / optional grouped, only four values marked `← edit`) plus a
  healthcheck, with the 1Panel app-package install demoted to a one-line pointer.
  The status banner reflects the published `0.1.2` image and the complete 1Panel
  install form. Synced the image-strategy notes in `CLAUDE.md` and `AGENTS.md`.
- Rewrote `README.md` into a concise user-facing guide (~185 → ~109 lines): a
  short intro, a focused "Deploy in 1Panel" section (local-image build, Compose
  with the required env vars including `SECURE_COOKIES`, and the Host-header
  reverse-proxy requirement), and a brief usage walkthrough. The architecture
  diagram, capabilities list, and full development section were dropped in favor
  of pointers to `docs/`, `CLAUDE.md`, and `AGENTS.md`.
- Removed the development-phase planning docs `docs/plan.md` and
  `docs/technical-roadmap.md` now that the design is implemented. Their durable
  content was folded into the maintenance docs: the authoritative environment
  variable table moved to `docs/1panel-app.md`, and the converter's top-level
  key handling moved to `docs/api-design.md`. All cross-references in
  `README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/`, and `src/converter.rs` were
  repointed accordingly.
- Compressed `AGENTS.md` (~137 → ~77 lines): merged the command blocks and
  condensed the change-rule and security-default bullets without dropping any
  rule.
- Compressed `CLAUDE.md` (~147 → ~111 lines): dropped the implemented-files
  enumeration that duplicated the code-layout map and merged the command blocks,
  keeping the non-obvious implementation rules and module map intact.

## [0.1.1] - 2026-06-13

### Security

- Hardened against YAML alias-expansion ("billion laughs"): `src/yaml.rs` now
  counts `&anchor`/`*alias` tokens in the raw text and rejects documents over a
  small cap *before* `serde_yaml` parses them (the bomb is tiny and expands
  inside the parser, so the size/depth/node checks can't help). Applies to both
  admin node/group content and fetched provider YAML.
- Made public-download rate limiting throttle token enumeration: the limiter is
  now keyed by client IP only (not IP + path), so guessing many distinct tokens
  from one IP shares a single budget and `404`-generating scans are throttled.
- Removed the version number from the unauthenticated `/health` response (it
  now returns only `{"status":"ok"}`) to avoid version disclosure.

### Documentation

- Documented the reverse-proxy `Host` passthrough requirement in
  `docs/1panel-app.md` (the `Origin` check 403s state-changing requests if the
  proxy rewrites `Host`), and updated `docs/security-design.md` for the
  pre-parse anchor/alias cap and the per-IP download limit.
- Synced `AGENTS.md` with the current repo state and GitHub hosting: corrected
  the key-paths list (library/bin split, `web/`, `migrations/`, CI workflow),
  dropped the "target" wording now that the design is implemented, aligned the
  commands with the CI gates (`fmt --check`, `clippy -D warnings`, `test`) plus
  the frontend build, and added a "Repository & CI" section (GitHub `main`/PR
  flow, CI gates, release/tag steps).

## [0.1.0] - 2026-06-13

### Added

- Reworked the `Dockerfile` into a three-stage build: a `node:20-slim` stage
  builds the SPA (`web/dist`), the Rust stage compiles the binary (now copying
  `migrations/` so `sqlx::migrate!` can embed them), and the runtime image
  ships only the binary plus the built assets (served via `WEB_DIR`). Bumped
  the build image to `rust:1.90-slim` (a transitive dependency now requires
  edition2024). Expanded `.dockerignore`. Smoke-tested: `/health` OK, SPA
  served, unauthenticated `/api` returns `401`. The 1Panel app package update
  is intentionally deferred.
- Built the profile detail page and editors (frontend step 2): hosted-link
  header (copy, QR, reset-token with confirm, and a "modified but not
  generated" banner derived client-side from the latest sub-resource
  modification vs `last_generated_at`); six configuration cards (basic info,
  source with masked URL / last-fetch status / write-only URL replacement /
  manual refresh, custom nodes and groups CRUD, a CodeMirror rules editor, and
  output preview); and a generate footer that maps itemized `400` validation
  errors back to the editor — rule-line errors get a click-to-jump into the
  CodeMirror editor. Added `last_generated_at` to the profile API response
  (correlated subquery in `src/profiles.rs`) to drive the banner, aligning with
  `docs/api-design.md`. New web deps: `@uiw/react-codemirror`,
  `@codemirror/lang-yaml`, `@codemirror/state`.
- Scaffolded the `web/` SPA (Vite + React + TypeScript + Ant Design +
  react-i18next): routes for `/login`, `/` (profile list), `/profiles/:id`
  (skeleton detail with hosted-link copy + QR), and `/settings` (public-path
  reset with typed confirmation); a fetch client whose `401` handler clears the
  session so `RequireAuth` redirects to `/login` preserving the return path;
  the Vite dev server proxies `/api` and `/health` to the backend. Added a
  frontend CI job (`npm ci` + `npm run build`). The full configuration cards
  and editors come in the next step.
- Implemented client-IP derivation and rate limiting: `src/net.rs` derives the
  client IP from `X-Forwarded-For` counting `TRUSTED_PROXY_HOPS` from the right
  (spoofed left entries ignored; falls back to the TCP peer), fully unit-tested;
  `src/rate_limit.rs` adds an in-memory fixed-window limiter plus login
  (per-IP) and public-download (per-IP+path) middleware. `main` serves with
  connect-info so the TCP peer is available, reads `TRUSTED_PROXY_HOPS`, and
  configures the limiters. This also supplies the login-failure rate limiting
  deferred from the auth step. Per-profile refresh limiting is provided
  structurally by the single-flight lock plus cache TTL.
- Implemented generation, preview, and the public subscription endpoint
  (`src/generate.rs`): `generate` (and the source-card manual refresh) fetches
  via the injected `SubscriptionFetcher`, converts, persists `generated_cache`,
  and updates `last_fetch_*`; `preview` is read-only (no cache write, no
  `last_fetch_*` change); the public endpoint serves fresh cache, refreshes
  under a per-profile single-flight lock (`src/single_flight.rs`), falls back
  to stale cache on refresh failure, returns a generic `503` when no cache
  exists and the fetch fails, and a uniform `404` (constant-time prefix
  compare, always-run token lookup) for wrong prefix / unknown token /
  disabled profile. Adds the documented response headers
  (`subscription-userinfo` passthrough, `profile-update-interval`,
  `content-disposition`). The fetch is abstracted behind `SubscriptionFetcher`
  (real `HttpFetcher` in production) so the paths are tested without network;
  `tests/generate.rs` covers cache-hit, single-flight coalescing, stale
  fallback, `503`, and uniform `404`. New env wiring: `FETCH_TIMEOUT_SECONDS`,
  `MAX_SUBSCRIPTION_SIZE_MB`, `CACHE_TTL_MINUTES`.
- Implemented the `mihomo`/`clash` -> `mihomo` converter (MVP release gate):
  `src/converter.rs` parses provider YAML (bounded), appends enabled custom
  nodes/groups, replaces `rules`, strips `proxy-providers`, and passes through
  `rule-providers` and all other top-level keys. Generate-time validation
  returns an itemized error list: custom-group/provider-group name collisions,
  custom-node/provider-proxy collisions, rule policy targets and group members
  that don't resolve to a known proxy/group/built-in. Logical/nested rules
  (with parentheses) pass through without target validation. Nine fixture
  unit tests cover append/replace/passthrough/strip/collision/dangling-ref.
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

- Updated `CLAUDE.md` for the frontend: added the `web/` npm commands
  (dev/build/typecheck) and completed the Code-layout module list
  (`converter`, `generate`, `single_flight`, `rate_limit`, `net`, the
  injectable `SubscriptionFetcher`) plus the SPA serving note.
- Flipped the project status from planning to implemented: removed the
  "状态:规划阶段 / Status: planning" banners from `api-design.md` and
  `data-model.md` (now implemented), reworded the `release.md` and
  `1panel-app.md` banners to "not yet released / package update pending",
  refreshed the status sections in `CLAUDE.md`, `AGENTS.md`, the root
  `README.md`, and `docs/README.md`, and removed the obsolete prototype-route
  compatibility note. The changelog version roll to a dated `0.1.0` is
  intentionally deferred until the 1Panel app package is updated and a release
  is actually cut.
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
