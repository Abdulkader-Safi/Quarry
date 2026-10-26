---
id: "m0-pen-pressure-spike-2026-10-26"
status: "review"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T16:10:00.000Z"
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

## Progress 2026-10-26

- macOS: `crates/quarry/src/pen.rs` installs an NSEvent local monitor (`objc2-app-kit` 0.3.2, already in the tree through GPUI) on mouse down/drag/up, tablet-point and pressure events. The spike window shows `pressure 0.000 (source)` top left, where source is mouse, pen or trackpad (Force Touch).
- Checked here: the monitor installs, the window keeps 100 fps, the readout draws. No tablet on this Mac, so values are not yet seen moving.
- Left for review: `cargo run -p quarry --release`, draw with the pen (or press hard on the trackpad) and watch the value change.
- Windows and Linux: octotablet not wired yet; no Windows machine or Wacom available in this session. Record the result, or take the fallback (pressure macOS-only, pen as mouse).
