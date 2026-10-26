---
id: "m3-scene-tree-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T01:20:00.000Z"
labels: ["m3", "scene", "ui"]
order: 37
---

# M3 · Scene tree: rename, reorder, parent, hide, lock

FR-SCENE-01.

**Build**
- Tree of objects and layers with drag to reorder and reparent, rename inline, hide and lock toggles, filter field (FR-STATE-06, Should).
- Parent transforms compose; moving A moves B.
- Locked objects are not selectable in the viewport; hidden ones are not drawn or picked.
- Selection syncs both ways between tree and viewport.

**Done when**
- [x] Parent B under A, move A: B follows.
- [ ] Lock A: clicking it in the viewport selects nothing.
- [ ] Filter "crate" hides non-matching rows.

## Backend done 2026-10-26 (UI waits for the Figma design)

- `scene.parent(objects, parent?)` keeps world placement and refuses loops; `scene.reorder(object, index)` with a saved per-object `order`; `query.scene` lists siblings in that order. Rename, visibility, lock and delete existed from M1.
- Locked and hidden objects are skipped by `select.box` and `select.all`.
- Tested: B parented under A stays put, then follows A up 2 m; reorder puts B first.
- Left: the tree panel with drag to reorder and reparent, inline rename, the filter field, and viewport picking that ignores locked objects.
