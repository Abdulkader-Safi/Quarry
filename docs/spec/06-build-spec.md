# Build spec: Quarry

> **This is the hand-off document for Claude Code.** It is self-contained: build from this file alone. It pulls together the product, users, requirements and tech decisions from stages 1 to 5. Where it says "see Stage N", that is optional background.

Status: draft. Sources: 01 to 05, all signed off 2026-10-26.

## 1. What we're building

Quarry is a desktop low-poly modeler and level builder for macOS, Windows and Linux. It combines tile placement (pixel-art tiles placed on a 3D grid) with free low-poly mesh editing, a pixel paint panel and painting on the model, live refresh of PNG and PSD art saved in Affinity, Krita or Photoshop, weighted skeletal rigging with two-bone IK, and keyframe animation. It exports one self-contained .glb per asset for Safi's own game engine, Cairn (C++20/Vulkan, imports glTF/GLB only, CPU skinning). Every editor action is also an MCP tool, so Claude can model, rig and animate in the same window and the same undo history as Safi. Personal tool: private repo now, MIT later, no accounts, no network.

## 2. Stack (decided, see Stage 4)

| Layer | Choice |
|-------|--------|
| Language | Rust (stable), Cargo workspace |
| UI | gpui-kit =0.8.0 (pins gpui-pre =0.3.9), Apache-2.0 |
| UI fallback | egui 0.37 + egui-wgpu, only if M0 fails |
| 3D viewport | wgpu 30.0.1 offscreen; macOS zero-copy via `Window::paint_surface` + IOSurface texture (wgpu-hal Metal interop); Windows/Linux double-buffered readback |
| Math | glam 0.34.1 |
| Mesh model | own polygon mesh (3 or 4 corner faces, per-corner UV and colour, 4 weights per vertex) |
| Gizmo | transform-gizmo 0.11.0 core |
| Undo | command-level inverses or snapshots in `quarry-core`, one shared history |
| Rigging, IK, animation | own code in `quarry-ops`: distance auto weights, analytic two-bone IK, TRS tracks, export sampling at project fps |
| Input | GPUI pinch and scroll; pen pressure via objc2 0.6.5 NSEvent monitor (macOS) and octotablet 0.1.0 (Windows/Linux) |
| Project file | `.quarry` zip via zip 9.x (pin a version at least two weeks old): project.json (serde_json) + binary mesh buffers + PNGs |
| Images | image 0.25.10 (PNG), psd 0.3.5 (flattened PSD) |
| File watching | notify 8.2.1 + notify-debouncer-full 0.7.0, 100 ms debounce, own-write suppression |
| glTF | gltf 1.4.1 (import), gltf-json 1.4.1 + own GLB writer (export) |
| Command server | interprocess 2.4.7, per-user Unix socket / named pipe, JSON lines |
| MCP | rmcp =3.7.0 over stdio in a separate `quarry-mcp` binary; schemas from schemars 1.2.3 |
| Settings | directories 6.0.0 + serde_json, atomic writes |
| Logging | tracing, rolling file (7 days) + in-app console |
| Packaging | cargo-packager 0.11.8: .app/.dmg (aarch64, x86_64), NSIS .exe, AppImage; unsigned |
| CI | GitHub Actions on macos-14, windows-latest, ubuntu-22.04: fmt, clippy -D warnings, test, cargo-deny, Khronos glTF Validator (npm, CI only), lavapipe renderer tests |
| Fonts and icons | Geist Sans (OFL), system monospace, gpui-kit Lucide icons |

## 3. Architecture and principles

```
 Human                                   Claude (Claude Code / desktop app)
   |  mouse, trackpad, pen, keys            |  MCP over stdio
   v                                        v
+----------------------------+       +-------------------+
| quarry (GPUI app)          |       | quarry-mcp        |
|  panels, timeline, UV,     |       |  rmcp server,     |
|  viewport element          |       |  tools from the   |
|  input -> Command          |       |  registry schema  |
+-------------+--------------+       +---------+---------+
              |                                | JSON lines over local socket
              |   +----------------------------v-----+
              |   | quarry-server (in the app, or     |
              |   | `quarry --headless`)              |
              |   +----------------+-----------------+
              v                    v
        +-------------------------------------------+
        | quarry-core: Document + CommandRegistry   |
        |   + History (undo/redo, source labels)    |
        +------+-------------------+----------------+
               |                   |
     +---------v------+   +--------v---------+   +------------------+
     | quarry-ops     |   | quarry-io        |   | quarry-render    |
     | mesh, UV, tile,|   | .quarry zip,     |   | wgpu offscreen,  |
     | paint, weights,|   | glTF in/out,     |   | screenshots,     |
     | IK, sampling   |   | PNG/PSD, watcher |   | picking          |
     +----------------+   +------------------+   +------------------+
```

**Non-negotiable principles:**
1. **One door to the document** : nothing mutates the `Document` except `CommandRegistry::execute(command, source)`. UI, shortcuts, command search, socket and MCP all call it.
2. **Every action is a command, every command is a tool** : each user-facing action is a typed command with a schemars schema; `quarry-mcp` generates its tool list from the registry. A CI test fails if any command lacks a tool.
3. **Failure changes nothing** : commands run on a working copy and commit as one history entry or discard everything. Cancel restores the pre-command state.
4. **One shared undo history** : human and agent edits land in the same history, labelled by source.
5. **No GPUI below the app crate** : `quarry-core`, `quarry-ops`, `quarry-io`, `quarry-render`, `quarry-server` never depend on GPUI, so the egui fallback swaps one crate.
6. **Local only** : no network calls, no TCP listener, per-user socket; agent writes only inside the open project folder.
7. **glTF is the contract** : every export passes the Khronos validator; skins and clips are checked in Cairn by hand at M5, M6, M7.
8. **Free licences only** : cargo-deny allow list (MIT, MIT-0, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, ISC, Zlib, 0BSD, CC0-1.0, Unlicense, BSL-1.0, Unicode-3.0, MPL-2.0, OFL-1.1, Ubuntu-font-1.0, bzip2-1.0.6, NCSA). Never patch MPL crates in place. A new licence needs Safi's OK.
9. **Pin GPUI** : gpui-kit exact pin; upgrade only on purpose in its own commit.

