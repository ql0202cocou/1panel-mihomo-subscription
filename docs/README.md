# Documentation

This directory stores project documentation, packaging notes, and release
materials for Mihomo Subscription Manager.

These are the maintenance-phase reference docs for the implemented service.
(The development-phase planning docs `plan.md` and `technical-roadmap.md` were
removed once the design was implemented; their durable content was folded into
the docs below — the environment-variable table into `1panel-app.md` and the
converter's top-level-key handling into `api-design.md`.)

## Documents

- `api-design.md`: API request/response contracts, authentication behavior, and
  the converter's top-level-key handling.
- `data-model.md`: SQLite schema, indexes, and migration notes.
- `security-design.md`: Security goals, public link design, SSRF protection,
  authentication, and abuse-control notes.
- `1panel-app.md`: 1Panel local app packaging, the authoritative environment
  variable table, and validation notes.
- `release.md`: Image build, tag, and publish steps.
- `changelog.md`: Changelog template and notable project changes.

The documented design is implemented (backend + SPA). Remaining before a formal
release: updating the 1Panel app package install form (`1panel-app.md`) and
cutting the release (`release.md`). Implementation trade-offs are tracked in
`changelog.md`.
