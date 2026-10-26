---
id: "m0-glb-skin-spike-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Validator reports zero errors.
- [ ] The generated file and the writing code are kept in `tests/goldens/` and `quarry-io`.
