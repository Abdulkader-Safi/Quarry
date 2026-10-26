---
id: "m5-glb-writer-2026-10-26"
status: "review"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T03:00:00.000Z"
labels: ["m5", "gltf", "export"]
order: 46
---

# M5 · Self-contained GLB writer

FR-EXP-01, FR-EXP-03, FR-EXP-04. Builds on the M0 spike writer.

**Build**
- `gltf-json` 1.4.1 + own GLB writer: one .glb with meshes, materials, base-colour PNG, nodes.
- Sampler magFilter 9728 (NEAREST) when the project uses nearest filtering.
- Node names plus `extras.quarry` for markers (type, properties) and collision flags.
- Selection and scale options; separate `.gltf` + `.bin` + PNG option for debugging only.
- `export.run` is long-running, returns the validator summary.

**Done when**
- [ ] Export the sample robot: exactly one .glb is written.
- [ ] Zero Khronos validator errors; opens in Blender with mesh, texture, all clips.
- [x] A flagged floor node carries `extras.quarry.collision = true`; a marker keeps its name and props.

## Progress 2026-10-26

- `export.run(path, format: glb|gltf_separate, selection: all|selected|objects, objects?, scale?)`, long, not undoable; agents may only export inside the project folder.
- `quarry-io::export`: nodes keep names and hierarchy (a root exported without its parent keeps its world placement); one mesh per mesh object, one primitive per image; instances reuse the source mesh; materials are unlit-looking PBR (metallic 0, roughness 1) with a PNG base colour, MASK at 0.5 when the image has transparent pixels; sampler NEAREST (9728) when the image is nearest-filtered; markers, collision and properties in `extras.quarry`; smoothing groups average normals.
- The reply carries files written and a check report from re-reading the file with the `gltf` crate (the Khronos validator runs in CI).
- Tested (`crates/quarry-io/tests/export.rs`): one .glb for a box; room: magFilter Nearest, Floor extras collision true, spawn_player marker and team=1 under Floor, instance shares the crate mesh, 2 meshes total; separate format writes .gltf, .bin and a PNG.
- Left for review: open `tests/goldens/room.glb` in Blender and Cairn. Skins and clips arrive in M6/M7, so the sample robot box waits for them.
