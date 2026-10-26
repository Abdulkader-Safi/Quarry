# Functional requirements

Status: signed off 2026-10-26

> **Changes:** 2026-10-26: removed every reference to the product this was modelled on, at Safi's request. No requirement changed.
> 2026-10-26: FR-ANIM-10 added (export sampling, from the "like Blender" answer). It was decided before sign-off but missing from the file; caught while generating Stage 06.

Requirements say what Quarry must do, not how. Tech choices are in Stage 4. Each traces to [[Projects/Games/quarry/spec/01-product|Stage 01]] and the jobs and journey in [[Projects/Games/quarry/spec/02-users|Stage 02]].

"Every editor action" (FR-MCP-01) means every requirement below marked with (A) is also an MCP tool.

## Core features (MVP)

### Project and files (PROJ)
- **FR-PROJ-01 [Must] (A)** Create, open, save and save-as a project as one `.quarry` file.
  *Acceptance:* Save a scene with 2 tilesets, 3 objects, a rig and 2 clips; quit; reopen; every tile, UV, weight, keyframe and image is identical.
- **FR-PROJ-02 [Must]** The `.quarry` file is a zip container holding `project.json` (scene, settings, rig, clips), binary mesh buffers, and images as PNG.
  *Acceptance:* Rename a saved file to `.zip`, unzip it, and `project.json` opens in a text editor and the PNGs open in Krita.
- **FR-PROJ-03 [Must]** Autosave every 2 minutes to a side copy, never over the user's file.
  *Acceptance:* Edit, wait 2 minutes, kill the process; the original file is untouched and the autosave holds the edit.
- **FR-PROJ-04 [Must]** After a crash, offer to recover the autosave on next launch.
  *Acceptance:* Kill the app mid-edit, relaunch, a recovery prompt shows the file name and autosave time; Recover restores the edit.
- **FR-PROJ-05 [Should]** Keep the last 10 backups per project, oldest dropped first.
  *Acceptance:* After 11 saves the backups folder holds exactly 10.
- **FR-PROJ-06 [Must]** Recent projects list on the start screen (last 10).
  *Acceptance:* Open three projects, relaunch, all three listed most recent first; a moved file shows as missing, not as a crash.
- **FR-PROJ-07 [Must]** New project starts with pixel-art defaults: nearest filtering, 16 px tiles, 1 tile = 1 m.
  *Acceptance:* New project, check settings panel shows those values.
- **FR-PROJ-08 [Should]** Ship a CC0 16 px sample tileset and a rigged sample robot, openable from the start screen.
  *Acceptance:* Start screen shows "Sample robot"; it opens with skeleton and an idle clip that plays.

### Viewport and camera (VIEW)
- **FR-VIEW-01 [Must]** Orbit, pan, zoom with mouse (middle-drag), trackpad (two-finger orbit, pinch zoom, Shift+two-finger pan) and keyboard.
  *Acceptance:* Every camera move works on a Mac trackpad with no mouse attached.
- **FR-VIEW-02 [Must]** Perspective and orthographic modes, plus front, side, top and isometric presets.
  *Acceptance:* Isometric preset shows a cube with three equal visible faces.
- **FR-VIEW-03 [Must]** Frame selection, and frame all.
  *Acceptance:* Select one vertex far from origin, press frame; it is centred.
- **FR-VIEW-04 [Must]** A 3D crosshair (cursor) that marks where tiles, primitives, bones and markers are placed; moves with keys, snaps to grid, vertices and objects.
  *Acceptance:* Snap to a vertex, add a bone; the bone head sits on that vertex.
- **FR-VIEW-05 [Must]** Display toggles: wireframe, textured, vertex colour, weights, bones, grid, backface culling.
  *Acceptance:* Each toggle changes the view and persists after reopen.
- **FR-VIEW-06 [Must] (A)** Take a screenshot of the viewport from a given camera preset or position, at a given size.
  *Acceptance:* MCP call for front, side and iso at 768x768 returns three PNGs matching the view.

### Tile placement (TILE)
- **FR-TILE-01 [Must] (A)** Import a PNG or PSD as a tileset with a tile size and padding.
  *Acceptance:* Import a 256x256 PNG at 16 px; the tileset panel shows a 16x16 grid of tiles.
- **FR-TILE-02 [Must] (A)** Pick one tile or a rectangle of tiles as the brush.
  *Acceptance:* Drag-select 2x2 tiles; the brush preview shows all four.
