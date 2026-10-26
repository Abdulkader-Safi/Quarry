---
id: "m1-viewport-camera-crosshair-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T22:30:00.000Z"
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

## Progress 2026-10-26

- `quarry-core::camera::OrbitCamera`: front, side, top and iso presets, orbit, pan, zoom, frame. Iso shows a cube's three faces equal (test); framing a far point centres it (test).
- `Document.view` (camera, crosshair, display toggles) is saved with the project and never enters undo; undo and redo keep the current view.
- Commands: `view.camera` (preset, position/target, ortho), `view.frame` (objects or all), `view.cursor` (position, snap to grid, an object's nearest vertex or its origin), `view.display` (all seven FR-VIEW-05 toggles). `mesh.add_primitive` lands on the crosshair.
- `quarry-render`: `snapshot()` (world-space batches per image) and `DocumentRenderer` (textures with each image's filter, grid with X/Z axes, backface culling and textured toggles, RGBA/PNG readback). `thumbnail_png` writes the 256 px iso `thumbnail.png` into saved projects.
- Left for the app (Figma design): the viewport element, mouse/trackpad/keyboard camera input, the crosshair overlay, and the wireframe, vertex colour, weights and bones toggles in the renderer.
