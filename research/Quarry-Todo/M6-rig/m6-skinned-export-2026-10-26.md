---
id: "m6-skinned-export-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Posing then exporting does not move the bind pose.
