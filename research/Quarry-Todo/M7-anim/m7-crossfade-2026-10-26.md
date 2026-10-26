---
id: "m7-crossfade-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T07:30:00.000Z"
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
- [x] Preview idle → walk with 0.25 s fade: the pose blends over 0.25 s.
- [x] Fade time 0 gives a hard cut, matching Cairn.
- [x] Preview leaves the document untouched and undo history clean.

## Backend done 2026-10-27

- `anim.crossfade_preview(from, to, fade, time)` returns the blended pose (linear weight time/fade like Cairn's Fade, slerp for rotations); fade 0 is a hard cut; nothing in the document or history changes.
- Tested: halfway through a 0.25 s fade the rotation is half way; the document and history are identical afterwards.
- Left: the on-screen readout and default fade setting; matching against Cairn by eye.