## 4. Repository structure

```
quarry/
  Cargo.toml              workspace, exact pins
  deny.toml               licence allow list, bans
  Packager.toml           cargo-packager config
  crates/
    quarry-core/          Document, IDs, CommandRegistry, History, command structs
    quarry-ops/           mesh, UV, tile, paint, weights, IK, animation sampling (pure functions)
    quarry-render/        wgpu renderer, offscreen targets, screenshots, picking
    quarry-io/            .quarry zip, migrations, glTF import/export, PNG/PSD, watcher, settings
    quarry-server/        local socket server, headless runner, agent permission checks
    quarry-mcp/           rmcp stdio binary bridging to quarry-server
    quarry/               GPUI app: views, panels, viewport element, input, timeline, UV and paint elements
  xtask/                  validate-gltf, bench, golden updates
  assets/
    samples/              CC0 16 px tileset, sample-robot.quarry
    fonts/  icons/
  tests/goldens/          golden .glb/.gltf exports and reference screenshots
  docs/spec/              this spec (01 to 06)
  PROGRESS.md
```

## 5. Data model

Space is right-handed, Y up, metres (as glTF). Rotations are quaternions, shown as Euler degrees XYZ. Time in seconds; project fps default 30. IDs are u32 slot + generation, written as short strings; every object, bone, clip, marker and image also has a unique name, and every command accepts name or ID. Hard delete; undo and backups recover.

- **Project** : format_version, name, settings (tile_px 16, filter nearest, fps 30, palette_lock true, grid_step 1.0), objects, tilesets, textures, palettes, skeletons, clips, export_presets.
- **Object** : id, name, kind (mesh, instance, marker, layer), parent, transform (T, R quat, S), visible, locked, mesh?, instance_of?, marker_type?, collision, properties (map), skin? (skeleton + weighted or rigid bone).
- **Mesh** : positions, weights (up to 4 bone/weight pairs per vertex, sum 1), faces, mirror axis?.
- **Face** : 3 or 4 corners {vertex, uv in pixels, colour}, image, smooth_group, tile?, hidden.
- **Image** : id, name, role (tileset, texture), size, tile_px, padding, zip_path, source_path?, source_hash, filter.
- **Palette** : name, colours (max 256), weights?.
- **Skeleton / Bone / IkChain** : bones (max 128) with name, parent, head, tail, roll; IK chains with tip bone, target, pole.
- **Clip / Track / Key** : name (glTF animation name), skeleton, length, loop; tracks per bone or IK target and channel; keys with time, value, interp (step, linear, ease_in, ease_out, ease_in_out, bezier), handles?.
- **ExportPreset** : name, path (relative), format (glb default, gltf_separate, obj), selection, scale.
- **HistoryEntry** (memory only) : label, source, time, inverse or snapshot. 200 entries or 512 MB.
- **Settings** (app config) : ui_scale, theme, shortcuts, preset, external_editor, autosave_minutes 2, weight_ramp, recent_files 10, window_state.

`.quarry` zip layout: `project.json`, `meshes/<object>.bin`, `images/<image>.png` (stored), `thumbnail.png`. Full tables: Stage 05.

## 6. API / command surface

Commands are `area.verb`; MCP tool names are `area_verb`. Socket: `{"id","command","args"}` in, `{"id","ok","result"|"error"}` out, plus `{"id","progress"}` lines for long commands. Error codes: invalid_args, not_found, nothing_selected, conflict, out_of_bounds, forbidden, io, cancelled, internal; messages say how to fix.

- **project** : new, open (long), save (long), save_as, info, set_settings.
- **query** (read-only, 4,000-token cap, paged) : scene, object, mesh, selection, skeleton, clip, images, history.
- **view** : camera, frame, cursor, display, screenshot (PNG, default 768x768).
- **select** : mode, by_ids, box, all, none, invert, loop, by_image, by_normal.
- **tile** : import_tileset, brush, plane, place, erase, fill_rect, erase_rect, place_sticky, block_add, block_remove, palette_set, eyedrop.
- **mesh** : add_primitive, transform, extrude, merge, split, delete, make_face, flip_normals, flip_diagonal, subdivide, inset, bevel, knife, mirror_mode, mirror_apply, smooth_group.
- **uv** : transform, mark_seam, unwrap, project_view, snap_pixels, pack.
- **paint** : texture_new, pixels, rect, fill, stroke_3d, palette_set, palette_lock, vertex_color, bake_ao (long).
- **art** : link, unlink, reload, import.
- **scene** : rename, parent, reorder, visibility, lock, object_from_selection, origin_to_cursor, instance_add, marker_add, set_collision, set_props, duplicate, mirror, delete.
- **rig** : skeleton_new, bone_add, bone_edit, bone_delete, bind_auto (long), weight_paint, bind_rigid, ik_add, ik_set, mirror_weights, mirror_bones, pose, pose_reset.
- **anim** : clip_new, clip_edit, clip_duplicate, clip_delete, key_set, key_move, key_delete, key_copy, key_paste, autokey, curve_handles, playhead, play, stop, crossfade_preview, mirror_pose, mirror_clip.
- **export / import** : export.run (long, returns validator report), export.preset_save, export.sprite_sheet (long), import.gltf (long).
- **history / control** : history.undo, history.redo, control.cancel.
- **MCP resources** : `quarry://guide` (units, axes, naming, sample workflow, screenshot after every major step), `quarry://commands`.

