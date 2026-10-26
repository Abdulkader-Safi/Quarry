---
id: "m3-mirror-modelling-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m3", "mesh"]
order: 35
---

# M3 · Live mirror modelling

FR-MESH-08.

**Build**
- Mirror axis per mesh (X default), live: moving or adding a vertex on +X updates its twin on −X.
- Handles vertices exactly on the axis (welded, not duplicated).
- `mesh.mirror_apply` bakes the mirror into the mesh and turns the mode off, as one undo entry.

**Done when**
- [ ] Turn on X mirror, move a +X vertex: its −X twin matches.
- [ ] Extrude on one side mirrors on the other.
- [ ] Apply leaves one mesh, no duplicate centre vertices.
