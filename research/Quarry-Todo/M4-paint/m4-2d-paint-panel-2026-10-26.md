---
id: "m4-2d-paint-panel-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m4", "paint"]
order: 41
---

# M4 · 2D pixel paint panel

FR-PAINT-01, FR-PAINT-04, FR-PAINT-06.

**Build**
- `paint.texture_new` at a given size, assigned to selected faces.
- 2D panel: zoom (400%+), pencil, eraser, fill, eyedropper, rectangle select, palette strip.
- Palette lock: every painted pixel snaps to the nearest palette colour; toggle per project.
- Painting writes into the `Image` used by the faces; one undo entry per stroke, not per pixel.

**Done when**
- [ ] Pencil one pixel at 400%: that texel changes and nothing else.
- [ ] 16-colour palette lock: painting an off-palette colour lands on the nearest palette entry.
- [ ] A new 32x32 texture on a box shows on all faces.
