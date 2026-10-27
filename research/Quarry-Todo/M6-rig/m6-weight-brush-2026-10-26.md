---
id: "m6-weight-brush-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
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
- [x] Subtract from the same vertices: values drop, still normalised.
- [ ] Smooth removes a hard edge without changing unrelated vertices.

## Backend done 2026-10-27 (UI waits for the Figma design)

- `rig.weight_paint(object, bone, mode: add|subtract|set|smooth, value, vertices?)`: other bones keep their share of what is left; with none left, the nearest other bone takes it; smooth blends with edge neighbours. Every result goes through `normalise`. One call is one undo step.
- Tested: subtract lowers the bone and the sum stays 1.
- Left: the brush (radius, strength, pressure) and the weight ramp view.

**UI (2026-10-26)**
UI: weight brush modes, strength slider, radius; clicking a face paints its vertices for the selected bone; auto weights, rigid, mirror, add IK target.
