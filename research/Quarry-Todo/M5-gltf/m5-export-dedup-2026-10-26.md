---
id: "m5-export-dedup-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Box golden is ≤ 24 vertices.
- [ ] Validator runs in CI on the export goldens and passes.
- [ ] A deliberate geometry change shows as a golden diff, not a silent overwrite.
