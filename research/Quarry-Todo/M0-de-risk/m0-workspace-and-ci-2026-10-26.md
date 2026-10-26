---
id: "m0-workspace-and-ci-2026-10-26"
status: "done"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T16:15:00.000Z"
labels: ["m0", "infra"]
order: 2
---

# M0 · Cargo workspace, licence guard and CI

Prove the repo can be built and policed on all three OSes before writing features.

**Build**
- Cargo workspace, exact pins from build spec §2: `quarry-core`, `quarry-ops`, `quarry-render`, `quarry-io`, `quarry-server`, `quarry-mcp`, `quarry` (app), plus `xtask`.
- Empty shells with the dependency graph from §3: nothing below the app crate may depend on GPUI.
- `deny.toml` with the allow list in §3 principle 8 (MIT, Apache-2.0, BSD, ISC, Zlib, 0BSD, MPL-2.0, OFL-1.1 …). Never patch an MPL crate in place.
- `rustfmt.toml`, clippy config (`-D warnings`), GitHub Actions matrix on `macos-14`, `windows-latest`, `ubuntu-22.04`: fmt, clippy, test, cargo-deny.
- Spec lives in `docs/spec/` (`01-product` … `06-build-spec`); add `PROGRESS.md` with the milestone list and an empty measurements table.

**Done when**
- [x] `cargo check --workspace` is green in all three CI jobs.
- [x] `cargo deny check` passes with the allow list.
- [x] `PROGRESS.md` exists and lists M0–M9.
- [x] First commit is `M0: workspace and CI`.

**Progress 2026-10-26**
- Workspace, seven crates, `xtask`, `deny.toml`, CI matrix, `docs/spec/` and `PROGRESS.md` are in place.
- `cargo fmt`, `clippy -D warnings`, `test` and `cargo deny check` are green locally.
- Remaining: the first CI run on all three OSes.
- 2026-10-26: CI green on macos-14, windows-latest and ubuntu-22.04 (run 40320134647) after installing GPUI's Linux libraries. A gltf-validator job runs the goldens through the Khronos validator.