- **FR-TILE-03 [Must] (A)** Place tiles on the plane at the crosshair; click to place, drag to paint, right-click to erase, rectangle fill and erase.
  *Acceptance:* Drag across 5 cells; 5 quads exist, each textured with the brush tile.
- **FR-TILE-04 [Must] (A)** Lock the placement plane to X, Y or Z and tilt it.
  *Acceptance:* Lock to X, place tiles; all their normals point along X.
- **FR-TILE-05 [Must] (A)** Rotate, flip and mirror the brush.
  *Acceptance:* Rotate 90 and place; UVs are rotated 90 compared to the unrotated tile.
- **FR-TILE-06 [Should] (A)** Sticky placement: new tiles extend off an existing tile edge.
  *Acceptance:* Click a floor tile's edge; a wall tile appears joined at that edge.
- **FR-TILE-07 [Should] (A)** Block brush: add or remove a cube of tiles, each face textured.
  *Acceptance:* Add one block; 6 faces exist, sharing 8 vertices.
- **FR-TILE-08 [Should] (A)** Tile palette with weights for random placement and random rotation.
  *Acceptance:* Palette of 2 grass tiles at 50/50, place 100 tiles, each appears 35 to 65 times.
- **FR-TILE-09 [Must] (A)** Eyedrop a tile from the scene into the brush.
  *Acceptance:* Eyedrop a placed tile; the brush shows that tile with its rotation.

### Mesh editing (MESH)
- **FR-MESH-01 [Must] (A)** Add primitives at the crosshair: box, wedge, cylinder, cone, sphere, plane, with segment counts.
  *Acceptance:* Add an 8-segment cylinder; it has 8 side quads and two caps.
- **FR-MESH-02 [Must] (A)** Select vertices, edges, faces and objects by click, box and lasso; add, remove, invert, select all.
  *Acceptance:* Box-select 4 vertices, Shift-click a fifth; 5 are selected.
- **FR-MESH-03 [Must] (A)** Move, rotate and scale selections with a gizmo, typed values and snapping.
  *Acceptance:* Type a move of 0.5 on Y; the selection's centre rises exactly 0.5.
- **FR-MESH-04 [Must] (A)** Extrude faces and edges.
  *Acceptance:* Extrude a box's top face by 1; the box has 5 more faces and the new top sits 1 higher.
- **FR-MESH-05 [Must] (A)** Merge vertices (at centre, at crosshair, by distance), split, delete with or without faces.
  *Acceptance:* Merge by distance 0.001 on two coincident vertices; one remains.
- **FR-MESH-06 [Must] (A)** Create a face from 3 or 4 selected vertices; flip normals; flip quad diagonal.
  *Acceptance:* Select 4 vertices, create face; one quad exists facing the camera.
- **FR-MESH-07 [Should] (A)** Subdivide, inset, bevel edges (one segment), loop select, knife cut between two points.
  *Acceptance:* Bevel one edge of a box; the box gains one face along that edge.
- **FR-MESH-08 [Must] (A)** Mirror modelling across an axis, live, until applied.
  *Acceptance:* Turn on X mirror, move a vertex on +X; its twin on -X moves to match.
- **FR-MESH-09 [Should] (A)** Flat or smooth shading per face group.
  *Acceptance:* Mark a cylinder's sides smooth; export shows shared normals on the sides and hard caps.

### UVs (UV)
- **FR-UV-01 [Must] (A)** UV panel showing selected faces over the texture; move, rotate, scale, flip UVs.
  *Acceptance:* Rotate a face's UVs 90 in the panel; the viewport texture rotates.
- **FR-UV-02 [Must] (A)** Unwrap selected faces with marked seams; project from view; snap UVs to the pixel grid.
  *Acceptance:* Unwrap a box with 7 seams marked; 6 faces lay flat with no overlap.
- **FR-UV-03 [Should] (A)** Pack UV islands inside the texture with pixel padding.
  *Acceptance:* Pack 10 islands at 1 px padding; none overlap and all are inside 0 to 1.

### Painting (PAINT)
- **FR-PAINT-01 [Must] (A)** Paint in a 2D texture panel: pencil, eraser, fill, eyedropper, rectangle select, with a palette strip.
  *Acceptance:* Pencil one pixel at 400% zoom; that exact texel changes and nothing else.
