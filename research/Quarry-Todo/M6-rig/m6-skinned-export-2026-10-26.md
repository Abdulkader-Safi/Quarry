---
id: "m6-skinned-export-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T06:10:00.000Z"
labels: ["m6", "rig", "gltf"]
order: 56
---

# M6 · Skinned export: skin, inverse bind matrices, joints and weights

FR-EXP-02. The bridge between Quarry and Cairn.

**Build**
- Skin with `inverseBindMatrices`, `JOINTS_0` and `WEIGHTS_0` (4 influences), skeleton root node.
- Per-clip glTF animations named exactly after the clips.
- Rest pose exported correctly even when the skeleton is currently posed.
- Validator + Blender check in CI goldens; manual Cairn check.

**Done when**
- [ ] The sample robot .glb shows one skeleton and its clips by name in Cairn.
- [ ] Validator zero errors; Blender shows the armature and vertex groups.
- [x] Posing then exporting does not move the bind pose.

## Progress 2026-10-27

- Export writes, per skeleton, a root node named after it with one joint node per bone in rest pose, a glTF skin (`inverseBindMatrices` = inverse rest matrices, `skeleton` = that root), and for skinned meshes `JOINTS_0` (u8) and `WEIGHTS_0`, with positions baked to skeleton space and the node at the scene root (glTF ignores skinned node transforms). Joints and weights are part of the vertex-sharing key.
- Golden `tests/goldens/arm.glb` (2-bone arm, auto weights): Khronos validator 0 errors, 0 warnings, 0 infos. Tests: one skin, joints upper.L and lower.L, rest world times IBM is identity for both, weights sum to 1; exporting after a 70 degree pose gives the same bytes as at rest; re-importing gives the same rest skeleton.
- Left for review: Blender shows the armature and vertex groups; Cairn shows one skeleton. Clips by name arrive with M7.
