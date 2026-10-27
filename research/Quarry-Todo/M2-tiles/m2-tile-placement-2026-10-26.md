---
id: "m2-tile-placement-2026-10-26"
status: "review"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
labels: ["m2", "tiles"]
order: 23
---

# M2 · Place, erase, fill and eyedrop tiles

FR-TILE-02, FR-TILE-03, FR-TILE-09. The core loop of the tool.

**Build**
- Brush: pick one tile, or drag-select a rectangle (2x2 shows all four in the preview).
- Placement plane at the crosshair; click places, drag paints a line of tiles, right-click erases, rectangle fill and rectangle erase.
- One quad per cell, UVs from the tile rect, per-face image; drag across 5 cells writes 5 quads as one undo entry.
- `tile.eyedrop` puts the clicked tile (with its rotation) into the brush.

**Done when**
- [ ] Drag across 5 cells: exactly 5 quads, each textured with the brush tile.
- [ ] Rectangle fill 3x3 then rectangle erase 1 row leaves 6 quads.
- [ ] Eyedrop a rotated tile: the brush shows that tile at that rotation.

## Backend done 2026-10-26 (UI waits for the Figma design)

- `tile.brush` (tile number or col/row, width x height), `tile.place` (cells, default the crosshair cell; one call = one undo step), `tile.erase`, `tile.fill_rect` (repeats multi-tile brushes), `tile.erase_rect`, `tile.eyedrop(object, face)` (reads rotation, mirror and flip back from the UVs).
- Tiles are quads in a "Tiles" mesh object (or a named one), sharing vertices; placing on a filled cell replaces it; erasing drops orphan vertices.
- Tested: 5-cell drag is 5 quads, 12 vertices, one history entry; 3x3 fill then erasing a row leaves 6; eyedrop of a 90-degree tile gives rotate 1.
- Left: mouse input (click, drag, right-click erase, rectangle drag) and the brush preview.

**UI (2026-10-26)**
UI: click places the brush on the plane cell under the cursor, drag paints, right-click erases, rectangle fill, pick tile from the view.
