---
id: "m0-viewport-spike-macos-2026-10-26"
status: "in-progress"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T10:03:06.000Z"
labels: ["m0", "render", "spike"]
order: 3
---

# M0 · Viewport spike: GPUI + wgpu on macOS

The riskiest decision in the project. Settle it with numbers.

**Build**
- GPUI window (gpui-kit `=0.8.0`) with a custom element that shows a wgpu 30.0.1 offscreen render.
- macOS path: `Window::paint_surface` + IOSurface-backed texture via wgpu-hal Metal interop, zero copy.
- Scene: 100k triangles, 5 skinned meshes, 50 bones, 1600x1000, one bone animating.
- Measure: sustained fps, frame time, click-to-render input latency.

**Done when**
- [ ] 60 fps sustained and input latency under 50 ms on the M1 MacBook Air 8 GB.
- [ ] Numbers and the exact interop approach written into `PROGRESS.md`.
- [ ] If it misses: record the failure, switch the `quarry` app crate to egui 0.37 + egui-wgpu, and note it — that is the only pre-approved swap.
