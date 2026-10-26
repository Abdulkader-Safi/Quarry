---
id: "m3-primitives-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m3", "mesh"]
order: 30
---

# M3 · Primitives at the crosshair

FR-MESH-01. The fastest way for a human or an agent to start geometry.

**Build**
- `mesh.add_primitive`: box, wedge, cylinder, cone, sphere, plane, with segment counts and size.
- Built at the crosshair, oriented to the active plane, one object with correct per-face UVs (default texture or blank).
- UVs laid out per primitive so texturing works immediately.

**Done when**
- [ ] An 8-segment cylinder has 8 side quads and two caps.
- [ ] Every primitive has sane UVs, no NaNs, no inverted normals.
- [ ] Each add is one undo step.
