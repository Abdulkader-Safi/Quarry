# Tech decisions

Status: signed off 2026-10-26

For each decision: what was chosen, what else was considered, and why, traced to a requirement in [[Projects/Games/quarry/spec/03-requirements|Stage 03]] or the mission and users in Stages 01 and 02. Versions and licences checked on crates.io on 2026-10-26.

> **Constraints going in:** Rust, GPUI for the app UI (Safi's call, same as mdk). MIT-compatible dependencies only (NFR-09). macOS, Windows and Linux at v1 (NFR-05). No network (NFR-03). One developer working with Claude Code. $0 running cost. Output must load in Cairn, which imports glTF/GLB only and skins on the CPU. Patterns and pins reused from the signed-off mdk spec (`Projects/Personal/md-kanban/gpui-rebuild-spec/04-tech-decisions.md`).
> **Changes:** 2026-10-26: removed every reference to the product this was modelled on, at Safi's request. No decision changed.
> 2026-10-26: Safi first picked Rust + wgpu + egui, then GPUI. GPUI has no portable way to show a 3D viewport, so the decision is GPUI with an M0 viewport spike and egui as the written fallback (see "3D viewport inside GPUI").

## Overall architecture
- **Choice:** a Cargo workspace. Everything that is not UI lives in library crates with no GPUI dependency, so it is tested with plain `cargo test` and survives a UI swap to egui.
  - `quarry-core`: the document (scene, meshes, tilesets, skeletons, clips, markers), the command registry, undo history. No I/O, no GPU.
  - `quarry-ops`: mesh, UV, tile, paint, weight, IK and animation operations as pure functions over the document.
  - `quarry-render`: the wgpu renderer. Draws the viewport and screenshots into an offscreen texture. Knows nothing about GPUI.
  - `quarry-io`: `.quarry` zip read and write, glTF/GLB import and export, PNG and PSD loading, file watching.
  - `quarry-server`: the local JSON command server (socket) and headless mode.
  - `quarry-mcp`: a small binary that speaks MCP over stdio and forwards to `quarry-server`.
  - `quarry` (app): GPUI views, panels, input, the viewport element.
- **The keystone:** one command registry in `quarry-core`. Every action marked (A) in Stage 03 is one `Command` type with a name, a description and a typed input struct that derives a JSON schema. The UI buttons, shortcuts, the command search, the socket server and the MCP tools all dispatch the same commands. That is how FR-MCP-01 ("every editor action is a tool") holds by construction, and how FR-MCP-03 (one undo step per agent edit) and FR-MCP-04 (a failed command changes nothing) are enforced in one place.
- **Considered:**
  - One crate: simpler at first, but ties the document to GPUI and makes the egui fallback a rewrite.
  - MCP inside the app process: the app would have to be launched by the MCP client, which breaks "agent drives the window Safi already has open" (FR-MCP-05).
- **Why:** FR-MCP-01, FR-MCP-03, FR-MCP-04, FR-MCP-05, FR-STATE-01, and the viewport risk below.

## UI framework
- **Choice:** `gpui-kit = "=0.8.0"` (2026-10-19, Apache-2.0), which pins `gpui-pre =0.3.9` (a Zed snapshot). We depend on GPUI only through the kit, as in mdk.
- **Considered:**
  - egui 0.37.0 + egui-wgpu (2026-10-15, MIT/Apache): first-class 3D in a panel on all three platforms. The fallback if M0 fails.
  - gpui-ce fork with its wgpu renderer: real GPU compositing on macOS, Windows still an open PR (#313, 3 Oct 2026), and gpui-kit does not build on it.
  - Raw Zed git dependency: newest GPUI, but gpui-kit may not compile against it.
- **Why:** Safi's choice, about 70 ready components (tabs, tree via list, command palette, dialogs, toasts, sliders, inputs, menus), and his mdk experience. Panels, timeline, scene tree and settings are 2D UI where GPUI is strong (FR-UI-01 to FR-UI-05).
- **Landmines:**
  - GPUI is pre-1.0 and breaks between snapshots. Upgrade the kit only on purpose, in its own commit.
  - mdk pinned gpui-kit 0.7.1; Quarry starts on 0.8.0. Do not share a workspace with mdk.
  - No timeline, curve editor or UV editor component exists. Those are custom GPUI elements drawn with `canvas` and paths.

## 3D viewport inside GPUI
- **Choice:** `quarry-render` draws with its own wgpu device into an offscreen texture. A custom GPUI element shows it:
  - **macOS:** render into an IOSurface-backed texture and hand it to GPUI's `Window::paint_surface(bounds, CVPixelBuffer)`. No CPU copy.
  - **Windows and Linux:** copy the frame back to the CPU (double-buffered staging buffers, async map) and paint it as an image. Render only when something changed, and at the element's real pixel size.
- **Considered:**
  - gpui_interop (zero-copy dma-buf on Linux, D3D12 on Windows): Apache-2.0 but experimental, 1 star, own GPUI core, no macOS.
  - GPUI-3D template (shares GPUI's device): 3 stars, Linux untested, vendored fork.
  - A native child window beside GPUI: no copies, but focus, clipping, DPI and resize become per-platform work.
- **Why:** NFR-01 (60 fps, 100k triangles, M1 8 GB) is measured on the Mac, and the Mac path has no copy. The readback path works today on Windows and Linux with no fork.
- **Landmines:**
  - The gpui-kit discussion #2594 (Jul 2026) calls CPU frames "a workable prototype, not the architecture I would keep for a 60 FPS editor". Measured in M0 on Windows and Linux.
  - `paint_surface` is `#[cfg(target_os = "macos")]` and takes a `CVPixelBuffer`; wgpu has no public IOSurface export, so the Mac path needs `wgpu-hal` Metal interop (unsafe) to create the texture over an IOSurface.
  - GPUI repaints on its own schedule. The viewport element must request a redraw only when the scene, camera or playback changes.
  - **Fallback:** if M0 misses its exit criteria, swap the `quarry` app crate to egui + egui-wgpu. Every other crate is unchanged.

## Input: trackpad, mouse, pen
- **Choice:**
  - Trackpad pinch: GPUI's `on_pinch` (`PinchEvent`), present in gpui-pre for macOS, Windows (direct manipulation) and Linux (X11, Wayland). Two-finger orbit from scroll events, Shift+scroll to pan (FR-VIEW-01).
  - Pen pressure on macOS: an `NSEvent` local monitor via `objc2` 0.6.5 (MIT) reading `pressure` on mouse-drag events. GPUI's own `MousePressureEvent` is Force Touch trackpad pressure only, not tablet pressure.
  - Pen pressure on Windows and Linux: `octotablet` 0.1.0 (MIT; Windows Ink and Wayland tablet protocol).
- **Considered:** SDL3's pen API: would mean a second windowing layer under GPUI. winit tablet support: not shipped.
- **Why:** FR-PAINT-03, FR-RIG-03 (pressure brushes), FR-VIEW-01.
- **Landmines:** octotablet's last release was March 2024 with about 2,500 recent downloads. If it fails in M0, see open question 1.

## Document model and meshes
- **Choice:** our own polygon mesh in `quarry-core`: vertices with position and up to 4 bone weights; faces of 3 or 4 corners, each corner with its own UV and vertex colour; per-face texture, smoothing group and tile ID. IDs are `u32` slot indices with generation counters. Math with `glam` 0.34.1 (MIT/Apache).
- **Considered:** half-edge libraries: none current and small; half-edge makes loop select and extrude easier but every tile placement slower to write. Tiles are quads, and everything else is quads and triangles.
- **Why:** tiles are quads with their own UVs (FR-TILE-03), FR-MESH-05 merge by distance and FR-EXP-05 dedup need per-corner data, and FR-RIG-05 caps weights at 4.
- **Landmines:** loop select (FR-MESH-07) needs adjacency; build it on demand per object and cache it until the mesh changes.

## Undo and redo
- **Choice:** command-level undo. Each command returns an inverse, or for heavy ops (bake, auto weights, import) a snapshot of the objects it touched. One history shared by the human and the agent, labelled by source, at least 200 steps (FR-STATE-01, FR-MCP-03).
- **Why:** one mechanism, and a stopped command (FR-MCP-07) restores its own snapshot.

## Project file (.quarry)
- **Choice:** a zip via `zip` 9.x (MIT). `project.json` (deflate) via `serde_json`; mesh buffers as raw little-endian binary (deflate); images as PNG (stored, already compressed). Writes go to a temp file, `fsync`, then rename (as in mdk). A `format_version` field and a migration step on open.
- **Considered:**
  - YAML: rapidyaml's benchmarks show rapidjson parsing at about 1,740 MB/s against yaml-cpp at about 23 MB/s; JSON wins on speed and simplicity (Safi's question, Stage 03).
  - SQLite single file: queryable, but images and buffers are blobs anyway and git diff is lost either way.
- **Why:** FR-PROJ-01, FR-PROJ-02, NFR-01 (open or save 100k triangles in under 2 s), NFR-04.
- **Landmines:** `zip` 9.1.2 was published the day of this check. Pin an exact version that is at least two weeks old at build time.

## Autosave, backups, settings
- **Choice:** autosave every 2 minutes on a background thread from a document snapshot to `<app data>/quarry/autosave/<project hash>.quarry`; backups in the same folder, last 10. Settings and recent files in `config.json` under `directories` 6.0.0 (MIT/Apache) paths, written atomically. Reused from mdk.
- **Why:** FR-PROJ-03 to FR-PROJ-06, FR-UI-05.

## Images, PSD and live refresh
- **Choice:** `image` 0.25.10 for PNG. `psd` 0.3.5 (MIT/Apache) for the flattened composite of PSD files. `notify` 8.2.1 + `notify-debouncer-full` 0.7.0 at 100 ms debounce, with own-write suppression by content hash (mdk pattern). External editor launched with the OS "open with" call.
- **Why:** FR-ART-01 to FR-ART-06, NFR-02 (under 1 s).
- **Landmines:** `psd` was last released January 2024. Affinity and Photoshop only write a composite image when "maximize compatibility" is on; without it the crate may return blank. Test PSDs from Affinity and Photoshop in M2; if they fail, show "Re-save with Maximize Compatibility" rather than guessing.

## Renderer
- **Choice:** wgpu 30.0.1 (MIT/Apache). Forward renderer: unlit or simple Lambert, texture times vertex colour, nearest or linear sampler, alpha cutout for tiles, GPU skinning in the vertex shader (50 bones), overlays for wireframe, weights ramp, bones, grid, crosshair, selection. One pipeline set, no PBR. Screenshots render the same pipeline to an offscreen target of any size (FR-VIEW-06).
- **Considered:** reusing Cairn's C++/Vulkan renderer through FFI: matches the game exactly, but drags C++ into a Rust workspace for a flat-shaded preview.
- **Why:** FR-VIEW-05, FR-VIEW-06, NFR-01. Pixel-art scenes need no lighting model beyond what Cairn's PBR shows anyway.
- **Landmines:** GPUI on Linux uses wgpu 29 internally (`gpui-pre-wgpu`); Quarry's own device on wgpu 30 is separate, so the versions do not need to match unless a zero-copy path is added later.

## Transform gizmo
- **Choice:** `transform-gizmo` 0.11.0 (MIT/Apache), its framework-agnostic core: it returns drawable geometry and hit results; `quarry-render` draws them.
- **Considered:** our own gizmo: about a week of work for translate, rotate, scale with snapping.
- **Why:** FR-MESH-03.
- **Landmines:** its maintained integrations are egui and Bevy; the core API may assume their coordinate conventions. Checked in M1.

## Rigging, IK and animation
- **Choice:** our own code in `quarry-ops`. Auto weights by distance to bone segments with a falloff, then top-4 and normalise (FR-RIG-02, FR-RIG-05). Analytic two-bone IK with a pole vector (FR-RIG-06). Clips as per-bone TRS tracks with step, linear and eased keys (FR-ANIM-03); crossfade preview as a linear blend of two sampled poses (FR-ANIM-08).
- **Considered:** heat-diffusion auto weights (Blender's method): better results, much more code. Revisit if distance weights look bad on the sample robot.
- **Why:** each piece is small and testable without a GPU; no maintained crate does game-style rig editing.

## glTF import and export
- **Choice:** import with `gltf` 1.4.1 (MIT/Apache). Export by building `gltf-json` 1.4.1 structures and writing our own GLB container (12-byte header, JSON chunk, BIN chunk). Export pipeline: dedup vertices on position, normal, UV, colour and weights (FR-EXP-05); sample every clip at the project fps into LINEAR keys, keep STEP keys, drop keys a straight line predicts within 0.01 degrees or 0.0001 m (FR-ANIM-10); bake IK to rotations (FR-ANIM-05); NEAREST sampler when set (FR-EXP-03); markers and flags into node `extras.quarry` (FR-EXP-04).
- **Considered:** writing glTF through a JS tool (glTF-Transform): best-in-class optimisation, but adds Node to a Rust app.
- **Why:** FR-EXP-01 to FR-EXP-05, FR-IMP-01, NFR-07.
- **Landmines:** `gltf` was last released May 2024; it is stable and widely used but will not get new extensions. Validate every export in CI with the Khronos glTF Validator (Apache-2.0, run from npm in CI only, not shipped). Test every skin against Cairn by hand at the end of M5 and M6, since Cairn is private and cannot run in CI.

## AI control (command server and MCP)
- **Choice:**
  - `quarry-server`: newline-delimited JSON over a local socket (Unix domain socket on macOS and Linux, named pipe on Windows) via `interprocess` 2.4.7 (0BSD/Apache). Same message shape as Cairn: `{"command": ..., ...}` in, `{"ok": true, "result": ...}` or `{"ok": false, "error": ...}` out.
  - `quarry-mcp`: `rmcp` 3.7.0 (Apache-2.0, the official MCP Rust SDK) over stdio. On start it connects to the running Quarry's socket; if none is listening, it starts `quarry --headless` and connects to that (FR-MCP-05). Tool list and JSON schemas are generated from the command registry with `schemars` 1.2.3 (MIT), so a new command is a new tool with no extra code (FR-MCP-01).
  - Screenshots return as MCP image content (PNG, 768x768 default, NFR-08).
  - The server rejects writes outside the project folder and the excluded commands (delete files, settings, quit) for agent connections (FR-MCP-06).
- **Considered:** a TCP port like Cairn's 47800: any local process on any user account could connect; a per-user socket path is tighter. MCP in TypeScript: second language for no gain now that rmcp exists.
- **Why:** FR-MCP-01 to FR-MCP-10, NFR-03, NFR-08.
- **Landmines:** rmcp moves fast (3.7.0 on 2026-10-22). Pin exactly.

## Packaging and distribution
- **Choice:** `cargo-packager` 0.11.8, one `Packager.toml`: macOS `.app` + `.dmg` (aarch64 and x86_64), Windows NSIS `.exe` (x64), Linux AppImage (x64). Unsigned in v1. The `quarry-mcp` binary ships inside each package; Help > Connect Claude shows its path (FR-MCP-08). Reused from mdk.
- **Landmines:** unsigned apps need right-click Open on macOS and "Run anyway" on Windows. README says so.

## CI/CD and testing
- **Choice:** GitHub Actions on `macos-14`, `windows-latest`, `ubuntu-22.04`: `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo-deny` licence check (fails on GPL or AGPL, NFR-09), Khronos glTF Validator on the golden exports, and the MCP coverage check (FR-MCP-01). Renderer tests run headless on Linux with Mesa's software Vulkan (lavapipe). Reused from mdk.

## Fonts, icons, theme
- **Choice:** gpui-kit's built-in Lucide icons; Geist Sans (OFL) for UI and the system monospace for numbers; a custom light and dark theme decided at design time, not Safi's social brand. No visual parity target, unlike mdk.

## Reused from existing work
- mdk spec (`gpui-rebuild-spec/04`): gpui-kit pinning rules, atomic writes, `notify` watcher with own-write suppression, `directories` config, cargo-packager setup, the three-OS CI and cargo-deny.
- Cairn: the JSON command message shape and the idea that every editor action is a command (cairn.safi-studio.com/agents).

## Cost
$0 a month at launch and at the NFR scale. No servers, no APIs, no paid licences. GitHub Actions minutes are free for public repos; while private, the free tier (2,000 minutes a month) covers about 60 three-OS runs, so run the full matrix on main and PRs only.

## Licensing
Rule (Safi, 2026-10-26): only MIT or other licences that are free to use and never need a paid licence.

Audit on 2026-10-26: `cargo metadata` resolved the full Stage 04 dependency set for all three platforms, **885 crates**. None needs a paid or commercial licence, and none is GPL-only or AGPL. Breakdown:
- 773 are MIT, Apache-2.0, or a choice of the two.
- 112 use other free licences: Zlib, Unicode-3.0, BSD-2/3-Clause, ISC, 0BSD, CC0, Unlicense, BSL-1.0, MIT-0, bzip2-1.0.6, NCSA, OFL-1.1 and Ubuntu-font-1.0 (fonts).
- **Dual-licensed with a GPL or LGPL option:** `self_cell` (Apache-2.0 OR GPL-2.0) and `r-efi` (MIT OR Apache-2.0 OR LGPL). We take the Apache or MIT option; no GPL obligation applies.
- **MPL-2.0 (3 crates):** `dwrote` (Windows fonts, via GPUI), `option-ext` (via `directories`), `cbindgen` (build tool only, not shipped). MPL is free. Its only condition: if we edit those crates' own source files, those edited files stay MPL and their source must be shared. Using them unmodified inside an MIT app needs nothing. Rule: never patch them in place.
- **Network code compiled in:** `rustls` and `hyper-rustls` arrive through dependencies. They are free (Apache/ISC/MIT), but NFR-03 says no network calls, so `cargo-deny` also bans direct use and a test checks the app makes no outbound connections.

Enforced in CI with `cargo-deny`, `deny.toml` allow list:
`MIT, MIT-0, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, CC0-1.0, Unlicense, BSL-1.0, Unicode-3.0, MPL-2.0, OFL-1.1, Ubuntu-font-1.0, bzip2-1.0.6, NCSA`.
Anything else fails the build: GPL, AGPL, LGPL-only, SSPL, BUSL, Commons Clause, "commercial", or a crate with no licence. A new dependency with a new licence needs Safi's OK before it is added to the list.

Non-code: Geist font (OFL-1.1), Lucide icons (ISC), sample tileset (CC0). The Khronos glTF Validator (Apache-2.0) runs in CI only. Zed's own editor crates (GPL) must not be copied.

Things that would cost money and are deliberately skipped: Apple Developer ID for signing and notarising the Mac app ($99 a year) and a Windows code-signing certificate. v1 ships unsigned; add them only if Quarry is distributed widely.

Full list: `spec/references/licence-audit.md`.

## Top technical risks
1. **3D viewport in GPUI** : no supported portable path. Spike: M0 renders a 100k-triangle skinned scene through the Mac `paint_surface` path and the Windows/Linux readback path at 1600x1000; pass is 60 fps on the M1 8 GB and at least 30 fps on Windows and Linux with input latency under 50 ms. Fail means switch the app crate to egui.
2. **Pen pressure on Windows and Linux** : octotablet is stale. Spike in M0 with a Wacom on Windows.
3. **glTF skins that Cairn plays correctly** : inverse bind matrices, joint order and sampled clips are easy to get subtly wrong. Spike: export a 3-bone bending bar in M0 and load it in Cairn before building the rig UI.
4. **GPUI churn** : pin exactly, upgrade deliberately.
5. **Scope** : about 80 Must requirements for one developer. Milestones put the tile-to-GLB path first so something usable exists early.

---

### Open questions

None.

### Assumptions confirmed

1. **Language and UI : RESOLVED.** Rust with GPUI (via gpui-kit), Safi's choice.
2. **Viewport : RESOLVED.** GPUI + M0 spike, egui + wgpu as the written fallback.
3. **Cairn reuse : RESOLVED.** No Cairn code is reused (Cairn is C++ on GLFW and Dear ImGui). Only its command message shape.
4. **Licences : RESOLVED.** MIT or other free licences only, never a paid licence. 885 resolved crates audited 2026-10-26, all pass; cargo-deny allow list above.
5. **Pen pressure fallback : RESOLVED.** If octotablet fails on Windows or Linux in M0, pressure is macOS-only in v1 and the pen acts as a mouse elsewhere. FR-PAINT-03 acceptance is tested on macOS.
6. **Viewport fallback trigger : RESOLVED.** M0 numbers decide (60 fps on the M1, 30 fps elsewhere). Mac pass with Windows or Linux at 20 to 30 fps keeps GPUI as a logged known limit.
7. **Intel Mac : RESOLVED.** Build x86_64, test on Apple Silicon only.