- **FR-PAINT-02 [Must] (A)** Paint directly on the model in the 3D viewport; strokes land on the texel under the cursor.
  *Acceptance:* Paint a line across two faces sharing a seam; the line is continuous in 3D.
- **FR-PAINT-03 [Must]** Pen pressure changes brush size for paint (in pixel steps) when a tablet is present.
  *Acceptance:* Light and heavy strokes with the pen produce 1 px and 3 px lines at max size 3.
- **FR-PAINT-04 [Must] (A)** Palette-locked mode: painting snaps to the nearest palette colour.
  *Acceptance:* With a 16-colour palette, paint with a colour not in it; the texel gets the nearest palette colour.
- **FR-PAINT-05 [Should] (A)** Vertex colours: paint per vertex, multiply over the texture, bake ambient occlusion into vertex colours.
  *Acceptance:* Bake AO on a box inside a corner; vertices in the corner are darker than exposed ones.
- **FR-PAINT-06 [Must] (A)** Create a blank texture at a given size and assign it to selected faces.
  *Acceptance:* New 32x32 texture on a box; all faces show it.

### External art (ART)
- **FR-ART-01 [Must]** Link a tileset or texture to a PNG or PSD file on disk; Quarry keeps a copy inside the `.quarry` file and remembers the source path.
  *Acceptance:* Move the source file away; the project still opens with the embedded copy and marks the link as missing.
- **FR-ART-02 [Must]** When a linked file changes on disk, reload it within 1 second, without the user switching to Quarry.
  *Acceptance:* Save a PNG in Krita while Quarry sits behind it; the model updates in under 1 s.
- **FR-ART-03 [Must]** PSD files are read as the flattened image, so Photoshop and Affinity PSD exports work.
  *Acceptance:* A 3-layer PSD from Affinity shows the same pixels as its flattened PNG export.
- **FR-ART-04 [Must]** If the new image has a different size, keep UVs in pixel positions and warn.
  *Acceptance:* Grow a 64x64 texture to 128x128 by canvas resize; existing faces still show the same pixels and a warning appears.
- **FR-ART-05 [Should]** "Open in external editor" button per texture using the app set in Settings.
  *Acceptance:* Set Krita as the editor; click the button; the source PNG opens in Krita.
- **FR-ART-06 [Must] (A)** Drag an image file into the window to import it as a tileset or texture.
  *Acceptance:* Drop a PNG; a dialog asks tileset or texture and imports it.

### Scene and level (SCENE)
- **FR-SCENE-01 [Must] (A)** Scene tree of objects and layers: rename, reorder, parent, hide, lock.
  *Acceptance:* Parent object B under A, move A; B follows.
- **FR-SCENE-02 [Must] (A)** Group selected faces into a new object; set an object's origin to the crosshair.
  *Acceptance:* Make an object from 6 faces; it appears in the tree with its origin at the crosshair.
- **FR-SCENE-03 [Must] (A)** Instances (prefabs): place copies of an object that update when the source changes.
  *Acceptance:* Place 5 instances of a crate, edit the source; all 5 change.
- **FR-SCENE-04 [Must] (A)** Markers: named empty points with a type (spawn, pickup, trigger, custom) and custom key-value properties.
  *Acceptance:* Add marker "spawn_player" with property team=1; export and the glTF node has that name and those extras.
- **FR-SCENE-05 [Must] (A)** Flag any object as a collision mesh, hidden or visible.
  *Acceptance:* Flag a floor; the exported node carries `extras.quarry.collision = true`.
- **FR-SCENE-06 [Should] (A)** Custom key-value properties on any object.
  *Acceptance:* Add `hp=40` to an object; it appears in glTF node extras.
- **FR-SCENE-07 [Should] (A)** Duplicate and mirror objects, bones and instances across an axis.
  *Acceptance:* Mirror a left leg object on X; a right leg appears with mirrored geometry and a `.R` name.

### Rigging (RIG)
- **FR-RIG-01 [Must] (A)** Add bones at the crosshair as a chain or branch; name, parent, set roll; show and hide.
  *Acceptance:* Add 3 chained bones; each child's head is its parent's tail.
- **FR-RIG-02 [Must] (A)** Bind a mesh to a skeleton with automatic weights from bone distance.
  *Acceptance:* Bind a 2-bone arm; rotating the lower bone bends only the forearm half, the shoulder vertices do not move.