Agent permissions: all commands; file writes only inside the open project's folder (canonicalised, symlinks resolved), reads from any given path; no commands exist for deleting files, settings, shortcuts, quitting or opening external editors.

## 7. Screens

- **Start screen** : recent list with thumbnails and missing markers, New (name, tile size), Open, Sample robot, recovery prompt, first-run overlay. States: no projects, missing file, recovery available.
- **Editor window** : top bar (menu, modes Tile/Edit/Paint/Rig/Animate, agent status pill with Stop); viewport element (render, gizmo, crosshair, overlays, grid); side tabs Tileset, UVs, Paint, Scene, Rig; bottom timeline (clip picker, tracks, keys, playhead, controls, graph editor, onion skin, crossfade preview); status bar; command search overlay; history panel; console. States: empty scene hint, empty timeline hint, progress overlay, missing link badge, toasts.
- **Settings window** : shortcuts with conflict warning and presets, UI scale 100 to 200%, theme, external editor, autosave interval, weight ramp.
- **Export dialog** : format (GLB default), selection, scale, path, save as preset, validator summary.
- **Connect Claude dialog** : `quarry-mcp` path, `claude mcp add quarry -- <path>`, desktop app JSON snippet.

## 8. Build plan (milestones)

### M0: De-risk (do this first)
- **Builds:** no feature requirements; proves the risks
- **Tasks:**
  - Cargo workspace with the seven crates as empty shells; CI on macos-14, windows-latest, ubuntu-22.04 with fmt, clippy, test, cargo-deny (allow list from Stage 04).
  - Viewport spike: GPUI window with a custom element showing a wgpu offscreen render of a 100k-triangle scene with 5 skinned meshes (50 bones) at 1600x1000. macOS through `paint_surface` + IOSurface-backed texture; Windows and Linux through double-buffered readback.
  - Pen spike: read tablet pressure on macOS with an objc2 NSEvent monitor; on Windows with octotablet and a Wacom.
  - Skin spike: hand-build a 3-bone bending bar, write it as .glb with our GLB writer, pass the Khronos validator, load it in Cairn and Blender, play its clip.
  - PSD spike: read flattened PSDs saved by Affinity and Photoshop with `psd` 0.3.5.
- **Exit criteria:** 60 fps on the M1 8 GB on the Mac path and at least 30 fps on Windows and Linux with input latency under 50 ms; pressure values arrive on macOS (Windows result recorded); the bar bends correctly in Cairn. Fallbacks: viewport fails, switch the `quarry` app crate to egui 0.37 + egui-wgpu; Windows/Linux pressure fails, pressure is macOS-only (Stage 04 decision); PSD composite missing, show the 'Re-save with Maximize Compatibility' message.

### M1: Core, shell and command server
- **Builds:** FR-PROJ-01, FR-PROJ-02, FR-PROJ-03, FR-PROJ-04, FR-PROJ-06, FR-PROJ-07, FR-VIEW-01, FR-VIEW-02, FR-VIEW-03, FR-VIEW-04, FR-VIEW-05, FR-UI-01, FR-UI-02, FR-UI-03, FR-UI-04, FR-UI-05, FR-STATE-01, FR-STATE-02, FR-STATE-03, FR-STATE-04, FR-STATE-05, FR-MCP-01, FR-MCP-03, FR-MCP-04, FR-MCP-05
- **Also (Should/Could, drop first if time runs short):** FR-PROJ-05, FR-STATE-06, FR-STATE-07, FR-STATE-08
- **Tasks:**
  - `quarry-core`: Document, IDs, CommandRegistry with schemars schemas, History (200 entries or 512 MB).
  - `quarry-io`: .quarry zip read/write with atomic save, format_version, zip-bomb guards; autosave and backups; settings via `directories`.
  - `quarry-server` (local socket, JSON lines, progress lines, cancel) and `quarry --headless`; `quarry-mcp` bridge generating tools from the registry.
  - App shell: start screen with recent list and recovery prompt, editor layout, camera controls incl. pinch, crosshair, display toggles, command search, shortcuts editor with Blender preset, status bar, toasts, progress overlay, confirm dialogs, history panel.
  - Property test: random command sequences then undo all equals the start state. MCP coverage test.
- **Exit criteria:** All M1 boxes ticked; a `project.new` + `project.save` sent through `quarry-mcp` appears in the open window and undoes with one Cmd+Z.

### M2: Tiles and linked images
- **Builds:** FR-TILE-01, FR-TILE-02, FR-TILE-03, FR-TILE-04, FR-TILE-05, FR-TILE-09, FR-ART-01, FR-ART-02, FR-ART-03, FR-ART-04, FR-ART-06
- **Also (Should/Could, drop first if time runs short):** FR-TILE-06, FR-TILE-07, FR-TILE-08, FR-ART-05
- **Tasks:**
  - Tileset import (PNG, PSD), brush picking, plane lock and tilt, place/erase/fill, eyedropper, sticky and block brushes, tile palettes.
  - Image links with embedded copies, notify watcher with own-write suppression, pixel-space UVs on resize, drag-and-drop import, open in external editor.
