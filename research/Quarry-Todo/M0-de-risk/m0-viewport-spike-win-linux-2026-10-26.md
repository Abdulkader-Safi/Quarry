---
id: "m0-viewport-spike-win-linux-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m0", "render", "spike"]
order: 4
---

# M0 · Viewport spike: Windows and Linux readback

Same scene, different presentation path.

**Build**
- Present the wgpu offscreen target through double-buffered CPU readback (no zero-copy path exists there).
- Run on real Windows 10/11 x64 and Ubuntu 22.04 x64 where possible; note where CI lavapipe was used instead of hardware.

**Done when**
- [ ] At least 30 fps and input latency under 50 ms on each OS, or the measured shortfall recorded.
- [ ] Results in `PROGRESS.md` next to the macOS numbers.
