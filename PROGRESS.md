# Progress

Spec: `docs/spec/06-build-spec.md`. Board: `research/Quarry-Todo/`.

Building milestone by milestone per spec §13. Don't start the next milestone with a failing box.

## Milestones

| M | Scope | State | Notes |
|---|-------|-------|-------|
| M0 | De-risk: workspace, CI, viewport, pen, skin, PSD | in progress | built and measured on the M2 Pro; Cairn, Blender, PSD files, pen, M1 Air and Windows/Linux checks need Safi |
| M1 | Core, shell, command server | in review | core, file, autosave, settings, server, MCP and the app shell from the Figma design done; one keymap with rebinding and a Blender-style preset |
| M2 | Tiles and linked images | in review | commands, linked art, live refresh (~135 ms), UI, drag and drop; optional block brush, sticky walls, random palette and external editor done |
| M3 | Mesh editing, UVs, scene | in review | commands and UI done, gizmo, box select, mirror extrude; optional inset, subdivide, bevel, knife, loop select, shading, UV pack and mirrored duplicates done |
| M4 | Painting | in review | paint commands and UI, pen pressure in the app (macOS); optional vertex colours and AO bake done; tablet check needs Safi |
| M5 | glTF export and import | in review | export and import (goldens validate 0/0/0), Export dialog; optional presets and OBJ done; Cairn, Blender and Kenney checks need Safi |
| M6 | Rigging | in review | rig commands, skinned export (arm golden 0/0/0), Rig panel with pose and rest modes; Cairn and Blender checks need Safi |
| M7 | Animation | in review | clips, keys, playback, export with IK bake (worst 0.19°), timeline UI; optional onion skin, mirror, curves and sprite sheets done; Cairn checks need Safi |
| M8 | Agent experience | in review | query paging, MCP screenshots, guide, permissions, scripted session (6 s), samples, Connect dialog; optional auto-rigs done |
| M9 | Hardening and release | in progress | no-network check, corrupt-file test, bench (viewport ~70 fps, cold start <0.5 s on the M2 Pro), packaging and release workflow done; tag, M1 Air numbers and accessibility pass (with the new design) left |

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
| Editor viewport, 100,800 tris, 50 bones, 5 skinned meshes playing, 1600x1000 | 14 ms/frame, about 70 fps | 60 fps |
| Same at 2360x1600 (the viewport on a Retina screen) | 14.5 ms/frame, about 69 fps | 60 fps |
| Cold start to first frame (release, empty start screen) | 223 to 466 ms over 3 launches | < 2 s |

The viewport row times what the app does per frame: advance the clip, skin on the CPU, render
offscreen, read back, swap to BGRA. It leaves out the upload of that image to GPUI, which the
benchmark cannot reach. During playback the app redraws only when the clip time changes (30 fps
by default), so the real budget is 33 ms. The 60 fps margin here is thin: the M1 Air may land
under 60, and the place to look first is the CPU skinning and readback in `quarry-render`.

Startup reads settings, the recent list's thumbnails, and one listing of the autosave folder (to
offer crash recovery). Nothing else.

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
- 2026-10-27: **Where things stand.** Every Must requirement's commands, data, file format, export and
  tests are built and pass headless (CI on three OSes; five goldens validate with no issues). What
  remains for v1: the GPUI editor (start screen, panels, viewport input, timeline, paint and UV
  panels, Stop pill, Connect dialog) from Safi's Figma design; the hand checks in Cairn, Blender,
  Affinity/Photoshop, with a pen and on the M1 Air; and the optional Should/Could cards.
- 2026-10-27: Command error messages are written inline in each command, not in one strings file.
  They reach agents as much as people. The UI's own strings go in one file when the shell is built;
  moving the command messages there is part of the accessibility pass.

- 2026-11-19: `rmcp` bumped from the spec's `=3.7.0` to `=3.9.0` (stdio only, no API change) for
  RUSTSEC-2026-0462, the unbounded response buffering in its streamable-HTTP clients. Those client
  features are not enabled here, but the advisory covers the whole crate. `async-executor` moved
  1.14.0 → 1.14.1 because 1.14.0 was yanked. `chrono` (RUSTSEC-2026-0543, unmaintained) is added to
  the deny ignore list: it comes in through GPUI and there is no upgrade path we control.
- 2026-11-19: GitHub retired the macOS 14 runners on 2026-11-02, so the CI matrix is
  `macos-15` (Apple Silicon) now, and the release build uses `macos-15` plus `macos-15-intel`.
  The spec text still names `macos-14`; the current workflow files are the truth.
