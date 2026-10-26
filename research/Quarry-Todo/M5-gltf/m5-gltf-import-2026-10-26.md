---
id: "m5-gltf-import-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T03:40:00.000Z"
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
- [x] Skeleton and clips are editable after import.
- [x] A malformed .glb gives a plain error, no crash.

## Progress 2026-10-26

- `import.gltf(path)` (long, one undo step): `quarry-io::import` reads the file into a fragment; `Document::merge` adds it with fresh IDs and unique names and fixes every reference.
- One object per node with names and hierarchy; skin joints become skeleton bones (rest pose from inverse bind matrices, tails toward children), not objects; base-colour textures become images with the sampler's filter; JOINTS/WEIGHTS become top-4 normalised influences; animations on skin joints become clips per skeleton (step or linear; cubic-spline keys keep values); Quarry's own `extras.quarry` come back as markers, collision and properties.
- Tests (`crates/quarry-io/tests/import.rs`): room.glb returns Floor (collision), spawn_player under it with team=1, Crate and its instance, a nearest texture; undo removes all of it. skin-bar.glb returns 3 bones and the 2 s `bend` clip with 2 tracks, weights summing to 1, no objects for joints. A broken .glb is an io error naming the file.
- Left for review: import a Kenney CC0 glTF character and check each mesh is its own object (no network here to fetch one). Node animation outside skins is not imported yet.
