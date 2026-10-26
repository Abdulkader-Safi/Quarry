---
id: "m3-objects-instances-markers-2026-10-26"
status: "done"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T01:20:00.000Z"
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
- [x] Make an object from 6 faces: it appears in the tree with its origin at the crosshair.
- [x] Place 5 crate instances, edit the source: all 5 change.
- [x] Marker `spawn_player` with `team=1` survives save/reopen and is queryable.

## Done 2026-10-26

- `scene.object_from_selection(name?)` (faces move to a new object with its origin at the crosshair, geometry stays put), `scene.origin_to_cursor(object?)`, `scene.instance_add(source, position?, rotation?, scale?)` (draws the source mesh; deleting a source with instances is refused), `scene.marker_add(name, type, props, position?)`, `scene.set_collision(objects, collision)`, `scene.set_props(object, set, remove)`.
- Tests (`crates/quarry-core/tests/scene.rs`): 6 faces become "Lid" at the crosshair; 5 instances show an extrude on the source; `spawn_player` with team=1 survives a JSON round trip; a collision flag sticks.
- The exporter turns these into node names and extras in M5.