- **FR-RIG-03 [Must] (A)** Weight brush: add, subtract, smooth, set to value, with pressure; weights view as a colour ramp.
  *Acceptance:* Paint 1.0 on 10 vertices for bone B; those vertices show full red and follow bone B fully.
- **FR-RIG-04 [Must] (A)** Rigid-part binding: assign a whole object or face set to one bone at 100%.
  *Acceptance:* Bind a robot hand object to `hand.L`; rotating the bone moves the hand rigidly.
- **FR-RIG-05 [Must]** At most 4 bone influences per vertex, normalised to sum 1, enforced on every weight edit.
  *Acceptance:* Paint a vertex for 5 bones; it keeps the 4 largest and their weights sum to 1.000.
- **FR-RIG-06 [Must] (A)** Two-bone IK: target and pole per chain, used while posing and keying.
  *Acceptance:* Drag a leg's foot target; thigh and shin rotate so the foot reaches it and the knee points at the pole.
- **FR-RIG-07 [Must] (A)** Mirror weights and bone names left to right (`.L` / `.R`).
  *Acceptance:* Paint left leg weights, mirror; right leg vertices get matching weights on `.R` bones.
- **FR-RIG-08 [Should] (A)** Rest pose and pose mode, with reset pose.
  *Acceptance:* Pose a bone, reset; the bone is back to rest.

### Animation (ANIM)
- **FR-ANIM-01 [Must] (A)** Named clips per skeleton (idle, walk); create, rename, duplicate, delete, set length and loop.
  *Acceptance:* Make clips "idle" and "walk"; export; Cairn lists both by name.
- **FR-ANIM-02 [Must] (A)** Timeline with per-bone tracks for translation, rotation, scale; add, move, copy, paste, delete keys; auto-key.
  *Acceptance:* With auto-key on, rotate a bone at 0.5 s; a rotation key appears at 0.5 s.
- **FR-ANIM-03 [Must] (A)** Interpolation per key: step, linear, and eased (ease in, ease out).
  *Acceptance:* Set a key to step; playback jumps at that key instead of blending.
- **FR-ANIM-04 [Must]** Play, pause, stop, scrub, loop playback at the clip's frame rate.
  *Acceptance:* A 1 s loop plays 10 times smoothly with no hitch at the wrap.
- **FR-ANIM-10 [Must]** On export, every clip is sampled at the project frame rate (default 30 fps) into LINEAR keys (STEP keys stay STEP), then keys that a straight line through their neighbours already predicts are removed, as Blender's sampling and "optimize animation size" do.
  *Acceptance:* Export a 1 s walk with eased keys; the clip plays in Cairn matching Quarry's playback within 0.5 degrees per bone, and a bone held still for the whole clip exports with 2 keys or fewer.
- **FR-ANIM-05 [Must]** IK targets are keyable; IK is baked to bone rotations on export.
  *Acceptance:* Key a foot target at 3 times; exported glTF has rotation keys on thigh and shin only, and the foot lands where it did in Quarry.
- **FR-ANIM-06 [Should] (A)** Graph editor for curves of selected tracks.
  *Acceptance:* Drag a curve handle; playback changes to match.
- **FR-ANIM-07 [Should]** Onion skin: show the pose at the previous and next key, faded.
  *Acceptance:* Turn on onion skin at a key; two ghost poses show.
- **FR-ANIM-08 [Must] (A)** Crossfade preview between two clips over a set time, matching Cairn's animator Fade.
  *Acceptance:* Preview idle to walk with 0.25 s fade; the pose blends over 0.25 s.
- **FR-ANIM-09 [Should] (A)** Mirror a pose or a clip left to right.
  *Acceptance:* Mirror a walk clip; the left and right leg tracks swap.

### Export (EXP)
- **FR-EXP-01 [Must] (A)** Export the scene or selected objects as one self-contained .glb holding every mesh, texture, material, skeleton and clip, like Blender's default glTF export. A separate .gltf + .bin + PNG option exists for debugging.
  *Acceptance:* Export the sample robot; exactly one .glb file is written; it passes the Khronos glTF Validator with zero errors and opens in Blender with mesh, texture, skeleton and all clips.
