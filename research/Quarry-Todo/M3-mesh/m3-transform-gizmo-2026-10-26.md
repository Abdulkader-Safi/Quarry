---
id: "m3-transform-gizmo-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m3", "mesh", "ui"]
order: 32
---

# M3 · Transform gizmo, typed values and snapping

FR-MESH-03, using `transform-gizmo` 0.11.0 core.

**Build**
- Viewport gizmo for move, rotate, scale over the current selection (vertices, edges, faces, objects).
- Typed numeric entry for delta and absolute values; local/world space switch.
- Snap to grid, to vertex, and increments for rotate/scale; snaps reported in the status bar.
- Pivot: median, crosshair, individual origins.

**Done when**
- [ ] Type a 0.5 move on Y: the selection centre rises exactly 0.5.
- [ ] Rotate 45 with 15° snapping lands on 45.
- [ ] The gizmo is usable at 1600x1000 and scales with UI scale.
