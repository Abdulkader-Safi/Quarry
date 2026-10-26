---
id: "m5-export-dedup-2026-10-26"
status: "done"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T03:00:00.000Z"
labels: ["m5", "gltf", "test"]
order: 47
---

# M5 · Vertex dedup, goldens and validator in CI

FR-EXP-05 and NFR-07. The exporter must not be bloated like the tool this replaces.

**Build**
- Dedup vertices on (position, normal, UV, colour, bone weights): a 6-face box exports 24 vertices, not 36.
- `tests/goldens/` with reference .glb files and a byte/semantic compare; `xtask validate-gltf` runs the Khronos validator (npm, CI only).
- CI job fails on validator errors or an unexpected golden diff; golden updates are an explicit command.

**Done when**
- [x] Box golden is ≤ 24 vertices.
- [x] Validator runs in CI on the export goldens and passes.
- [x] A deliberate geometry change shows as a golden diff, not a silent overwrite.

## Done 2026-10-26

- Vertices are shared when position, normal, UV and colour match (untextured faces carry no UVs at all). A 6-face box exports 24 vertices.
- Goldens: `tests/goldens/box.glb`, `room.glb` (tiles, collision floor, marker child, crate and an instance) and `skin-bar.glb`, compared byte for byte; `QUARRY_UPDATE_GOLDENS=1` is the explicit update. Removing unused UVs showed as a golden diff on both files before the update.
- Khronos validator 2.0.0-dev.3.10 on all three: 0 errors, 0 warnings, 0 infos. CI runs it on every golden.
