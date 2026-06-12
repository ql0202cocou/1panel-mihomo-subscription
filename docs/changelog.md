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
