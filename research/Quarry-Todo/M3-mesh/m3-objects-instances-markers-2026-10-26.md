---
id: "m3-objects-instances-markers-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m3", "scene"]
order: 38
---

# M3 · Objects, instances, markers and collision

FR-SCENE-02 to FR-SCENE-05.

**Build**
- Object from selected faces with the origin at the crosshair; origin-to-crosshair on any object.
- Instances: place copies that update when the source changes.
- Markers: named empty points, type (spawn, pickup, trigger, custom), custom key-values.
- Collision flag per object, hidden or visible.
- All of these are the data the exporter turns into node names and `extras` in M5.

**Done when**
- [ ] Make an object from 6 faces: it appears in the tree with its origin at the crosshair.
- [ ] Place 5 crate instances, edit the source: all 5 change.
- [ ] Marker `spawn_player` with `team=1` survives save/reopen and is queryable.
