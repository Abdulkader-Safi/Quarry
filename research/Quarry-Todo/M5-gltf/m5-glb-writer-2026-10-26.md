---
id: "m5-glb-writer-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m5", "gltf", "export"]
order: 46
---

# M5 · Self-contained GLB writer

FR-EXP-01, FR-EXP-03, FR-EXP-04. Builds on the M0 spike writer.

**Build**
- `gltf-json` 1.4.1 + own GLB writer: one .glb with meshes, materials, base-colour PNG, nodes.
- Sampler magFilter 9728 (NEAREST) when the project uses nearest filtering.
- Node names plus `extras.quarry` for markers (type, properties) and collision flags.
- Selection and scale options; separate `.gltf` + `.bin` + PNG option for debugging only.
- `export.run` is long-running, returns the validator summary.

**Done when**
- [ ] Export the sample robot: exactly one .glb is written.
- [ ] Zero Khronos validator errors; opens in Blender with mesh, texture, all clips.
- [ ] A flagged floor node carries `extras.quarry.collision = true`; a marker keeps its name and props.
