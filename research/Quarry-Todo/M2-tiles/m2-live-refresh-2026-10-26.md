---
id: "m2-live-refresh-2026-10-26"
status: "review"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T23:00:00.000Z"
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
- [x] Saving the project does not trigger a refresh.
- [x] Undo after a refresh restores the previous pixels.

## Progress 2026-10-26

- `quarry-io::watcher::ArtWatcher`: notify 8.2.1 + debouncer-full 0.7.0 at 100 ms, watching the folders of linked images with `watch` on (folders, because editors save by rename). `changed()` names images whose file was touched; the owner runs `art.reload`, which compares file hashes, so own writes and no-op saves never reload.
- Wired into `quarry --headless`; the app loop does the same when the shell lands.
- Measured (test, M2 Pro): editor-style save to new pixels in the document 133 to 137 ms, three runs. NFR-02 target is 1 s.
- Tested: project save in the same folder triggers nothing; undo after a refresh restores the old pixels.
- Per-image toggle: `art.watch(image, watch)`, saved in the project.
- Left for review: save a PNG in Krita while Quarry sits behind it and watch the viewport (needs the app window).
