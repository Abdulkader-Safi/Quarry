---
id: "m3-mesh-cleanup-2026-10-26"
status: "done"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T00:50:00.000Z"
labels: ["m3", "mesh"]
order: 34
---

# M3 · Merge, split, delete, make face, flip

FR-MESH-05 and FR-MESH-06.

**Build**
- Merge vertices: at centre, at crosshair, by distance (with a threshold).
- Split faces/edges, delete vertices/faces with or without a hole.
- `mesh.make_face` from 3 or 4 selected vertices; flip normals; flip quad diagonal.
- Every op keeps UVs and weights sane and is one undo entry.

**Done when**
- [x] Merge by distance 0.001 on two coincident vertices leaves one.
- [x] Select 4 vertices, make face: one quad facing the camera.
- [x] Flip a quad diagonal: triangulation changes, the quad outline does not.

## Done 2026-10-26

- `mesh.merge(at: center|cursor|distance, distance?)`, `mesh.split`, `mesh.delete(vertices|faces|only_faces)`, `mesh.make_face` (3 or 4 vertices, ordered around their centre, facing the camera), `mesh.flip_normals`, `mesh.flip_diagonal`. Collapsed faces are removed; orphan vertices dropped; weights follow vertices.
- Tests (`crates/quarry-core/tests/mesh.rs`): split then merge by distance 0.001 removes the 4 doubles; 4 vertices make one quad facing the top camera; flip diagonal keeps the corner set; deleting a box corner removes 3 faces.
