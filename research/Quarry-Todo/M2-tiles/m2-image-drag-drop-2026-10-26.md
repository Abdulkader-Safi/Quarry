---
id: "m2-image-drag-drop-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
