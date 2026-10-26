---
id: "m9-accessibility-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m9", "nfr", "ui"]
order: 77
---

# M9 · Accessibility and UI scale pass

NFR-06 and part of FR-UI-05.

**Do**
- UI scale 100–200% with no clipped labels or unusable panels at either end.
- Minimum 13 px text at 100%; check the timeline and status bar, which are the tightest.
- Every action reachable without a shortcut (menus and command search cover everything).
- English only, all strings in one file; no hardcoded strings in view code.

**Done when**
- [ ] Screenshots at 100%, 150% and 200% are in `PROGRESS.md`.
- [ ] A keyboard-only pass can add a primitive, paint, rig and export.
- [ ] A grep finds no user-facing string outside the strings file.
