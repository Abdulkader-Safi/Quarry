---
id: "m9-packaging-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m9", "release", "infra"]
order: 78
---

# M9 · Packaging, CI release and README

NFR-05 and NFR-09.

**Do**
- `cargo-packager` 0.11.8 (or a pinned version at least two weeks old): `.app`/`.dmg` for aarch64 and x86_64, NSIS `.exe`, AppImage.
- CI builds installers on a tag for all three OSes; unsigned is expected for v1.
- README: what Quarry is, build from source, unsigned-app open instructions per OS, Connect Claude, licence.
- Confirm `cargo deny` still passes and every dependency is MIT-compatible so the repo can go MIT.

**Done when**
- [ ] A tagged build produces all four artifacts in CI.
- [ ] Each installs and launches on macOS, Windows 10/11 and Ubuntu 22.04.
- [ ] README lets a stranger build and run without asking questions.
