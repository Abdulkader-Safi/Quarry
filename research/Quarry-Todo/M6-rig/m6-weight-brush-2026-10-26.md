---
id: "m6-weight-brush-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m6", "rig", "paint"]
order: 53
---

# M6 · Weight brush with pressure and ramp view

FR-RIG-03.

**Build**
- Add, subtract, smooth, set-to-value; brush radius and strength; pen pressure affects strength.
- Weights view toggle rendering a colour ramp (choice in Settings); the ramp is legible at 13 px labels.
- One undo entry per stroke; the 4-influence rule applies after every stroke.

**Done when**
- [ ] Paint 1.0 on 10 vertices for bone B: they show full red and follow B fully.
- [ ] Subtract from the same vertices: values drop, still normalised.
- [ ] Smooth removes a hard edge without changing unrelated vertices.
