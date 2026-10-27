---
id: "m2-image-drag-drop-2026-10-26"
status: "review"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T14:40:00.000Z"
labels: ["m2", "images", "ui"]
order: 27
---

# M2 · Drag and drop an image into the window

FR-ART-06.

**Build**
- Native file drop on the window accepts PNG and PSD.
- Small dialog: tileset (tile size + padding) or texture (assign to selection), then import.
- Multiple files in one drop import in order, each as its own image.

**Done when**
- [ ] Drop a PNG: the dialog asks tileset or texture and the import completes.
- [ ] Drop a PSD: same flow, flattened.
- [ ] Dropping an unsupported file gives a plain error and changes nothing.

## Backend done 2026-10-26

- `art.import(path, as: tileset|texture, tile_px?, padding?)` is what a drop runs; PNG and flattened PSD; unsupported files return an io error and change nothing.
- Left: the native file drop and the tileset/texture dialog (app shell).

## App done 2026-10-27

- Dropping files on the window: a `.quarry` opens; PNG and PSD open the Import images dialog (tileset with tile size 8/16/32/64 and padding 0/1/2, or texture); other files are named in the status bar and nothing changes. Several images import in drop order, each its own image, each one `art.import` (one undo step each).
- `art.import` takes `assign: true` for textures: the texture goes on the selected faces of the edited mesh, or the whole selected object in object mode, with UVs stretched to the image. `paint.texture_new` shares that code. Test: `crates/quarry-io/tests/import_assign.rs`.
- Texture is preselected when an object is selected, tileset otherwise.
- `QUARRY_UI="drop=a.png,b.txt"` simulates a drop for screenshots. Checked that way: the dialog opens and the .txt is named as left out.
- For review: a real Finder drag of a PNG and a PSD.
