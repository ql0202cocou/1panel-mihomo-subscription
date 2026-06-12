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
