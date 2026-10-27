---
id: "m7-timeline-keys-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
labels: ["m7", "anim", "ui"]
order: 60
---

# M7 · Timeline, tracks, keys and interpolation

FR-ANIM-02 and FR-ANIM-03. The largest UI element after the viewport.

**Build**
- Timeline element: track tree per bone, T/R/S channels, key diamonds, playhead, zoom and pan.
- Add, move, copy, paste, delete keys; auto-key toggle; multi-select and box-select keys.
- Per-key interpolation: step, linear, ease in, ease out, ease in/out (bezier handles are M7 optional).
- Snapping to frames and to other keys; undo per edit.

**Done when**
- [x] With auto-key on, rotate a bone at 0.5 s: a rotation key appears at 0.5 s.
- [x] Copy keys from 0–1 s and paste at 2 s: values match, times shift.
- [x] A step key makes playback jump instead of blending.

## Backend done 2026-10-27 (UI waits for the Figma design)

- `anim.key_set(clip?, bone, channel, time?, value?, interp?)` keys the current pose when no value is given; `anim.key_move`, `anim.key_delete`, `anim.key_copy` (lists), `anim.key_paste(from_clip?, start, end, at)`, `anim.autokey`. `rig.pose` and `rig.ik_set` key into the active clip at the playhead when auto-key is on.
- Interpolation per key: step, linear, ease in, ease out, ease in/out (`quarry-ops::anim`); bezier plays as ease in/out until the optional graph editor.
- Tested: auto-key at 0.5 s; paste of 0 to 1 s at 2 s; a step key holds until the next key.
- Left: the timeline element (tracks, diamonds, zoom, box select, snapping).

**UI (2026-10-26)**
Timeline: ruler click sets the playhead, tracks per bone channel, keys as diamonds (agent keys violet), interpolation select, auto-key. Dragging keys is not in yet.
