---
id: "m7-ik-keying-bake-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T07:30:00.000Z"
labels: ["m7", "anim", "rig"]
order: 62
---

# M7 · Keyable IK and bake to bone rotations

FR-ANIM-05.

**Build**
- IK targets are keyable (position keys on the target object/empty).
- On export, solve the chain per sample and write rotation keys on thigh/shin (arm bones) only — no target keys in the glTF.
- Preview uses the same solver as export, so what plays is what exports.

**Done when**
- [x] Key a foot target at 3 times: the exported glTF has rotation keys on thigh and shin only, and the foot lands where it did in Quarry.
- [x] Deleting a target key falls back to the rest solve without breaking the chain.
- [x] Export output is identical when re-run with no changes.

## Progress 2026-10-27

- IK targets and poles are keyable (`anim.key_set` with channel ik_target or ik_pole on the chain's tip bone); a chain without keys uses its own target, so deleting keys falls back cleanly. Export bakes the solve into thigh and shin rotations; no target keys reach the glTF.
- Tested: three target keys put the foot exactly on each target; the exported animation has rotation channels on thigh and shin only, within 0.5 degrees of Quarry at each key; re-export is identical.
- Left for review: the foot landing in Cairn.
