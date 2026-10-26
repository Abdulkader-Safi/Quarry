---
id: "m1-viewport-camera-crosshair-2026-10-26"
status: "in-progress"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T21:00:00.000Z"
labels: ["m1", "render", "ui"]
order: 18
---

# M1 · Viewport, camera, display toggles and 3D crosshair

FR-VIEW-01 to FR-VIEW-05. Bring the M0 spike into the real app.

**Build**
- `quarry-render` wired into the viewport element: mesh draw, grid, overlays, picking buffer.
- Camera: middle-drag orbit, middle-drag pan, wheel zoom; trackpad two-finger orbit, pinch zoom, Shift + two-finger pan; keyboard moves. Perspective and orthographic; front, side, top, isometric presets. Frame selection and frame all.
- Display toggles: wireframe, textured, vertex colour, weights, bones, grid, backface culling — persisted per project.
- 3D crosshair: WASD/arrow movement in the view plane, snaps to grid, vertex or object; tilt and anchor stubs for tiles and bones.

**Done when**
- [ ] Every camera move works on a Mac trackpad with no mouse attached.
- [ ] Iso preset shows a cube with three equal visible faces.
- [ ] Frame one far vertex: it is centred.
- [ ] Snap to a vertex, add a bone (M6): the bone head sits on that vertex.
