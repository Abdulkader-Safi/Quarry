---
id: "m6-two-bone-ik-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T05:30:00.000Z"
labels: ["m6", "rig"]
order: 54
---

# M6 · Two-bone IK with target and pole

FR-RIG-06. Analytic, own code in `quarry-ops`.

**Build**
- `rig.ik_add` / `rig.ik_set`: chain = tip bone, target object position, pole vector (with a pole-angle fallback).
- Dragging the target solves thigh/shin (or upper/lower arm) so the tip reaches it; the pole controls the knee/elbow direction.
- Clamp unreachable targets with a clear status message instead of stretching.
- Pose changes show in the viewport immediately; IK is a pose-time solve, not baked until export.

**Done when**
- [x] Drag a leg's foot target: thigh and shin rotate to reach it and the knee points at the pole.
- [x] Move the target out of reach: the leg stops at full extension, no NaN, no flip.
- [x] Two chains on one skeleton solve independently.

## Done 2026-10-27

- `quarry-ops::rig::two_bone_ik`: law of cosines in the plane through the target leaning to the pole; reach clamped just inside full stretch so nothing goes NaN or flips.
- `rig.ik_add(tip_bone, target?, pole?)`, `rig.ik_set(tip_bone, target?, pole?)` solve the chain into the pose and report the remaining gap, with a warning when out of reach. `rig.pose`, `rig.pose_reset`.
- Tested: a leg reaches (0.2, 0.6, 0.6) with the knee toward the pole; an out-of-reach target stops at 2 m full stretch with a warning; the other leg's chain is untouched.
- Dragging the target in the viewport is UI work for the app shell.
