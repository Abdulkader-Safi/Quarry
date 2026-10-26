---
id: "start-here-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["index", "m0"]
order: 1
---

# Start here: build order and source of truth

The spec is the source of truth. Read `docs/spec/06-build-spec.md` before any card; every card below only points at it.

**Build order — do not skip ahead**
1. [[m0-workspace-and-ci|M0 workspace and CI]]
2. [[m0-gate|M0 gate]] — no feature work until its numbers are in `PROGRESS.md`
3. M1 core, shell and command server
4. M2 tiles and linked images
5. M3 mesh, UV and scene
6. M5 glTF export (POC ends here, per build-spec risk 6)
7. M4 paint, M6 rig, M7 animation, M8 agent experience, M9 release

**Rules that never bend**
- Only `CommandRegistry::execute` mutates the document.
- Every user-facing action is a command, every command is an MCP tool.
- A failed command changes nothing; one shared undo history labelled human or agent.
- No GPUI below the app crate. No network. glTF must pass the Khronos validator.
- Pin GPUI; the only pre-approved stack swap is GPUI → egui 0.38/0.37 if M0 misses its numbers.

**Per milestone**
- Work the `M<n>` cards in order, optional cards last and droppable first.
- Update `PROGRESS.md`: status, measurements, deviations and why.
- Tick the acceptance boxes the milestone covers. Don't move on with a failing box.
- Commit with the milestone in the message.
