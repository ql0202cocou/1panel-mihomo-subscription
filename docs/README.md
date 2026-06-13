# Documentation

This directory stores project documentation, packaging notes, and release
materials for Mihomo Subscription Manager.

## Documents

- `1panel-app.md`: 1Panel local app packaging and validation notes.
- `plan.md`: Initial product plan and user-facing requirements.
- `technical-roadmap.md`: Recommended architecture and implementation roadmap.
- `api-design.md`: API request/response contracts and authentication behavior.
- `data-model.md`: SQLite schema, indexes, and migration notes.
- `security-design.md`: Security goals, public link design, SSRF protection,
  authentication, and abuse-control notes.
- `release.md`: Image build, tag, and publish steps.
- `changelog.md`: Changelog template and notable project changes.

The documented design is implemented (backend + SPA). Remaining before a formal
release: updating the 1Panel app package install form (`1panel-app.md`) and
cutting the release (`release.md`). Implementation trade-offs are tracked in
`changelog.md`.
