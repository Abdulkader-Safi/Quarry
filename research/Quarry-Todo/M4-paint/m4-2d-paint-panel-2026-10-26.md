---
id: "m4-2d-paint-panel-2026-10-26"
status: "review"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
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
- [x] Pencil one pixel at 400%: that texel changes and nothing else.
- [x] 16-colour palette lock: painting an off-palette colour lands on the nearest palette entry.
- [x] A new 32x32 texture on a box shows on all faces.

## Backend done 2026-10-27 (UI waits for the Figma design)

- `paint.texture_new(width, height, color?, name?)` on selected faces (all when none), UVs rescaled so the whole texture shows; `paint.pixels` (pencil, and eraser with alpha 0), `paint.rect`, `paint.fill` (flood), `paint.palette_set` (makes it active), `paint.palette_lock`. Each call is one undo step; palette lock snaps by redmean distance and never snaps the eraser. Pixel logic in `quarry-ops::paint`.
- Tests (`crates/quarry-core/tests/paint.rs`): one pencil pixel changes exactly one texel and undoes; a 16-colour palette snaps an off-palette colour to its nearest entry; a new 32x32 texture covers all six box faces.
- Left: the 2D panel element (zoom, tools, rectangle select, palette strip, eyedropper reading the image).

**UI (2026-10-26)**
Paint tab: pencil, eraser, fill, eyedropper on the 2D canvas at whole-number zoom; palette swatches and lock; size slider; texture list and New blank texture.
