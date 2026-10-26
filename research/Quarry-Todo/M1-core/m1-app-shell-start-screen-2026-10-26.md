---
id: "m1-app-shell-start-screen-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "ui"]
order: 16
---

# M1 · App shell and start screen

FR-PROJ-06, FR-PROJ-04, first-run flow.

**Build**
- GPUI app crate with the start screen: recent projects with thumbnails and missing markers, New (name, tile size), Open, Sample robot (disabled until M8), recovery prompt with file name and autosave time, first-run overlay.
- Window state restore; no-project, missing-file and recovery-available states all designed.

**Done when**
- [ ] Open three projects, relaunch: all three listed newest first.
- [ ] A missing file is labelled, not a crash.
- [ ] New project lands in the editor with pixel-art defaults.