- **FR-EXP-02 [Must]** Skinned meshes export with skin, inverse bind matrices, JOINTS_0 and WEIGHTS_0, and each clip as one glTF animation named after it.
  *Acceptance:* Import the sample robot .glb into Cairn; it shows one skeleton and its clips by name, and the walk plays as it does in Quarry.
- **FR-EXP-03 [Must]** Textures export as PNG with nearest filtering set in the glTF sampler when the project uses it.
  *Acceptance:* The exported sampler has magFilter 9728 (NEAREST).
- **FR-EXP-04 [Must]** Markers, collision flags and custom properties export as node names and `extras`.
  *Acceptance:* See FR-SCENE-04 and FR-SCENE-05.
- **FR-EXP-05 [Must]** Clean geometry on export: shared vertices where position, normal, UV, colour and weights match; no duplicate data.
  *Acceptance:* A 6-face box exports with 24 vertices or fewer (not 36).
- **FR-EXP-06 [Should] (A)** Export presets saved per project (path, format, selection, scale) and re-export with one key.
  *Acceptance:* Re-export after a change writes the same path without a dialog.
- **FR-EXP-07 [Should] (A)** Export a turntable GIF or PNG sprite sheet of a clip from a chosen camera.
  *Acceptance:* Export walk at 8 frames from iso; the sheet has 8 frames in order.
- **FR-EXP-08 [Could] (A)** Export static meshes as OBJ.
  *Acceptance:* OBJ opens in Blender with the texture applied.

### Import (IMP)
- **FR-IMP-01 [Must] (A)** Import glTF 2.0 and .glb: meshes, materials with base colour texture, skins, animations, node hierarchy kept as separate objects.
  *Acceptance:* Import a Kenney CC0 glTF character; each mesh is its own object, skeleton and clips are editable.

### AI control (MCP)
- **FR-MCP-01 [Must]** Every action marked (A) in this file is an MCP tool with a name, a description and typed inputs.
  *Acceptance:* An automated check compares the list of (A) actions with the MCP tool list; none missing.
- **FR-MCP-02 [Must]** Query tools: scene tree, selection, an object's mesh stats, bones, clips, keys, tilesets, settings, in compact JSON with paging for large results.
  *Acceptance:* Query a 100k-triangle scene; the scene tree comes back under 4,000 tokens; full vertex lists only when asked by object and range.
- **FR-MCP-03 [Must]** Every MCP edit is one undo step in the shared history, labelled as from the agent.
  *Acceptance:* Agent extrudes a face; the human presses undo once; the extrude is gone and the history shows "Agent: extrude".
- **FR-MCP-04 [Must]** A failed tool call changes nothing and returns an error saying what was wrong and how to fix it.
  *Acceptance:* Call extrude with nothing selected; error says "No faces selected. Call select first."; the scene is unchanged.
- **FR-MCP-05 [Must]** Works live against the open window, and headless with no window for batch jobs.
  *Acceptance:* With the window open, an agent's add_box appears in the viewport. With none open, the same call creates the box in a headless instance and save works.
- **FR-MCP-06 [Must]** Agent writes only inside the open project's folder; reads images from paths it is given; cannot delete files, change settings or shortcuts, or quit.
  *Acceptance:* Agent export to `/tmp/x.glb` outside the project folder is refused with an error.
- **FR-MCP-07 [Must]** The human can stop the agent's current command from a visible button; the scene is left as before that command.
  *Acceptance:* Start a long bake via MCP, press Stop; the bake stops and the scene matches its state before the bake.
- **FR-MCP-08 [Must]** "Connect Claude" in the Help menu shows the exact config line for Claude Code and for the Claude desktop app.
  *Acceptance:* Paste the shown line into Claude Code; `/mcp` lists Quarry as connected.
- **FR-MCP-09 [Should]** A short built-in guide resource for agents: units, axes, naming conventions, a sample workflow (block out, paint, rig, animate, check, export).
  *Acceptance:* An agent reading only the guide builds and exports a rigged two-clip prop without asking how.
- **FR-MCP-10 [Could]** High-level helper tools built on the low-level ones: auto-rig biped, auto-rig quadruped, walk-cycle starter.
  *Acceptance:* Auto-rig biped on the sample humanoid makes a named 15-bone skeleton bound with weights.

### Interface (UI)
- **FR-UI-01 [Must]** Layout: viewport, tabbed side panel (Tileset, UVs, Paint, Scene, Rig), timeline at the bottom; panels resize and collapse; layout persists.
  *Acceptance:* Resize the side panel, relaunch; it keeps its width.
