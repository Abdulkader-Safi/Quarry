---
id: "m1-command-search-shortcuts-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "ui"]
order: 19
---

# M1 · Command search, shortcuts and menus

FR-UI-02 and FR-UI-03, driven by the registry rather than a hand-kept list.

**Build**
- Cmd/Ctrl+K palette over every command with its shortcut; type and Enter runs it.
- Rebindable shortcuts, default scheme plus a Blender-style preset, reset to default.
- Binding conflict warns and names the other action.
- Menus call the same commands, so search, menu and keyboard can never disagree.

**Done when**
- [ ] Typing "extr" shows Extrude with its shortcut and runs on Enter.
- [ ] Binding Extrude to a taken key shows a conflict naming that action.
- [ ] A new command appears in search, the menu and the shortcut editor with no extra wiring.
