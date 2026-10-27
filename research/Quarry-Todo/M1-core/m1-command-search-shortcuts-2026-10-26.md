---
id: "m1-command-search-shortcuts-2026-10-26"
status: "review"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T13:45:00.000Z"
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

**UI (2026-10-26)**
⌘K palette lists every registry command with fuzzy match, arrow keys and Enter. Shortcuts page in Settings lists the keys.

**Rebinding (2026-10-27)**
One keymap (`crates/quarry/src/keys.rs`) drives the keyboard, the Settings list, the palette hints, the menus and the view's hint bar. Click a key in Settings, press the new one (Esc cancels). If another action held it, that action is left with no key and the note under the list names it. Overrides save to `settings.shortcuts`; Reset to default clears them. Not built: the Blender-style preset (the selector still shows "Quarry default" only).
