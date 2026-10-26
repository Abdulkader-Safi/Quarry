# Progress

Spec: `docs/spec/06-build-spec.md`. Board: `research/Quarry-Todo/`.

Building milestone by milestone per spec §13. Don't start the next milestone with a failing box.

## Milestones

| M | Scope | State | Notes |
|---|-------|-------|-------|
| M0 | De-risk: workspace, CI, viewport, pen, skim, PSD | in progress | |
| M1 | Core, shell, command server | not started | |
| M2 | Tiles and linked images | not started | |
| M3 | Mesh editing, UVs, scene | not started | |
| M4 | Painting | not started | |
| M5 | glTF export and import | not started | |
| M6 | Rigging | not started | |
| M7 | Animation | not started | |
| M8 | Agent experience | not started | |
| M9 | Hardening and release | not started | |

## M0 measurements

| Check | Target | macOS (M1, 8 GB) | Windows | Linux |
|---|---|---|---|---|
| Viewport fps, 100k tri / 50 bones / 5 skinned @1600x1000 | 60 / ≥30 | | | |
| Input latency | < 50 ms | | | |
| Pen pressure | values arrive | | | |
| PSD composite | matches PNG | | | |
| 3-bone GLB in Cairn | bends, 0 validator errors | | | |

## Decisions and deviations

- 2026-10-26: Spec pinned to `docs/spec/` (build spec §4). `research/spec/` keeps the reference screenshots only.
- 2026-10-26: Every crate pin in §2 checked against crates.io; all exact versions exist. Nothing upgraded.
