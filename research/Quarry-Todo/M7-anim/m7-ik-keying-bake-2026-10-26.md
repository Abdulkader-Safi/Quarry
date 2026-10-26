---
id: "m7-ik-keying-bake-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Key a foot target at 3 times: the exported glTF has rotation keys on thigh and shin only, and the foot lands where it did in Quarry.
- [ ] Deleting a target key falls back to the rest solve without breaking the chain.
- [ ] Export output is identical when re-run with no changes.
