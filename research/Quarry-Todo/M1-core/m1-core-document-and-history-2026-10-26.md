---
id: "m1-core-document-and-history-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "core"]
order: 9
---

# M1 · Document model, IDs and undo history

The data model from build spec §5 and the history that makes every edit reversible.

**Build**
- `quarry-core`: `Project`, `Object`, `Mesh`, `Face`, `Image`, `Palette`, `Skeleton`/`Bone`/`IkChain`, `Clip`/`Track`/`Key`, `ExportPreset` exactly as §5.
- IDs: u32 slot + generation, written as short strings; every object, bone, clip, marker and image also has a unique name; lookup accepts name or ID.
- Defaults: nearest filtering, 16 px tiles, 1 tile = 1 m, fps 30, grid 1.0, palette lock on.
- History: 200 entries or 512 MB, inverse command or snapshot, source label (`Human` / `Agent`), display label such as `Agent: extrude`. Undo, redo, cancel.

**Done when**
- [ ] Serde round-trip test of the whole §5 model, including weights and keys.
- [ ] 200 random mutations then undo-all equals the start state; the memory cap is enforced.
- [ ] No GPUI dependency in `quarry-core`.
