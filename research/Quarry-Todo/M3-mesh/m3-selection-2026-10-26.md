---
id: "m3-selection-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m3", "mesh", "ui"]
order: 31
---

# M3 · Selection modes: click, box, lasso

FR-MESH-02. Selection is the input every mesh command relies on.

**Build**
- Modes: vertex, edge, face, object. Click, box and lasso; add, remove, invert, select all, deselect.
- Backface and through-selection handling; picking from the renderer's id buffer.
- Near-vertex disambiguation (the main complaint about the tool this replaces): cycling on repeated click, and a zoom-in hint.
- Selection state exposed to `query.selection` and the status bar counts.

**Done when**
- [ ] Box-select 4 vertices, Shift-click a fifth: 5 selected.
- [ ] Lasso selects the same set as a box around them.
- [ ] Invert twice returns the original set.
