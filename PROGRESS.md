# Progress

Spec: `docs/spec/06-build-spec.md`. Board: `research/Quarry-Todo/`.

Building milestone by milestone per spec §13. Don't start the next milestone with a failing box.

## Milestones

| M | Scope | State | Notes |
|---|-------|-------|-------|
| M0 | De-risk: workspace, CI, viewport, pen, skin, PSD | in progress | |
| M1 | Core, shell, command server | in progress | core, file, autosave, settings, server, MCP done; app shell waits for the Figma design |
| M2 | Tiles and linked images | in progress | commands, linked art and live refresh (~135 ms) done headless; UI waits for the design |
| M3 | Mesh editing, UVs, scene | in progress | all commands done headless (primitives, selection, edits, mirror, scene, UV); UI waits for the design |
| M4 | Painting | in progress | paint commands, 3D strokes and pressure sizing done headless; UI waits for the design |
| M5 | glTF export and import | in progress | export and import done headless (goldens validate 0/0/0); Cairn, Blender and Kenney checks need Safi |
| M6 | Rigging | in progress | all rig commands and skinned export done headless (arm golden 0/0/0); Cairn and Blender checks need Safi |
| M7 | Animation | in progress | clips, keys, auto-key, playback, crossfade, sampled export with IK bake done headless (worst 0.19°); UI and Cairn checks left |
| M8 | Agent experience | in progress | query cap and paging, screenshots as MCP images, guide, permissions, scripted session (6 s) done; samples, Stop pill and Connect dialog left |
| M9 | Hardening and release | not started | |

## M0 measurements

| Check | Target | macOS (M1, 8 GB) | Windows | Linux |
|---|---|---|---|---|
| Viewport fps, 100k tri / 50 bones / 5 skinned @1600x1000 | 60 / ≥30 | 100 fps, M2 Pro 16 GB (see note) | | |
| Input latency | < 50 ms | ~10 ms click-to-frame frontmost (+≤10 ms to display), M2 Pro (see note) | | |
| Pen pressure | values arrive | NSEvent monitor + on-screen readout built; hand test pending | octotablet not wired, not tested | not tested |
| PSD composite | matches PNG | reader + `xtask psd-check` ready; Affinity and Photoshop files not yet checked | | |
| 3-bone GLB in Cairn | bends, 0 validator errors | validator: 0 errors, 0 warnings; Cairn and Blender not yet checked | | |

Note on the fps row: measured on the development machine (MacBook Pro M2 Pro, 16 GB, 100 Hz
display), not yet on the M1 Air 8 GB the spec targets. Presentation is capped by the display:
100 fps when the window is frontmost, 25 fps when macOS occludes/throttles a background window.
Renderer-only cost, measured offscreen with a submission fence: 0.10 ms/frame at 100k tris
(1.6M tris → 0.80 ms/frame), so the scene leaves the whole frame budget free.

Note on latency: `--latency-check` posts 30 left clicks and measures from each event's
timestamp to the end of the next frame. Frontmost at 100 Hz: 8.8 to 11.2 ms, so under 21 ms
once the display shows the frame. Throttled in the background at 25 fps: about 40 ms. One
outlier of 88 ms at the moment the window came forward. Synthetic clicks skip the HID path,
so real clicks add a few ms; clicking in the window prints `click_to_frame_ms` for a hand check.
Both rows still need a run on the M1 Air 8 GB.

## M9 measurements

`cargo run --release -p xtask -- bench` (100,800 triangles: 450 spheres), MacBook Pro M2 Pro 16 GB,
2026-10-27. The M1 Air 8 GB numbers are still to be taken.

| Step | Time | Target (NFR-01) |
|---|---|---|
| Save .quarry (0.95 MB) | 66 ms | < 2 s |
| Open .quarry | 13 ms | < 2 s |
| Export .glb (6.1 MB) | 52 ms | < 2 s |

No network (NFR-03): `cargo run -p xtask -- no-network`, also a CI job. `quarry` and `quarry-mcp`
link no HTTP client, TLS or WebSocket crate, and Quarry's code opens no TCP or UDP socket. GPUI
brings the `http` types crate for its client trait; no client is linked. `quarry-mcp` links tokio's
`net` feature because rmcp's stdio transport enables it; it is not used.

## Decisions and deviations

- 2026-10-26: Spec pinned to `docs/spec/` (build spec §4). `research/spec/` keeps the reference screenshots only.
- 2026-10-26: Every crate pin in §2 checked against crates.io; all exact versions exist. Nothing upgraded.
- 2026-10-26: **Deviation from spec §6.3.** `Window::paint_surface` does not accept a BGRA
  CoreVideo buffer. GPUI's macOS renderer asserts `kCVPixelFormatType_420YpCbCr8BiPlanarFullRange`
  and samples the two NV12 planes directly. The viewport therefore renders BGRA with wgpu, converts
  to limited-range BT.709 NV12 into an IOSurface-backed `CVPixelBuffer` (two fullscreen passes
  wrapped around the IOSurface planes with `wgpu::hal::metal::Device::texture_from_raw`), and hands
  that buffer to GPUI. Still zero-copy at the display step; the cost is one 1600x1000 conversion
  pass pair (~0.07 ms). Checked with `--present-check`: mean channel error 1.35/255 against the
  scene target, worst 91/255 at colour edges (2x2 chroma subsampling, expected). GLES/Vulkan paths
  on Windows and Linux are unaffected — those windows go through a normal swapchain, no NV12.
- 2026-10-26: PSD with no composite (saved without Maximize Compatibility) shows "This PSD has no
  flattened image. Re-save it with Maximize Compatibility turned on." Detected as a layered file
  whose composite is one flat colour.
- 2026-10-26: **M1 core crates started while M0 hand checks are pending.** The open M0 items (Cairn and
  Blender loads, PSD files, pen, M1 Air numbers, Windows/Linux runs) only affect the app crate, and
  principle 5 keeps everything below it GPUI-free. The app shell waits for the Figma design.
- 2026-10-26: Added `project.recover(autosave)` to the command list. FR-PROJ-04 needs a way to load an
  autosave while keeping the project's own file path, and every change goes through a command.
- 2026-10-26: Tilesets and textures share one `images` arena with a `role` field (spec lists two
  lists). Same data, one lookup by name or ID.
- 2026-10-26: **Backend of later milestones goes ahead of the M1 app shell.** Safi has a Figma design
  for the app; the GPUI shell, panels and input wait for it. Meanwhile each milestone's commands,
  ops, io and tests are built headless (M2 tiles next), so the UI work only has to wire them up.
  The M1 gate (exit demo in the open window) stays open until the shell lands.
- 2026-10-26: Added `art.watch(image, watch)`. The live-refresh card asks for a per-image watch
  toggle that is saved, and every user-facing action is a command.
- 2026-10-26: Tile cells are 1 m with cell (0, 0) at the crosshair; tiles go into a mesh object named
  "Tiles" unless another is named. Tileset padding is both the margin and the gap between tiles.
- 2026-10-26: FR-MESH-04's check says a box top extrude gives "5 more faces". A region extrude (Blender's
  behaviour) moves the top and adds 4 side quads, 10 faces in all; that is what is built and tested.
- 2026-10-27: **Agent write sandbox tightened** (security review). Agents could open a project anywhere
  (reads were allowed) and then `project.save` into that folder. Now an agent may only open projects
  inside the folders it can write to, `project.save` checks the file it would write, and
  `project.recover` is human-only. Agents can still read images from any path.