- **Exit criteria:** A small tiled room built by hand and by an agent through MCP; a PNG saved in Krita updates the model in under 1 s.

### M3: Mesh editing, UVs and scene
- **Builds:** FR-MESH-01, FR-MESH-02, FR-MESH-03, FR-MESH-04, FR-MESH-05, FR-MESH-06, FR-MESH-08, FR-UV-01, FR-UV-02, FR-SCENE-01, FR-SCENE-02, FR-SCENE-03, FR-SCENE-04, FR-SCENE-05
- **Also (Should/Could, drop first if time runs short):** FR-MESH-07, FR-MESH-09, FR-UV-03, FR-SCENE-06, FR-SCENE-07
- **Tasks:**
  - Primitives, selection modes (click, box, lasso, loop), transform-gizmo integration with typed values and snapping, extrude, merge/split/delete, make face, flip, live mirror; bevel, inset, knife, subdivide as Should.
  - UV editor element: transform, seams, unwrap, project from view, snap to pixels, pack.
  - Scene tree with filter, objects from selection, origin to crosshair, instances, markers with properties, collision flag, duplicate and mirror.
- **Exit criteria:** Box-model a crate and a robot torso by hand; every M3 box ticked.

### M4: Painting
- **Builds:** FR-PAINT-01, FR-PAINT-02, FR-PAINT-03, FR-PAINT-04, FR-PAINT-06
- **Also (Should/Could, drop first if time runs short):** FR-PAINT-05
- **Tasks:**
  - 2D pixel editor element with palette strip and tools; 3D painting via texel picking from the renderer; pressure from the M0 input layer; palette lock; new texture; vertex colours and AO bake as Should.
- **Exit criteria:** Paint the robot torso's 32x32 texture in Quarry and finish it in Affinity with live refresh.

### M5: glTF export and import
- **Builds:** FR-EXP-01, FR-EXP-03, FR-EXP-04, FR-EXP-05, FR-IMP-01
- **Also (Should/Could, drop first if time runs short):** FR-EXP-06, FR-EXP-08
- **Tasks:**
  - GLB writer on gltf-json with vertex dedup, NEAREST sampler, node extras for markers/collision/props, separate .gltf option; import with the `gltf` crate keeping hierarchy; export presets; OBJ as Could.
  - Golden exports and the Khronos validator in CI.
- **Exit criteria:** The tiled room and the crate load in Cairn and Blender; validator reports zero errors.

### M6: Rigging
- **Builds:** FR-RIG-01, FR-RIG-02, FR-RIG-03, FR-RIG-04, FR-RIG-05, FR-RIG-06, FR-RIG-07, FR-EXP-02
- **Also (Should/Could, drop first if time runs short):** FR-RIG-08
- **Tasks:**
  - Bones at the crosshair, auto weights by distance with top-4 normalisation, weight brush with pressure and ramp view, rigid binding, two-bone IK with pole, mirror weights and bones, pose mode; skinned export with inverse bind matrices.
- **Exit criteria:** The robot is rigged with 13 bones, posed with IK, and its bind pose exports and loads in Cairn with the skeleton.

### M7: Animation
- **Builds:** FR-ANIM-01, FR-ANIM-02, FR-ANIM-03, FR-ANIM-04, FR-ANIM-05, FR-ANIM-08, FR-ANIM-10
- **Also (Should/Could, drop first if time runs short):** FR-ANIM-06, FR-ANIM-07, FR-ANIM-09, FR-EXP-07
- **Tasks:**
  - Clips, timeline element with tracks and keys, auto-key, interpolation, playback, keyable IK baked on export, crossfade preview, export sampling and key reduction; graph editor, onion skin, mirror clip and sprite sheet as Should.
- **Exit criteria:** The robot's idle and walk export as one .glb and play in Cairn within 0.5 degrees of Quarry. Save `assets/samples/sample-robot.quarry`.

### M8: Agent experience
- **Builds:** FR-MCP-02, FR-MCP-06, FR-MCP-07, FR-MCP-08, FR-VIEW-06
- **Also (Should/Could, drop first if time runs short):** FR-MCP-09, FR-MCP-10, FR-PROJ-08
- **Tasks:**
  - Query commands with paging and the 4,000-token cap; screenshots as MCP image content; agent path checks; Stop button and cancel; Connect Claude dialog; `quarry://guide` and `quarry://commands` resources; samples on the start screen; auto-rig helpers as Could.
  - Scripted agent session test: box, paint, rig, key, export through `quarry-mcp` against headless Quarry.
- **Exit criteria:** Claude builds a textured, rigged, two-clip prop from one prompt in under 10 minutes with no manual repair (Stage 01 success signal).

### M9: Hardening and release
- **Builds:** no feature requirements; proves the risks
- **Tasks:**
  - Performance pass against NFR-01 and NFR-10 on the M1; no-network test; accessibility pass (scale, 13 px, keyboard reach); cargo-packager builds for all three OSes in CI on a tag; README with unsigned-app instructions.
- **Exit criteria:** Every NFR box ticked; tagged build installs and runs on macOS, Windows and Linux.

## 9. Acceptance criteria

### POC (proves the core idea: tiles in, GLB out, agent drives the same editor)

