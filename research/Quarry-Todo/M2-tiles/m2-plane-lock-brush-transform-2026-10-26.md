---
id: "m2-plane-lock-brush-transform-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m2", "tiles"]
order: 24
---

# M2 · Plane lock, tilt and brush transforms

FR-TILE-04 and FR-TILE-05. Tiles go on walls and floors, not just one plane.

**Build**
- `tile.plane`: lock to X, Y or Z and tilt by degrees; the crosshair visual shows the active plane.
- `tile.brush`: rotate in 90° steps, flip horizontally/vertically, mirror — applied to UVs, not the texture.
- Placement respects the locked plane orientation, including normal direction.

**Done when**
- [ ] Lock to X, place tiles: all normals point along X.
- [ ] Rotate 90 and place: UVs are rotated 90 against the unrotated tile.
- [ ] Tilt 45 on X and place: quads follow the tilt with no gaps at the shared edge.
