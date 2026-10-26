---
id: "m7-export-sampling-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T07:30:00.000Z"
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
- [x] A bone held still all clip exports with ≤ 2 keys.
- [x] Key reduction never exceeds the 0.5° tolerance (fails CI if it does).

## Progress 2026-10-27

- Export samples every clip at project fps through `Clip::pose_at` (IK solved per sample), writes joint-local TRS, only for channels that leave rest, then `quarry-ops::anim::reduce` (0.05 degrees, 0.1 mm) keeps every original sample within tolerance. Bones keyed only with step keys and free of IK export those keys as STEP.
- Measured (test): eased walk on a 2-bone arm, 29 keys from 31 samples, worst error 0.19 degrees against Quarry's playback at 120 Hz. A bone held at 20 degrees exports 2 keys. Re-export is byte-identical. `anim.glb` golden: validator 0/0/0. The test fails CI if the error passes 0.5 degrees.
- Left for review: play the clip in Cairn and compare by eye (an xtask compare needs Cairn).
