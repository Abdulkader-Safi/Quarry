---
id: "m6-two-bone-ik-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Drag a leg's foot target: thigh and shin rotate to reach it and the knee points at the pole.
- [ ] Move the target out of reach: the leg stops at full extension, no NaN, no flip.
- [ ] Two chains on one skeleton solve independently.
