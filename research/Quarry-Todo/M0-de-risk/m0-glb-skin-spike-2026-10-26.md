---
id: "m0-glb-skin-spike-2026-10-26"
status: "review"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T15:25:00.000Z"
labels: ["m0", "gltf", "spike"]
order: 6
---

# M0 · Hand-built skinned GLB through the validator and Cairn

The contract with Cairn. Prove skin + clip export before any rig UI exists.

**Build**
- Hand-build a 3-bone bending bar as a 3 or 4 corner polygon mesh with weights.
- Write it with the own GLB writer on `gltf-json` 1.4.1 (the writer becomes the first export test later).
- Validate with the Khronos glTF Validator; load in Cairn and Blender; play its clip.

**Done when**
- [ ] The bar bends correctly in Cairn and in Blender, one clip, one skeleton.
- [x] Validator reports zero errors.
- [x] The generated file and the writing code are kept in `tests/goldens/` and `quarry-io`.

## Progress 2026-10-26

- GLB writer: `crates/quarry-io/src/glb.rs` (one JSON chunk, one BIN chunk, accessors with min/max where glTF needs them).
- Bar: `crates/quarry-io/tests/skin_bar.rs` builds a 0.2 m square bar, 3 m tall, 3 bones of 1 m, 7 rings, blended weights at each joint, flat normals. One clip `bend`: bones 1 and 2 rotate 40 degrees about Z and back over 2 s.
- Golden: `tests/goldens/skin-bar.glb`. The test compares bytes, reads it back with the `gltf` crate, checks weights sum to 1, and CPU-skins the peak pose (top swings to -X, root ring stays put).
- Khronos validator 2.0.0-dev.3.10: 0 errors, 0 warnings, 0 infos. Run with `cargo run -p xtask -- validate-gltf`; CI runs it too.
- Left for review: load `tests/goldens/skin-bar.glb` in Cairn and Blender and play `bend`.
