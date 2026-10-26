---
id: "m3-mesh-cleanup-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Merge by distance 0.001 on two coincident vertices leaves one.
- [ ] Select 4 vertices, make face: one quad facing the camera.
- [ ] Flip a quad diagonal: triangulation changes, the quad outline does not.