- **FR-UI-02 [Must]** Command search (Cmd/Ctrl+K style) for every action by name, showing its shortcut.
  *Acceptance:* Type "extr"; Extrude appears with its shortcut and runs on Enter.
- **FR-UI-03 [Must]** Rebindable shortcuts with conflict warning, default scheme and a Blender-style preset; reset to default.
  *Acceptance:* Bind Extrude to a key already used; a conflict warning names the other action.
- **FR-UI-04 [Must]** Status bar: selection counts, triangle and vertex counts, current tool, agent status.
  *Acceptance:* Select 3 faces; the bar shows "3 faces".
- **FR-UI-05 [Must]** Settings persist: UI scale 100 to 200%, theme light and dark, external editor app, autosave interval, weight ramp choice.
  *Acceptance:* Set UI scale 150%, relaunch; still 150%.

## Standard states
- **Undo and redo:** FR-STATE-01 [Must] (A) Undo and redo every scene edit, human or agent, including tileset imports, image refreshes, weight strokes and key edits; at least 200 steps. *Acceptance:* Import a tileset, place 3 tiles, undo 4 times; the tileset is gone too and nothing else changed.
- **Empty states:** FR-STATE-02 [Must] Empty scene shows a hint (add a primitive, import a tileset, open the sample); empty timeline says "Add a skeleton to animate". *Acceptance:* New project shows the hint.
- **Loading and progress:** FR-STATE-03 [Must] Any operation over 300 ms (open, save, bake, export) shows progress and does not freeze the window. *Acceptance:* Bake AO on 100k tris; the progress bar moves and the camera can still orbit.
- **Error states:** FR-STATE-04 [Must] Errors say what happened and what to do, in plain words, in a toast plus the console. *Acceptance:* Open a corrupt file; message names the file and says it could not be read, the app keeps running.
- **Confirm before destructive actions:** FR-STATE-05 [Must] Confirm before closing with unsaved changes, deleting a tileset in use, deleting a skeleton with clips. *Acceptance:* Delete a used tileset; a dialog lists the faces using it.
- **Autosave and reopen:** FR-PROJ-03, FR-PROJ-04, FR-PROJ-05.
- **Search and filter:** FR-UI-02 for actions; FR-STATE-06 [Should] filter the scene tree by name. *Acceptance:* Type "leg" in the tree filter; only matching items show.
- **Keyboard shortcuts:** FR-UI-03.
- **Settings that persist:** FR-UI-05.
- **First run and onboarding:** FR-STATE-07 [Should] First launch shows the start screen with New, Open, the sample robot and a one-page "basics" overlay of camera and placement controls. *Acceptance:* Fresh install shows the overlay once.
- **Offline:** Fully offline. No feature needs the network.
- **Permissions per role:** FR-MCP-06.
- **Notifications:** Out of scope: single-user desktop tool; toasts and the status bar cover feedback.
- **Import and export:** FR-IMP-01, FR-EXP-01 to FR-EXP-08.
- **Delete and restore:** Undo covers in-session deletes; backups (FR-PROJ-05) cover the rest. No trash bin.
- **Audit log:** FR-STATE-08 [Should] History panel lists every action with source (human or agent) and time for the session. *Acceptance:* After 5 agent and 3 human actions, the panel lists 8 labelled entries.
- **Multi-language and RTL:** Out of scope: English only, strings in one file (Stage 02).
- **Accessibility:** FR-UI-05 (scale), weight ramp option, keyboard reachable commands (FR-UI-02).
- **Mobile layout:** Out of scope: desktop app.
- **Rate limits and quotas:** Out of scope: local only. FR-MCP-02 paging keeps agent responses small.

## Account and auth

None. Single-user local desktop app. The MCP connection is local only (stdio to a local process); no network listener is exposed beyond the machine.

## Data the product handles
- **Project** : one `.quarry` zip file wherever the user saves it; holds everything below. Owned by the user, kept until they delete it.
- **Scene** : objects, layers, instances, markers, custom properties; in `project.json`.
- **Mesh** : vertices, faces, UVs, normals, vertex colours, weights; binary buffers in the zip.
- **Image** : tilesets and textures as PNG in the zip, plus the linked source path on disk.
- **Skeleton** : bones, hierarchy, rest pose, IK chains; in `project.json`.
- **Clip** : named animation with tracks and keys; in `project.json`.
- **Export preset** : path, format, scale, selection; in `project.json`.
- **Autosave and backups** : copies of the project in an app data folder per project, last 10.
- **Settings** : UI scale, theme, shortcuts, external editor, recent files; app config folder.
- **History** : undo stack and action log; memory only, not saved.

