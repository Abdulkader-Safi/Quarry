---
id: "m2-tileset-import-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T22:30:00.000Z"
labels: ["m2", "tiles", "images"]
order: 22
---

# M2 · Import a PNG or PSD tileset

FR-TILE-01. Everything tile-based starts here.

**Build**
- `tile.import_tileset`: PNG or PSD (flattened) → `Image` with role `tileset`, `tile_px`, `padding`, stored PNG in the zip.
- Tileset panel: slice into rows × columns (256 px at 16 px = 16x16 grid), scroll/zoom, tile index readout.
- Multiple tilesets per scene, each named; deleting a tileset in use asks first (M1 dialog).
- Undo: import is one history entry, and undoing it removes the tileset.

**Done when**
- [ ] Import a 256x256 PNG at 16 px: the panel shows a 16x16 grid.
- [ ] Imported art survives save/reopen byte-identical.
- [ ] Import then undo leaves no orphan image in the zip.

## Backend done 2026-10-26 (UI waits for the Figma design)

- `tile.import_tileset(path, tile_px?, padding?, name?)`: PNG or flattened PSD becomes a linked tileset; sets the brush to tile 0. Undo removes it. Tested: 256 px at 16 px is 16 x 16; pixels survive save and reopen.
- Left: the tileset panel (grid, scroll, zoom, index readout) and the delete-in-use confirm.
