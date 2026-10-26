---
id: "m7-export-sampling-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m7", "anim", "gltf"]
order: 63
---

# M7 · Export sampling and key reduction

FR-ANIM-10. The check that Cairn sees exactly what Quarry played.

**Build**
- Sample every clip at project fps into LINEAR keys; STEP keys stay STEP.
- Drop keys a straight line through their neighbours already predicts (Blender's "optimize animation size").
- IK baked before sampling; held bones export with 2 keys or fewer.
- xtask compare: run the same clip in Cairn and Quarry, report per-bone angle error.

**Done when**
- [ ] A 1 s walk with eased keys plays in Cairn within 0.5° per bone of Quarry.
- [ ] A bone held still all clip exports with ≤ 2 keys.
- [ ] Key reduction never exceeds the 0.5° tolerance (fails CI if it does).
