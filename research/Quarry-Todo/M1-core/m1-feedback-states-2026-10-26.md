---
id: "m1-feedback-states-2026-10-26"
status: "review"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
labels: ["m1", "ui", "state"]
order: 20
---

# M1 · Toasts, progress, errors, confirmations and history panel

FR-STATE-01 to FR-STATE-05 plus the history panel (FR-STATE-08, Should).

**Build**
- Toast + console line for every error, written as what happened and what to do.
- Progress overlay for anything over 300 ms (open, save, bake, export) on a worker so the viewport still orbits; progress lines from the server feed it.
- Confirm dialogs: close with unsaved changes, delete a tileset in use (list the faces), delete a skeleton with clips.
- Empty scene hint (add a primitive, import a tileset, open the sample); empty timeline hint ("Add a skeleton to animate").
- History panel listing entries with source labels; click to jump is optional.

**Done when**
- [ ] A 100k-tri bake shows moving progress and the camera still orbits.
- [ ] Opening a corrupt file names it, keeps the app alive, and appears in console.
- [ ] Undo after an agent edit is one step and reads `Agent: extrude`.

**UI (2026-10-26)**
Errors from any command show in the status bar for six seconds; empty scene, no tileset, no texture and no skeleton states have note boxes; live-refresh toast with Select those faces.