## Integrations
- **Claude Code / Claude desktop app** : MCP over stdio, local. In: tool calls. Out: results, JSON, PNG screenshots. No auth beyond being on the same machine.
- **External image editors (Affinity, Krita, Photoshop)** : file watching only. In: PNG and PSD changes. Out: "open in editor" launches the app with the file.
- **Cairn** : none directly. Quarry writes glTF/GLB that Cairn's asset importer reads.

## Non-functional requirements
- **NFR-01 Performance:** 60 fps viewport with 100,000 triangles, 50 bones and 5 skinned meshes playing, on an M1 MacBook Air with 8 GB RAM. Open or save a 100k-triangle project in under 2 s. Export .glb in under 2 s.
- **NFR-02 Live refresh speed:** linked image change shows in the viewport in under 1 s (FR-ART-02).
- **NFR-03 Privacy and security:** no network calls at all, no telemetry, no update check unless the user asks. MCP is local only. Agent file writes limited to the project folder.
- **NFR-04 Reliability:** no data loss from a crash beyond the last 2 minutes (autosave). A corrupt file never crashes the app.
- **NFR-05 Platforms:** macOS 13+ (Apple Silicon and Intel), Windows 10/11 x64, Ubuntu 22.04+ x64. All three built and tested in CI from the first milestone.
- **NFR-06 Accessibility and locale:** English, UI scale 100 to 200%, minimum 13 px text at 100%, every action reachable without a shortcut.
- **NFR-07 Interop:** every exported file passes the Khronos glTF Validator with zero errors and loads in Cairn and in Blender's glTF importer.
- **NFR-08 Agent efficiency:** a query response stays under 4,000 tokens by default; screenshots default to 768x768.
- **NFR-09 Licensing:** every dependency MIT-compatible so the repo can go MIT.
- **NFR-10 Startup:** cold start to an empty scene in under 2 s on the M1 8 GB.

## Out of scope for MVP
- **FBX, OBJ, DAE and Tiled .tmx import** : glTF/GLB only, as decided. Maybe v2 for OBJ.
- **Sculpting, high-poly, PBR maps (normal, roughness, metal)** : Stage 01 says no. Never.
- **Blend trees, animation layers, additive clips** : Cairn only fades between states. Revisit if Cairn grows them.
- **Morph targets / shape keys** : Cairn does not mention them. v2 if Cairn adds them.
- **Real-time lights exported to glTF** : levels get lit in Cairn. v2 via KHR_lights_punctual.
- **Animated UV tiles (water, torches)** : glTF has no simple standard for it and Cairn does not read it. v2.
- **Live link into a Cairn project** : decided against in Stage 01.
- **Built-in AI model or image generation** : Stage 01 says no.
- **Arabic and RTL** : Stage 02.
- **Plugin or scripting API for users** : the MCP is the scripting surface. v2.
- **Gamepad input** : not needed for Safi. Never in v1.

---

### Open questions

None.

### Assumptions confirmed

1. **Project file : RESOLVED.** One `.quarry` file, built as a zip holding `project.json`, binary mesh buffers and PNGs. JSON over YAML because it parses much faster; zip because images and vertex data should not be text.
2. **Imports : RESOLVED.** glTF and GLB only for models, plus PNG and PSD for images.
3. **Performance target : RESOLVED.** 100k triangles, 50 bones, 60 fps on an M1 MacBook Air 8 GB.
4. **Autosave : RESOLVED.** Every 2 minutes to a side copy, crash recovery, last 10 backups.
5. **Export shape and easing : RESOLVED.** Like Blender: one .glb with everything in it by default (FR-EXP-01). Eased and curve keys are sampled to linear at the project frame rate with redundant keys removed (FR-ANIM-10).
6. **Bone influences : RESOLVED.** Best practice: 4 per vertex, JOINTS_0 / WEIGHTS_0, normalised (FR-RIG-05).
7. **Bevel, knife, inset : RESOLVED.** Kept as Should (FR-MESH-07).
