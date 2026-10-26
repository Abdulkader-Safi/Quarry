---
id: "m3-scene-tree-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Parent B under A, move A: B follows.
- [ ] Lock A: clicking it in the viewport selects nothing.
- [ ] Filter "crate" hides non-matching rows.
