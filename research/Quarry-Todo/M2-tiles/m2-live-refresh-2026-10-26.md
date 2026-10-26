---
id: "m2-live-refresh-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m2", "images", "art"]
order: 26
---

# M2 · Live refresh from the watcher

FR-ART-02 and NFR-02. The feature that makes external art usable.

**Build**
- `notify` 8.2.1 + `notify-debouncer-full` 0.7.0, 100 ms debounce, watching every linked source.
- Own-write suppression so a Quarry save doesn't trigger a reload loop.
- Reload swaps the texture in place (same `Image` id) as one undoable history entry, without focusing Quarry; < 1 s end to end.
- Per-image toggle for "watch this file", persisted.

**Done when**
- [ ] Save a PNG in Krita while Quarry sits behind it: the model updates in under 1 s.
- [ ] Saving the project does not trigger a refresh.
- [ ] Undo after a refresh restores the previous pixels.