- [ ] **FR-PROJ-01** Create, open, save and save-as a project as one `.quarry` file. *Check:* Save a scene with 2 tilesets, 3 objects, a rig and 2 clips; quit; reopen; every tile, UV, weight, keyframe and image is identical.
- [ ] **FR-PROJ-02** The `.quarry` file is a zip container holding `project.json` (scene, settings, rig, clips), binary mesh buffers, and images as PNG. *Check:* Rename a saved file to `.zip`, unzip it, and `project.json` opens in a text editor and the PNGs open in Krita.
- [ ] **FR-VIEW-01** Orbit, pan, zoom with mouse (middle-drag), trackpad (two-finger orbit, pinch zoom, Shift+two-finger pan) and keyboard. *Check:* Every camera move works on a Mac trackpad with no mouse attached.
- [ ] **FR-VIEW-02** Perspective and orthographic modes, plus front, side, top and isometric presets. *Check:* Isometric preset shows a cube with three equal visible faces.
- [ ] **FR-VIEW-03** Frame selection, and frame all. *Check:* Select one vertex far from origin, press frame; it is centred.
- [ ] **FR-VIEW-04** A 3D crosshair (cursor) that marks where tiles, primitives, bones and markers are placed; moves with keys, snaps to grid, vertices and... *Check:* Snap to a vertex, add a bone; the bone head sits on that vertex.
- [ ] **FR-TILE-01** Import a PNG or PSD as a tileset with a tile size and padding. *Check:* Import a 256x256 PNG at 16 px; the tileset panel shows a 16x16 grid of tiles.
- [ ] **FR-TILE-02** Pick one tile or a rectangle of tiles as the brush. *Check:* Drag-select 2x2 tiles; the brush preview shows all four.
- [ ] **FR-TILE-03** Place tiles on the plane at the crosshair; click to place, drag to paint, right-click to erase, rectangle fill and erase. *Check:* Drag across 5 cells; 5 quads exist, each textured with the brush tile.
- [ ] **FR-EXP-01** Export the scene or selected objects as one self-contained .glb holding every mesh, texture, material, skeleton and clip, like Blender's... *Check:* Export the sample robot; exactly one .glb file is written; it passes the Khronos glTF Validator with zero errors and opens in Blender with mesh, texture, skeleton and all clips.
- [ ] **FR-MCP-01** Every action marked (A) in this file is an MCP tool with a name, a description and typed inputs. *Check:* An automated check compares the list of (A) actions with the MCP tool list; none missing.
- [ ] **FR-MCP-03** Every MCP edit is one undo step in the shared history, labelled as from the agent. *Check:* Agent extrudes a face; the human presses undo once; the extrude is gone and the history shows "Agent: extrude".
- [ ] **FR-MCP-04** A failed tool call changes nothing and returns an error saying what was wrong and how to fix it. *Check:* Call extrude with nothing selected; error says "No faces selected. Call select first."; the scene is unchanged.
- [ ] **FR-MCP-05** Works live against the open window, and headless with no window for batch jobs. *Check:* With the window open, an agent's add_box appears in the viewport. With none open, the same call creates the box in a headless instance and save works.
- [ ] **FR-STATE-01** Undo and redo every scene edit, human or agent, including tileset imports, image refreshes, weight strokes and key edits; at least 200... *Check:* Import a tileset, place 3 tiles, undo 4 times; the tileset is gone too and nothing else changed.

### MVP (shippable v1)

