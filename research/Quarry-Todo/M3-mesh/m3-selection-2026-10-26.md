---
id: "m3-selection-2026-10-26"
status: "review"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
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
- [x] Box-select 4 vertices, Shift-click a fifth: 5 selected.
- [ ] Lasso selects the same set as a box around them.
- [x] Invert twice returns the original set.

## Backend done 2026-10-26 (UI waits for the Figma design)

- `Document.selection` (mode, edited object, objects, vertices, edges, faces): not saved, not an undo step, restored by undo with its mesh; the registry drops indices that no longer exist after every command.
- Commands: `select.mode`, `select.by_ids` (add/remove), `select.box` (rect in viewport fractions through the saved camera), `select.all`, `select.none`, `select.invert`, `select.by_image`, `select.by_normal`, `query.selection` (ids and world bounds).
- Tested: front view box over the left half selects 4 vertices, adding one makes 5; invert twice is the original set; by_normal up picks the top face; bad indices change nothing.
- Left (UI): click and lasso input, picking from the renderer's id buffer with backface handling, near-vertex cycling, status bar counts. Lasso can feed `select.by_ids`.

**UI (2026-10-26)**
Object/Vertex/Edge/Face segmented control and 1-4 keys; click picks by ray against the real triangles; selection tinted cobalt in the view. Box drag is not wired in the view yet (select.box works through the palette and MCP).
