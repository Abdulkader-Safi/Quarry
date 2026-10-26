---
id: "m1-editor-layout-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "ui"]
order: 17
---

# M1 · Editor layout, tabs, status bar and console

FR-UI-01 and FR-UI-04.

**Build**
- Top bar: menu, mode switch (Tile / Edit / Paint / Rig / Animate), agent status pill (Stop wired in M8).
- Tabbed side panel: Tileset, UVs, Paint, Scene, Rig (empty states until their milestones).
- Bottom timeline area (placeholder), status bar with selection counts, tri/vertex counts, current tool, agent state.
- Console drawer; resizable and collapsible panels whose sizes persist.

**Done when**
- [ ] Resize the side panel, relaunch: the width is kept.
- [ ] Selecting nothing shows no fake counts; the status bar updates from real document state.
- [ ] Every panel has an empty state, not a blank box.