- [ ] **FR-PROJ-03** Autosave every 2 minutes to a side copy, never over the user's file. *Check:* Edit, wait 2 minutes, kill the process; the original file is untouched and the autosave holds the edit.
- [ ] **FR-PROJ-04** After a crash, offer to recover the autosave on next launch. *Check:* Kill the app mid-edit, relaunch, a recovery prompt shows the file name and autosave time; Recover restores the edit.
- [ ] **FR-PROJ-06** Recent projects list on the start screen (last 10). *Check:* Open three projects, relaunch, all three listed most recent first; a moved file shows as missing, not as a crash.
- [ ] **FR-PROJ-07** New project starts with pixel-art defaults: nearest filtering, 16 px tiles, 1 tile = 1 m. *Check:* New project, check settings panel shows those values.
- [ ] **FR-VIEW-05** Display toggles: wireframe, textured, vertex colour, weights, bones, grid, backface culling. *Check:* Each toggle changes the view and persists after reopen.
- [ ] **FR-VIEW-06** Take a screenshot of the viewport from a given camera preset or position, at a given size. *Check:* MCP call for front, side and iso at 768x768 returns three PNGs matching the view.
- [ ] **FR-TILE-04** Lock the placement plane to X, Y or Z and tilt it. *Check:* Lock to X, place tiles; all their normals point along X.
- [ ] **FR-TILE-05** Rotate, flip and mirror the brush. *Check:* Rotate 90 and place; UVs are rotated 90 compared to the unrotated tile.
- [ ] **FR-TILE-09** Eyedrop a tile from the scene into the brush. *Check:* Eyedrop a placed tile; the brush shows that tile with its rotation.
- [ ] **FR-MESH-01** Add primitives at the crosshair: box, wedge, cylinder, cone, sphere, plane, with segment counts. *Check:* Add an 8-segment cylinder; it has 8 side quads and two caps.
- [ ] **FR-MESH-02** Select vertices, edges, faces and objects by click, box and lasso; add, remove, invert, select all. *Check:* Box-select 4 vertices, Shift-click a fifth; 5 are selected.
- [ ] **FR-MESH-03** Move, rotate and scale selections with a gizmo, typed values and snapping. *Check:* Type a move of 0.5 on Y; the selection's centre rises exactly 0.5.
- [ ] **FR-MESH-04** Extrude faces and edges. *Check:* Extrude a box's top face by 1; the box has 5 more faces and the new top sits 1 higher.
- [ ] **FR-MESH-05** Merge vertices (at centre, at crosshair, by distance), split, delete with or without faces. *Check:* Merge by distance 0.001 on two coincident vertices; one remains.
- [ ] **FR-MESH-06** Create a face from 3 or 4 selected vertices; flip normals; flip quad diagonal. *Check:* Select 4 vertices, create face; one quad exists facing the camera.
- [ ] **FR-MESH-08** Mirror modelling across an axis, live, until applied. *Check:* Turn on X mirror, move a vertex on +X; its twin on -X moves to match.
- [ ] **FR-UV-01** UV panel showing selected faces over the texture; move, rotate, scale, flip UVs. *Check:* Rotate a face's UVs 90 in the panel; the viewport texture rotates.
- [ ] **FR-UV-02** Unwrap selected faces with marked seams; project from view; snap UVs to the pixel grid. *Check:* Unwrap a box with 7 seams marked; 6 faces lay flat with no overlap.
- [ ] **FR-PAINT-01** Paint in a 2D texture panel: pencil, eraser, fill, eyedropper, rectangle select, with a palette strip. *Check:* Pencil one pixel at 400% zoom; that exact texel changes and nothing else.
- [ ] **FR-PAINT-02** Paint directly on the model in the 3D viewport; strokes land on the texel under the cursor. *Check:* Paint a line across two faces sharing a seam; the line is continuous in 3D.
- [ ] **FR-PAINT-03** Pen pressure changes brush size for paint (in pixel steps) when a tablet is present. *Check:* Light and heavy strokes with the pen produce 1 px and 3 px lines at max size 3.
- [ ] **FR-PAINT-04** Palette-locked mode: painting snaps to the nearest palette colour. *Check:* With a 16-colour palette, paint with a colour not in it; the texel gets the nearest palette colour.
- [ ] **FR-PAINT-06** Create a blank texture at a given size and assign it to selected faces. *Check:* New 32x32 texture on a box; all faces show it.
- [ ] **FR-ART-01** Link a tileset or texture to a PNG or PSD file on disk; Quarry keeps a copy inside the `.quarry` file and remembers the source path. *Check:* Move the source file away; the project still opens with the embedded copy and marks the link as missing.
- [ ] **FR-ART-02** When a linked file changes on disk, reload it within 1 second, without the user switching to Quarry. *Check:* Save a PNG in Krita while Quarry sits behind it; the model updates in under 1 s.
- [ ] **FR-ART-03** PSD files are read as the flattened image, so Photoshop and Affinity PSD exports work. *Check:* A 3-layer PSD from Affinity shows the same pixels as its flattened PNG export.
- [ ] **FR-ART-04** If the new image has a different size, keep UVs in pixel positions and warn. *Check:* Grow a 64x64 texture to 128x128 by canvas resize; existing faces still show the same pixels and a warning appears.
- [ ] **FR-ART-06** Drag an image file into the window to import it as a tileset or texture. *Check:* Drop a PNG; a dialog asks tileset or texture and imports it.
- [ ] **FR-SCENE-01** Scene tree of objects and layers: rename, reorder, parent, hide, lock. *Check:* Parent object B under A, move A; B follows.
- [ ] **FR-SCENE-02** Group selected faces into a new object; set an object's origin to the crosshair. *Check:* Make an object from 6 faces; it appears in the tree with its origin at the crosshair.
- [ ] **FR-SCENE-03** Instances (prefabs): place copies of an object that update when the source changes. *Check:* Place 5 instances of a crate, edit the source; all 5 change.
- [ ] **FR-SCENE-04** Markers: named empty points with a type (spawn, pickup, trigger, custom) and custom key-value properties. *Check:* Add marker "spawn_player" with property team=1; export and the glTF node has that name and those extras.
- [ ] **FR-SCENE-05** Flag any object as a collision mesh, hidden or visible. *Check:* Flag a floor; the exported node carries `extras.quarry.collision = true`.
- [ ] **FR-RIG-01** Add bones at the crosshair as a chain or branch; name, parent, set roll; show and hide. *Check:* Add 3 chained bones; each child's head is its parent's tail.
- [ ] **FR-RIG-02** Bind a mesh to a skeleton with automatic weights from bone distance. *Check:* Bind a 2-bone arm; rotating the lower bone bends only the forearm half, the shoulder vertices do not move.
- [ ] **FR-RIG-03** Weight brush: add, subtract, smooth, set to value, with pressure; weights view as a colour ramp. *Check:* Paint 1.0 on 10 vertices for bone B; those vertices show full red and follow bone B fully.
- [ ] **FR-RIG-04** Rigid-part binding: assign a whole object or face set to one bone at 100%. *Check:* Bind a robot hand object to `hand.L`; rotating the bone moves the hand rigidly.
- [ ] **FR-RIG-05** At most 4 bone influences per vertex, normalised to sum 1, enforced on every weight edit. *Check:* Paint a vertex for 5 bones; it keeps the 4 largest and their weights sum to 1.000.
- [ ] **FR-RIG-06** Two-bone IK: target and pole per chain, used while posing and keying. *Check:* Drag a leg's foot target; thigh and shin rotate so the foot reaches it and the knee points at the pole.
- [ ] **FR-RIG-07** Mirror weights and bone names left to right (`.L` / `.R`). *Check:* Paint left leg weights, mirror; right leg vertices get matching weights on `.R` bones.
- [ ] **FR-ANIM-01** Named clips per skeleton (idle, walk); create, rename, duplicate, delete, set length and loop. *Check:* Make clips "idle" and "walk"; export; Cairn lists both by name.
- [ ] **FR-ANIM-02** Timeline with per-bone tracks for translation, rotation, scale; add, move, copy, paste, delete keys; auto-key. *Check:* With auto-key on, rotate a bone at 0.5 s; a rotation key appears at 0.5 s.
- [ ] **FR-ANIM-03** Interpolation per key: step, linear, and eased (ease in, ease out). *Check:* Set a key to step; playback jumps at that key instead of blending.
- [ ] **FR-ANIM-04** Play, pause, stop, scrub, loop playback at the clip's frame rate. *Check:* A 1 s loop plays 10 times smoothly with no hitch at the wrap.
- [ ] **FR-ANIM-10** On export, every clip is sampled at the project frame rate (default 30 fps) into LINEAR keys (STEP keys stay STEP), then keys that a... *Check:* Export a 1 s walk with eased keys; the clip plays in Cairn matching Quarry's playback within 0.5 degrees per bone, and a bone held still for the whole clip exports with 2 keys or fewer.
- [ ] **FR-ANIM-05** IK targets are keyable; IK is baked to bone rotations on export. *Check:* Key a foot target at 3 times; exported glTF has rotation keys on thigh and shin only, and the foot lands where it did in Quarry.
- [ ] **FR-ANIM-08** Crossfade preview between two clips over a set time, matching Cairn's animator Fade. *Check:* Preview idle to walk with 0.25 s fade; the pose blends over 0.25 s.
- [ ] **FR-EXP-02** Skinned meshes export with skin, inverse bind matrices, JOINTS_0 and WEIGHTS_0, and each clip as one glTF animation named after it. *Check:* Import the sample robot .glb into Cairn; it shows one skeleton and its clips by name, and the walk plays as it does in Quarry.
- [ ] **FR-EXP-03** Textures export as PNG with nearest filtering set in the glTF sampler when the project uses it. *Check:* The exported sampler has magFilter 9728 (NEAREST).
- [ ] **FR-EXP-04** Markers, collision flags and custom properties export as node names and `extras`. *Check:* See FR-SCENE-04 and FR-SCENE-05.
- [ ] **FR-EXP-05** Clean geometry on export: shared vertices where position, normal, UV, colour and weights match; no duplicate data. *Check:* A 6-face box exports with 24 vertices or fewer (not 36).
- [ ] **FR-IMP-01** Import glTF 2.0 and .glb: meshes, materials with base colour texture, skins, animations, node hierarchy kept as separate objects. *Check:* Import a Kenney CC0 glTF character; each mesh is its own object, skeleton and clips are editable.
- [ ] **FR-MCP-02** Query tools: scene tree, selection, an object's mesh stats, bones, clips, keys, tilesets, settings, in compact JSON with paging for large... *Check:* Query a 100k-triangle scene; the scene tree comes back under 4,000 tokens; full vertex lists only when asked by object and range.
- [ ] **FR-MCP-06** Agent writes only inside the open project's folder; reads images from paths it is given; cannot delete files, change settings or shortcuts,... *Check:* Agent export to `/tmp/x.glb` outside the project folder is refused with an error.
- [ ] **FR-MCP-07** The human can stop the agent's current command from a visible button; the scene is left as before that command. *Check:* Start a long bake via MCP, press Stop; the bake stops and the scene matches its state before the bake.
- [ ] **FR-MCP-08** "Connect Claude" in the Help menu shows the exact config line for Claude Code and for the Claude desktop app. *Check:* Paste the shown line into Claude Code; `/mcp` lists Quarry as connected.
- [ ] **FR-UI-01** Layout: viewport, tabbed side panel (Tileset, UVs, Paint, Scene, Rig), timeline at the bottom; panels resize and collapse; layout persists. *Check:* Resize the side panel, relaunch; it keeps its width.
- [ ] **FR-UI-02** Command search (Cmd/Ctrl+K style) for every action by name, showing its shortcut. *Check:* Type "extr"; Extrude appears with its shortcut and runs on Enter.
- [ ] **FR-UI-03** Rebindable shortcuts with conflict warning, default scheme and a Blender-style preset; reset to default. *Check:* Bind Extrude to a key already used; a conflict warning names the other action.
- [ ] **FR-UI-04** Status bar: selection counts, triangle and vertex counts, current tool, agent status. *Check:* Select 3 faces; the bar shows "3 faces".
- [ ] **FR-UI-05** Settings persist: UI scale 100 to 200%, theme light and dark, external editor app, autosave interval, weight ramp choice. *Check:* Set UI scale 150%, relaunch; still 150%.
- [ ] **FR-STATE-02** Empty scene shows a hint (add a primitive, import a tileset, open the sample); empty timeline says "Add a skeleton to animate". *Check:* New project shows the hint.
- [ ] **FR-STATE-03** Any operation over 300 ms (open, save, bake, export) shows progress and does not freeze the window. *Check:* Bake AO on 100k tris; the progress bar moves and the camera can still orbit.
- [ ] **FR-STATE-04** Errors say what happened and what to do, in plain words, in a toast plus the console. *Check:* Open a corrupt file; message names the file and says it could not be read, the app keeps running.
- [ ] **FR-STATE-05** Confirm before closing with unsaved changes, deleting a tileset in use, deleting a skeleton with clips. *Check:* Delete a used tileset; a dialog lists the faces using it.

