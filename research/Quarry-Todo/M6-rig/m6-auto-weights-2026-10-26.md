---
id: "m6-auto-weights-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T05:30:00.000Z"
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
- [x] Bind a 2-bone arm: rotating the lower bone bends only the forearm half; shoulder verts do not move.
- [x] Paint a vertex for 5 bones: 4 remain and they sum to 1.000.
- [x] Bind a hand object to `hand.L`: rotating the bone moves it rigidly.

## Done 2026-10-27

- `quarry-ops::rig::normalise` is the one helper every weight path uses (auto, brush, mirror, delete, import): merge duplicates, keep the 4 largest, sum exactly 1.
- `rig.bind_auto(object, skeleton?)` (long): 1/d^4 distance to each bone segment, under 2% dropped, so vertices follow their nearest bone and blend only at joints. `rig.bind_rigid(object, bone, selected_faces?)`.
- Tested (`crates/quarry-core/tests/rig.rs`, CPU skinning): bending the forearm 45 degrees leaves every shoulder vertex where it was and moves the hand end; a vertex painted for 5 bones keeps 4 summing to 1; a hand bound rigidly keeps all its distances when the bone turns 60 degrees.
