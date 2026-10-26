---
id: "m1-autosave-backups-recovery-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] Kill the process mid-edit: the original file is untouched and the autosave holds the edit.
- [ ] Relaunch shows the recovery prompt; Recover restores the edit.
- [ ] After 11 saves the backups folder holds exactly 10.