### Non-functional

- [ ] **NFR-01** Performance: 60 fps viewport with 100,000 triangles, 50 bones and 5 skinned meshes playing, on an M1 MacBook Air with 8 GB RAM. Open or save a 100k-triangle project in under 2 s. Export .glb in under 2 s.
- [ ] **NFR-02** Live refresh speed: linked image change shows in the viewport in under 1 s (FR-ART-02).
- [ ] **NFR-03** Privacy and security: no network calls at all, no telemetry, no update check unless the user asks. MCP is local only. Agent file writes limited to the project folder.
- [ ] **NFR-04** Reliability: no data loss from a crash beyond the last 2 minutes (autosave). A corrupt file never crashes the app.
- [ ] **NFR-05** Platforms: macOS 13+ (Apple Silicon and Intel), Windows 10/11 x64, Ubuntu 22.04+ x64. All three built and tested in CI from the first milestone.
- [ ] **NFR-06** Accessibility and locale: English, UI scale 100 to 200%, minimum 13 px text at 100%, every action reachable without a shortcut.
- [ ] **NFR-07** Interop: every exported file passes the Khronos glTF Validator with zero errors and loads in Cairn and in Blender's glTF importer.
- [ ] **NFR-08** Agent efficiency: a query response stays under 4,000 tokens by default; screenshots default to 768x768.
- [ ] **NFR-09** Licensing: every dependency MIT-compatible so the repo can go MIT.
- [ ] **NFR-10** Startup: cold start to an empty scene in under 2 s on the M1 8 GB.

