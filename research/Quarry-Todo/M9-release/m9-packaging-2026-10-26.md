---
id: "m9-packaging-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T11:40:00.000Z"
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

## Progress 2026-10-27

- `Packager.toml` (cargo-packager 0.11.8): Quarry with `quarry` as the main binary and `quarry-mcp` beside it, built by `cargo build --release` first. `cargo packager --release --formats app` builds `target/packages/Quarry.app` here; the scripted agent session passes against the packaged `quarry-mcp` (2.2 s), including the refused write outside the project folder.
- `.github/workflows/release.yml`: on a `v*` tag, builds `.app`/`.dmg` on macos-14 (Apple Silicon) and macos-13 (Intel), NSIS on Windows, AppImage on Ubuntu 22.04, and attaches them to a GitHub release. Not run yet: pushing a tag publishes a release, so that waits for Safi.
- README: install a release, unsigned-app steps per OS, building installers, all xtask commands, current status.
- `cargo deny check` passes; every dependency is on the MIT-compatible allow list.
- Left: push a tag, check the four artifacts install and launch on each OS.
