---
id: "m0-pen-pressure-spike-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m0", "input", "spike"]
order: 5
---

# M0 · Pen pressure spike

Pressure is a Must for painting (FR-PAINT-03), so prove it arrives before building a tablet UI.

**Build**
- macOS: `objc2` 0.6.5 NSEvent monitor reading tablet pressure, mapped 0..1.
- Windows/Linux: `octotablet` 0.1.0 with a Wacom, noting it has been stale since March 2024.
- A tiny pressure readout in the spike window to eyeball values.

**Done when**
- [ ] Hand test on macOS shows changing pressure values while drawing.
- [ ] Windows result recorded (working, or fallback: pressure is macOS-only, pen behaves as mouse elsewhere).
- [ ] Linux result recorded as not tested or as measured.