## 10. Standard states

- Undo and redo : FR-STATE-01 (M1).
- Empty states : FR-STATE-02 (M1).
- Loading and progress : FR-STATE-03 (M1).
- Error states : FR-STATE-04 (M1).
- Confirm before destructive actions : FR-STATE-05 (M1).
- Autosave and reopen : FR-PROJ-03, FR-PROJ-04, FR-PROJ-05 (M1).
- Search and filter : FR-UI-02 (M1); scene tree filter FR-STATE-06 (Should, M1).
- Keyboard shortcuts : FR-UI-03 (M1).
- Settings that persist : FR-UI-05 (M1).
- First run and onboarding : FR-STATE-07 (Should, M1); samples FR-PROJ-08 (Should, M8).
- Offline : fully offline, NFR-03.
- Permissions per role : FR-MCP-06 (M8).
- Notifications : out of scope, single-user desktop tool; toasts and status bar cover feedback.
- Import and export : FR-IMP-01, FR-EXP-01 to FR-EXP-08 (M5, M6, M7).
- Delete and restore : undo plus backups; no trash bin.
- Audit log : FR-STATE-08 history panel (Should, M1).
- Multi-language and RTL : out of scope, English only, strings in one file.
- Accessibility : FR-UI-05, FR-UI-02, NFR-06 (M1, M9).
- Mobile layout : out of scope, desktop app.
- Rate limits and quotas : out of scope, local only; FR-MCP-02 paging (M8).

## 11. Out of scope for MVP

Do not build these even if they look easy:
- FBX, OBJ, DAE and Tiled .tmx import (glTF/GLB only).
- Sculpting, high-poly, PBR maps (normal, roughness, metal).
- Blend trees, animation layers, additive clips (Cairn only fades).
- Morph targets / shape keys.
- Real-time lights exported to glTF.
- Animated UV tiles (water, torches).
- Live link into a Cairn project.
- Built-in AI model or image generation.
- Arabic and RTL.
- A user plugin or scripting API (MCP is the scripting surface).
- Gamepad input.
- Code signing and notarisation (paid; v1 ships unsigned).

## 12. Top risks and mitigations

1. **3D viewport in GPUI** (no supported portable path) : M0 spike with numeric pass bar; fallback swaps the app crate to egui + egui-wgpu; principle 5 keeps that swap to one crate.
2. **Pen pressure on Windows and Linux** (octotablet stale since March 2024) : M0 spike; fallback is macOS-only pressure, pen as mouse elsewhere.
3. **glTF skins Cairn plays correctly** : M0 3-bone bar into Cairn before any rig UI; Khronos validator in CI; manual Cairn checks at M5, M6, M7.
4. **GPUI churn** : exact pins, deliberate upgrades in their own commit.
5. **PSD composites from Affinity/Photoshop** : M0 spike; clear "Re-save with Maximize Compatibility" message instead of a blank texture.
6. **Scope for one developer** (about 80 Must requirements) : tiles-to-GLB POC first (M1, M2, M5 partial); Should items listed separately per milestone and dropped first.

## 13. Instructions for Claude Code

- Build milestone by milestone, in order. Finish M0 before any feature work.
- Keep a `PROGRESS.md` at the repo root: milestone status, decisions made, deviations from this spec and why, and the M0 measurements.
- At the end of each milestone, run the tests and tick the acceptance boxes it covers. Don't move on with a failing box.
- Don't add features outside this spec. If something seems missing, note it in `PROGRESS.md` and ask.
- If a spec decision turns out to be wrong in practice, stop and explain. Don't silently swap a stack choice. The only pre-approved swap is GPUI to egui if M0 misses its numbers.
- Every new user-facing action is a command in the registry with a schema. Never mutate the document outside `CommandRegistry::execute`.
- Never add a dependency whose licence is not on the cargo-deny allow list, and never patch an MPL crate in place.
- Commit after each milestone with a message naming the milestone.
- Keep secrets out of the repo (there are none). Config through `QUARRY_LOG`, `QUARRY_SOCKET`, `QUARRY_HEADLESS_IDLE_SECS`, `QUARRY_DATA_DIR`.

---

*Companion files in `docs/spec/`: 01-product, 02-users, 03-requirements, 04-tech-decisions, 05-tech-requirements. This build spec is the entry point.*
