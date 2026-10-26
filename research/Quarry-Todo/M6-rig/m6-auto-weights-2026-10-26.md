---
id: "m6-auto-weights-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m6", "rig"]
order: 52
---

# M6 · Auto weights and rigid binding

FR-RIG-02, FR-RIG-04, FR-RIG-05.

**Build**
- `rig.bind_auto`: weights from bone distance with falloff; keep the 4 largest, normalise to sum exactly 1.
- One shared `normalise_weights` helper used by every weight path (auto, brush, mirror, import) so the 4-influence invariant cannot drift.
- `rig.bind_rigid`: whole object or face set to one bone at 100%.
- Binding is undoable and keeps the rest pose intact.

**Done when**
- [ ] Bind a 2-bone arm: rotating the lower bone bends only the forearm half; shoulder verts do not move.
- [ ] Paint a vertex for 5 bones: 4 remain and they sum to 1.000.
- [ ] Bind a hand object to `hand.L`: rotating the bone moves it rigidly.
