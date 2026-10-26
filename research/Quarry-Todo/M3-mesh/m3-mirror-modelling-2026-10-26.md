---
id: "m3-mirror-modelling-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T00:50:00.000Z"
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
- [x] Turn on X mirror, move a +X vertex: its −X twin matches.
- [ ] Extrude on one side mirrors on the other.
- [x] Apply leaves one mesh, no duplicate centre vertices.

## Progress 2026-10-26

- `mesh.mirror_mode(x|y|z|off)` per mesh (saved); `mesh.transform` moves each moving vertex's twin and keeps on-axis vertices on the axis; `mesh.mirror_apply` keeps the + side, mirrors it, shares centre vertices, reverses mirrored winding, turns mirror off.
- Tested: moving a +X vertex moves its -X twin to match; apply on a folded box gives 12 vertices with 4 shared on the axis.
- Left: extrude on one side creating the mirrored geometry on the other (needs extrude to run on twins too).
