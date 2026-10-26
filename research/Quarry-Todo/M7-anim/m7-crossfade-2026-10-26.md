---
id: "m7-crossfade-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m7", "anim"]
order: 64
---

# M7 · Crossfade preview matching Cairn's Fade

FR-ANIM-08.

**Build**
- Pick two clips and a fade time; preview blends bone TRS over that time the way Cairn's animator Fade does.
- On-screen readout of current clip, fade progress and time.
- Settings for default fade; the preview never modifies keys.

**Done when**
- [ ] Preview idle → walk with 0.25 s fade: the pose blends over 0.25 s.
- [ ] Fade time 0 gives a hard cut, matching Cairn.
- [ ] Preview leaves the document untouched and undo history clean.
