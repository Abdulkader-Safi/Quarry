# Progress

Spec: `docs/spec/06-build-spec.md`. Board: `research/Quarry-Todo/`.

Building milestone by milestone per spec §13. Don't start the next milestone with a failing box.

## Milestones

| M | Scope | State | Notes |
|---|-------|-------|-------|
| M0 | De-risk: workspace, CI, viewport, pen, skin, PSD | in progress | |
| M1 | Core, shell, command server | in progress | core, file, autosave, settings, server, MCP done; app shell waits for the Figma design |
| M2 | Tiles and linked images | not started | |
| M3 | Mesh editing, UVs, scene | not started | |
| M4 | Painting | not started | |
| M5 | glTF export and import | not started | |
| M6 | Rigging | not started | |
| M7 | Animation | not started | |
| M8 | Agent experience | not started | |
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

