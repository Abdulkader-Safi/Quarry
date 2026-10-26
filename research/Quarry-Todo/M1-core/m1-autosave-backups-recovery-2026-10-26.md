---
id: "m1-autosave-backups-recovery-2026-10-26"
status: "done"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T19:00:00.000Z"
labels: ["m1", "io", "project"]
order: 12
---

# M1 · Autosave, backups and crash recovery

FR-PROJ-03, FR-PROJ-04, FR-PROJ-05.

**Build**
- Autosave every 2 minutes (configurable) to a side copy; never write over the user's file.
- Keep the last 10 backups per project, oldest dropped first.
- On launch, detect an autosave newer than the project file and offer recovery (file name + autosave time); Recover restores it.

**Done when**
- [x] Kill the process mid-edit: the original file is untouched and the autosave holds the edit.
- [x] Relaunch shows the recovery prompt; Recover restores the edit.
- [x] After 11 saves the backups folder holds exactly 10.

## Done 2026-10-26

- `quarry-io::autosave`: side copies in `<data>/autosave/<stem>-<fnv hash>.quarry` with a `.json` sidecar naming the project (or untitled); `recoverable()` lists autosaves newer than their file, newest first; `Autosaver` fires every N minutes only when the revision changed.
- Backups: `QuarryFiles { dirs }` copies the old file to `<data>/backups/<key>/` before each save and keeps 10; a failed backup is logged and never blocks the save. A successful save clears that project's autosave.
- `project.recover(autosave)` (new command, FR-PROJ-04): loads the autosave, keeps the original file path, leaves the project unsaved.
- Tests (`crates/quarry-io/tests/autosave.rs`): crash after an autosave leaves the user's file untouched and recovery restores the edit, then saving clears the side copy; untitled work is offered too; 11 saves keep 10 backups; timer rules.
- Still to wire: the timer in the app and headless loops (server card) and the recovery prompt UI (start screen card).
