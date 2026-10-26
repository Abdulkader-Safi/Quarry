---
id: "m5-gltf-import-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m5", "gltf", "import"]
order: 48
---

# M5 · glTF 2.0 and GLB import

FR-IMP-01.

**Build**
- `gltf` 1.4.1: meshes, base-colour material + texture, skins, animations, node hierarchy.
- Hierarchy stays separate objects (no merge-everything behaviour); names preserved; clips become editable.
- Import is one undo entry; textures land as embedded images; failures name the file and the reason.

**Done when**
- [ ] A Kenney CC0 glTF character imports with each mesh its own object.
- [ ] Skeleton and clips are editable after import.
- [ ] A malformed .glb gives a plain error, no crash.
