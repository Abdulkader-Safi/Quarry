# Technical requirements

Status: signed off 2026-10-26

> **Changes:** 2026-10-26: removed every reference to the product this was modelled on, at Safi's request. Reference images stay in `references/` but are not linked from the spec. No requirement changed.

The engineering blueprint. Tech context: Rust workspace, GPUI via gpui-kit 0.8.0 for UI, wgpu 30 offscreen viewport (Mac zero-copy, Windows/Linux readback), own polygon mesh and command registry, `.quarry` zip of JSON + binary + PNG, glTF/GLB via `gltf` + `gltf-json`, MCP via `rmcp` over stdio bridged to a local-socket JSON command server ([[Projects/Games/quarry/spec/04-tech-decisions|Stage 04]]).

## Architecture

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

**The keystone:** one `Document` owned by one thread (the app's main thread, or the headless runner), and one `CommandRegistry`. Nothing mutates the document except `registry.execute(command, source)`. Execute validates input, runs the command against a working copy of the touched objects, and either commits (pushing one history entry) or discards everything and returns an error. UI, shortcuts, command search, socket server and MCP all go through it. Rendering reads an immutable snapshot of the document after each commit.

## Data models

Conventions:
- IDs: `u32` slot index + `u32` generation, written as `"o12g1"` style strings in JSON and MCP so the agent sees short stable names. Objects, bones, clips, markers, tilesets and images also have a unique human `name`; every MCP tool accepts a name or an ID.
- Space: right-handed, Y up, 1 unit = 1 metre, matching glTF. Rotations stored as quaternions, shown to humans and the agent as Euler degrees (XYZ).
- Time: seconds as `f32` on the timeline; project frame rate (default 30) for sampling and frames view.
- Colours: sRGB `u8` RGBA in images and palettes; vertex colours linear `f32` RGBA.
- Delete: hard delete inside the document; undo and backups are the recovery path. No soft-delete flags.
- Tenancy: none, single user.

### Project
| Field | Type | Notes |
|---|---|---|
| format_version | u32 | starts at 1, migrations on open |
| name | string | |
| settings | ProjectSettings | tile_px (16), filter (nearest), fps (30), palette_lock (true), grid_step (1.0) |
| objects | [Object] | scene tree roots and children |
| tilesets | [Image] | images used as tilesets |
| textures | [Image] | images used as painted textures |
| palettes | [Palette] | named colour lists |
| skeletons | [Skeleton] | |
| clips | [Clip] | each belongs to one skeleton |
| export_presets | [ExportPreset] | |

### Object
| Field | Type | Notes |
|---|---|---|
| id, name | ObjectId, string | name unique in project |
| kind | enum | mesh, instance, marker, layer |
| parent | ObjectId? | |
| transform | translation vec3, rotation quat, scale vec3 | local |
| visible, locked | bool | |
| mesh | Mesh? | kind = mesh |
| instance_of | ObjectId? | kind = instance; source must be kind mesh |
| marker_type | enum? | spawn, pickup, trigger, custom; kind = marker |
| collision | bool | exported as `extras.quarry.collision` |
| properties | map string -> string/number/bool | exported as `extras.quarry.props` |
| skin | Skin? | skeleton id + bind mode (weighted, rigid bone id) |

### Mesh
| Field | Type | Notes |
|---|---|---|
| positions | [vec3] | per vertex |
| weights | [[u16 bone, f32 w]; up to 4] | per vertex, sum 1, sorted by weight |
| faces | [Face] | |
| mirror | axis? | live mirror modelling (FR-MESH-08) |

### Face
| Field | Type | Notes |
|---|---|---|
| corners | 3 or 4 x {vertex u32, uv vec2 (pixels), color vec4} | UVs stored in pixels so image resizes keep them (FR-ART-04) |
| image | ImageId | tileset or texture |
| smooth_group | u8 | 0 = flat |
| tile | u32? | tile index the face came from |
| hidden | bool | |

### Image (tileset or texture)
| Field | Type | Notes |
|---|---|---|
| id, name | ImageId, string | |
| role | enum | tileset, texture |
| size | u32 x u32 | pixels |
| tile_px, padding | u32, u32 | tilesets only |
| zip_path | string | `images/<id>.png` inside the `.quarry` |
| source_path | string? | linked PNG/PSD on disk |
| source_hash | u64 | last loaded content hash, for own-write suppression |
| filter | enum | nearest, linear |

### Palette
| Field | Type | Notes |
|---|---|---|
| name | string | |
| colors | [rgba u8] | max 256 |
| weights | [f32]? | for tile palettes (FR-TILE-08) |

### Skeleton and Bone
| Field | Type | Notes |
|---|---|---|
| skeleton.id, name | SkeletonId, string | |
| bones | [Bone] | max 128 (NFR scale is 50) |
| bone.name | string | unique per skeleton, `.L`/`.R` suffix for mirroring |
| bone.parent | u16? | index |
| bone.head, tail, roll | vec3, vec3, f32 | rest pose in skeleton space |
| ik_chains | [IkChain] | |
| ik.tip_bone, target, pole | u16, vec3, vec3 | two-bone chain ending at tip |

### Clip, Track, Key
| Field | Type | Notes |
|---|---|---|
| clip.name | string | unique per skeleton, becomes glTF animation name |
| clip.skeleton | SkeletonId | |
| clip.length, loop | f32 seconds, bool | |
| tracks | [Track] | |
| track.target | bone index or ik target | |
| track.channel | enum | translation, rotation, scale, ik_target, ik_pole |
| keys | [Key] sorted by time | |
| key.time, value | f32, vec3 or quat | |
| key.interp | enum | step, linear, ease_in, ease_out, ease_in_out, bezier |
| key.handles | (vec2, vec2)? | bezier only, from the graph editor |

### ExportPreset
| Field | Type | Notes |
|---|---|---|
| name | string | |
| path | string | relative to the project file's folder |
| format | enum | glb (default), gltf_separate, obj |
| selection | enum | all, selected, objects by name |
| scale | f32 | default 1.0 |

### HistoryEntry (memory only)
| Field | Type | Notes |
|---|---|---|
| label | string | "Agent: extrude" |
| source | enum | human, agent |
| time | instant | |
| undo | Inverse or Snapshot | |

### Settings (app config, not in project)
`ui_scale`, `theme`, `shortcuts` (map action -> key), `shortcut_preset`, `external_editor`, `autosave_minutes` (2), `weight_ramp` (classic, colorblind), `recent_files` (10), `window_state`.

### Relationships and indexes
Object names, bone names per skeleton and clip names per skeleton are kept in hash maps for name lookup (agent calls by name). Mesh adjacency for loop select and sticky tiles is built on demand per object and dropped on change. Images are referenced by faces; deleting an image in use is blocked behind a confirm (FR-STATE-05).

## .quarry file layout
```
project.json          // everything above except binary arrays and pixels
meshes/<object>.bin   // positions, weights, corners (little-endian, versioned header)
images/<image>.png    // stored, not deflated
thumbnail.png         // 256x256 for the recent list
```

## API / command surface

Style: one registry of commands. Name `area.verb` (Cairn style); the MCP tool name is the same with `_` (`tile.place` -> `tile_place`). Input is a typed struct deriving `serde` + `schemars`. Output is JSON. Socket protocol is one JSON object per line: `{"id":1,"command":"tile.place","args":{...}}` -> `{"id":1,"ok":true,"result":{...}}` or `{"id":1,"ok":false,"error":{"code":"nothing_selected","message":"No faces selected. Call select.faces first."}}`.

Error codes: `invalid_args`, `not_found`, `nothing_selected`, `conflict`, `out_of_bounds`, `forbidden` (agent permission), `io`, `cancelled`, `internal`. A failed command changes nothing (FR-MCP-04).

Auth: none beyond the per-user socket path. Each connection is tagged `human` (the app itself) or `agent` (socket clients). Agent connections get the restrictions in Permissions. Commands marked (long) report progress and can be cancelled (FR-MCP-07).

### project
- `project.new(name, settings?) -> project` (FR-PROJ-07). Agent: allowed, path must be in a project folder it was given.
- `project.open(path) -> project` (FR-PROJ-01). (long)
- `project.save() / project.save_as(path)` (FR-PROJ-01). (long)
- `project.info() -> settings, counts` (FR-MCP-02).
- `project.set_settings(partial)` (FR-PROJ-07).

### query (read-only, FR-MCP-02)
- `query.scene(depth?, page?) -> tree of {id,name,kind,children}`; capped at 4,000 tokens, pages beyond.
- `query.object(name) -> transform, counts, skin, props, bounds`.
- `query.mesh(name, from?, count?) -> vertices/faces slice`.
- `query.selection() -> mode, ids, counts, bounds`.
- `query.skeleton(name) -> bones with head/tail/parent`.
- `query.clip(name) -> tracks and keys`.
- `query.images() -> tilesets and textures with sizes and links`.
- `query.history(n) -> last n entries`.

### view (FR-VIEW)
- `view.camera(preset | position+target, ortho?)` (FR-VIEW-02).
- `view.frame(selection | all)` (FR-VIEW-03).
- `view.cursor(position | snap_to: vertex/object/grid)` (FR-VIEW-04).
- `view.display(toggles)` (FR-VIEW-05). Agent: allowed, it is per-view not a setting.
- `view.screenshot(preset | camera, width?, height?, overlays?) -> PNG` (FR-VIEW-06). Default 768x768.

### select (FR-MESH-02)
- `select.mode(vertex|edge|face|object)`, `select.by_ids`, `select.box(screen rect)`, `select.all`, `select.none`, `select.invert`, `select.loop(edge)`, `select.by_image`, `select.by_normal(dir, angle)`.

### tile (FR-TILE)
- `tile.import_tileset(path, tile_px, padding) -> image` (FR-TILE-01).
- `tile.brush(tileset, rect, rotate?, flip?, mirror?)` (FR-TILE-02, FR-TILE-05).
- `tile.plane(axis, tilt?)` (FR-TILE-04).
- `tile.place(cells | at cursor)`, `tile.erase(cells)`, `tile.fill_rect(a, b)`, `tile.erase_rect(a, b)` (FR-TILE-03).
- `tile.place_sticky(face, edge)` (FR-TILE-06). `tile.block_add / block_remove(cell)` (FR-TILE-07).
- `tile.palette_set(weights, random_rotate)` (FR-TILE-08). `tile.eyedrop(face)` (FR-TILE-09).

### mesh (FR-MESH)
- `mesh.add_primitive(kind, segments?, size?)` (FR-MESH-01).
- `mesh.transform(translate?, rotate?, scale?, pivot?)` (FR-MESH-03).
- `mesh.extrude(distance)` (FR-MESH-04).
- `mesh.merge(at: center|cursor|distance)`, `mesh.split`, `mesh.delete(verts|faces|only_faces)` (FR-MESH-05).
- `mesh.make_face`, `mesh.flip_normals`, `mesh.flip_diagonal` (FR-MESH-06).
- `mesh.subdivide`, `mesh.inset`, `mesh.bevel`, `mesh.knife(a, b)` (FR-MESH-07).
- `mesh.mirror_mode(axis | off)`, `mesh.mirror_apply` (FR-MESH-08).
- `mesh.smooth_group(n)` (FR-MESH-09).

### uv (FR-UV)
- `uv.transform`, `uv.mark_seam`, `uv.unwrap`, `uv.project_view`, `uv.snap_pixels`, `uv.pack(padding)`.

### paint (FR-PAINT)
- `paint.texture_new(w, h, assign_to_selection)` (FR-PAINT-06).
- `paint.pixels(image, [{x,y,rgba}])`, `paint.rect`, `paint.fill(image, x, y, rgba)` (FR-PAINT-01). Agent paints by pixel lists, not strokes.
- `paint.stroke_3d(points, rgba, size)` (FR-PAINT-02).
- `paint.palette_set`, `paint.palette_lock(bool)` (FR-PAINT-04).
- `paint.vertex_color(rgba)`, `paint.bake_ao(samples)` (FR-PAINT-05). Bake is (long).

### art (FR-ART)
- `art.link(image, path)`, `art.unlink(image)`, `art.reload(image)` (FR-ART-01, FR-ART-02).
- `art.import(path, as: tileset|texture)` (FR-ART-06).
- Not commands: the watcher, PSD flattening and "open in external editor" (human only).

### scene (FR-SCENE)
- `scene.rename`, `scene.parent`, `scene.reorder`, `scene.visibility`, `scene.lock` (FR-SCENE-01).
- `scene.object_from_selection(name)`, `scene.origin_to_cursor` (FR-SCENE-02).
- `scene.instance_add(source, transform)` (FR-SCENE-03).
- `scene.marker_add(name, type, props)` (FR-SCENE-04).
- `scene.set_collision(name, bool)` (FR-SCENE-05). `scene.set_props(name, map)` (FR-SCENE-06).
- `scene.duplicate`, `scene.mirror(axis)` (FR-SCENE-07).
- `scene.delete(names)`.

### rig (FR-RIG)
- `rig.skeleton_new(name)`, `rig.bone_add(skeleton, name, parent?, head?, tail?)` (FR-RIG-01), `rig.bone_edit`, `rig.bone_delete`.
- `rig.bind_auto(object, skeleton)` (FR-RIG-02). (long)
- `rig.weight_paint(object, bone, vertices | stroke, mode, value)` (FR-RIG-03).
- `rig.bind_rigid(object | faces, bone)` (FR-RIG-04).
- `rig.ik_add(tip_bone, target, pole)`, `rig.ik_set(target?, pole?)` (FR-RIG-06).
- `rig.mirror_weights(axis)`, `rig.mirror_bones(axis)` (FR-RIG-07).
- `rig.pose(bone, rotation | translation)`, `rig.pose_reset` (FR-RIG-08).

### anim (FR-ANIM)
- `anim.clip_new(skeleton, name, length, loop)`, `anim.clip_edit`, `anim.clip_duplicate`, `anim.clip_delete` (FR-ANIM-01).
- `anim.key_set(clip, bone | ik, channel, time, value?, interp?)` (keys the current pose if value omitted), `anim.key_move`, `anim.key_delete`, `anim.key_copy`, `anim.key_paste`, `anim.autokey(bool)` (FR-ANIM-02, FR-ANIM-03, FR-ANIM-05).
- `anim.curve_handles(clip, track, key, handles)` (FR-ANIM-06).
- `anim.playhead(time)`, `anim.play`, `anim.stop` (FR-ANIM-04). Headless playback is stepped, not real time.
- `anim.crossfade_preview(from, to, fade)` (FR-ANIM-08).
- `anim.mirror_pose`, `anim.mirror_clip` (FR-ANIM-09).

### export / import (FR-EXP, FR-IMP)
- `export.run(preset | {path, format, selection, scale}) -> files, validator report` (FR-EXP-01 to FR-EXP-05). (long)
- `export.preset_save(name, ...)` (FR-EXP-06).
- `export.sprite_sheet(clip, camera, frames, path)` (FR-EXP-07). (long)
- `import.gltf(path) -> objects, skeletons, clips` (FR-IMP-01). (long)

### history and control
- `history.undo`, `history.redo` (FR-STATE-01, FR-MCP-03).
- `control.cancel(request id)` (FR-MCP-07). Also triggered by the Stop button.

### MCP resources (FR-MCP-09)
- `quarry://guide`: units, axes, naming (`.L`/`.R`, `spawn_*`), a sample workflow, and "screenshot from front, side and iso after every major step".
- `quarry://commands`: the registry list with one-line descriptions.

## Permissions

| Action | Human | Agent |
|---|---|---|
| Every command above | yes | yes |
| Write files (save, export, sprite sheet) | anywhere | only inside the open project's folder; `forbidden` otherwise |
| Read files (import, link) | anywhere | any path given in the call |
| Delete files on disk | no command exists | no command exists |
| App settings, shortcuts, theme | Settings UI | no command exposed |
| Quit, close window | yes | no command exposed |
| Open in external editor | yes | no command exposed |

Enforced in `quarry-server`: each agent request's file paths are canonicalised and checked against the project folder before the command runs.

## Pages, screens and key components

### Start screen
- **Purpose:** new, open, recent, sample robot, first-run overlay (FR-PROJ-06, FR-PROJ-08, FR-STATE-07).
- **Components:** recent list with thumbnails and missing markers; New (name, tile size); Open; Sample robot; recovery prompt when an autosave is newer than its file (FR-PROJ-04).
- **States:** empty recent list ("No projects yet"), missing file, recovery available.

### Editor window
- **Purpose:** everything else (FR-UI-01).
- **Components:**
  - Top bar: menu, mode switch (Tile, Edit, Paint, Rig, Animate), agent status pill ("Claude is working", Stop) (FR-MCP-07).
  - Viewport element: wgpu image, gizmo, crosshair, overlays, grid; orbit/pan/zoom/pinch; pressure for brushes (FR-VIEW, FR-PAINT-02, FR-RIG-03).
  - Side panel tabs: Tileset (tileset picker, brush, palette), UVs (UV editor element), Paint (2D pixel editor element, palette strip, tools), Scene (tree with filter, properties, markers, collision), Rig (skeletons, bones, IK, weight brush settings).
  - Timeline (bottom): clip picker, per-bone tracks, keys, playhead, play controls, graph editor toggle, onion skin, crossfade preview (FR-ANIM).
  - Status bar: selection and mesh counts, tool, agent status (FR-UI-04).
  - Command search overlay (FR-UI-02). History panel (FR-STATE-08). Console with errors (FR-STATE-04).
- **States:** empty scene hint (FR-STATE-02); empty timeline hint; progress overlay for long commands (FR-STATE-03); missing linked image badge; toasts for errors and warnings.

### Settings window
- **Purpose:** FR-UI-03, FR-UI-05. Shortcuts editor with conflict warning and presets, UI scale, theme, external editor, autosave interval, weight ramp.

### Export dialog
- **Purpose:** FR-EXP-01, FR-EXP-06. Format (GLB default), selection, scale, path, save as preset, result with validator summary.

### Connect Claude dialog
- **Purpose:** FR-MCP-08. Shows the `quarry-mcp` path and copyable config for Claude Code (`claude mcp add quarry -- <path>`) and the desktop app JSON.

## State management
- **Source of truth:** the `Document` in `quarry-core`, one per open project, owned by the main thread as a GPUI entity in the app (or by the headless runner).
- **Changes:** only through `CommandRegistry::execute`. After a commit the document bumps a revision number; views and the renderer compare revisions and redraw.
- **Renderer:** receives an immutable `RenderSnapshot` (meshes uploaded once and re-uploaded only for changed objects by revision per object).
- **Undo/redo:** owned by `History` in `quarry-core`, shared by human and agent, 200 entries minimum, cleared on project close.
- **Socket and MCP:** the server thread parses requests and sends them to the main thread through a channel; results return on a oneshot. Commands run strictly in arrival order (Stage 02: no locking needed).
- **UI-only state** (panel sizes, tab, zoom, tool options) lives in GPUI view state and the settings file, never in the document.
- **Persistence:** save writes the document; autosave writes a snapshot from a background thread (clone of the document at a revision).

## Background and long-running tasks
- **Autosave** : timer every 2 minutes if the revision changed; clones the document, writes on a worker, never blocks input (FR-PROJ-03).
- **File watcher** : `notify` debounced 100 ms; on change, hash, compare with `source_hash`, decode on a worker, then `art.reload` as a human-sourced history entry (FR-ART-02).
- **Open, save, export, import, bake AO, auto weights, sprite sheet** : run on a worker against a document clone; report progress 0 to 1 every 100 ms to the progress overlay and to the socket as `{"id":1,"progress":0.4}` lines; on success the result commits as one history entry; on cancel or error nothing commits.
- **Viewport rendering** : on demand after a revision, camera or playback change; readback path double-buffers staging buffers and maps async.
- **Headless runner** : `quarry --headless --socket <path>` runs the same document and registry with no window; screenshots render offscreen; exits when the last client disconnects and no unsaved changes remain, or after 10 minutes idle (unsaved changes autosaved first).

## Security
- Socket path in the per-user runtime folder (macOS/Linux `$TMPDIR` or `XDG_RUNTIME_DIR`, file mode 0600; Windows named pipe with a per-user ACL). No TCP listener.
- Every request validated against its schema before execution; sizes capped (pixel lists 65,536 entries, mesh slices 10,000 items, screenshots 4096x4096).
- Agent path checks as in Permissions; symlinks resolved before the check.
- Zip reading: reject absolute paths and `..` entries, cap total uncompressed size at 2 GB and each entry at 512 MB (zip-bomb guard).
- PSD and PNG decode on a worker with size caps (16,384 px per side).
- No network calls (NFR-03); a CI test runs the app headless with networking blocked and checks no connection is attempted.
- No secrets exist in the app.

## Logging and observability
- `tracing` to a rolling log file in the app data folder (7 days) and the in-app console.
- Logged: command name, source, duration, ok/error code; file open/save/export paths; watcher reloads; renderer timings when the debug overlay is on.
- Not logged: pixel data, full mesh data, file contents.
- Crash: a panic hook writes the log tail and triggers the recovery path next launch (FR-PROJ-04).

## Testing strategy
- **Unit (`cargo test`, no GPU):** every command in `quarry-core` and `quarry-ops`: success, failure-changes-nothing, undo returns to identical state (property test: random command sequences then undo all equals start).
- **Golden files:** export the sample robot, a tiled room and a 3-bone bar; compare JSON structure to checked-in goldens; run the Khronos glTF Validator on each (CI, npm `gltf-validator`).
- **Round trip:** save -> open equality on documents; import(export(x)) keeps vertices, weights, bones and sampled clip values within tolerance.
- **Animation:** sampled export within 0.5 degrees of Quarry playback (FR-ANIM-10).
- **MCP:** coverage test that every command in the registry is a tool with a schema (FR-MCP-01); scripted agent session through `quarry-mcp` against headless Quarry building a box, rigging, keying and exporting.
- **Renderer:** headless screenshots on Linux CI with lavapipe compared to reference PNGs with a small tolerance.
- **Performance:** a benchmark scene of 100k triangles, 50 bones, 5 skinned meshes; frame time logged on the M1 manually per milestone (NFR-01).
- **Manual per milestone:** Cairn import of the golden GLBs; pen pressure on Mac and Windows; Krita/Affinity/Photoshop live refresh.
- Run: `cargo test --workspace`, `cargo run -p xtask -- validate-gltf`, `cargo run -p xtask -- bench`.

## Environment and configuration
- `QUARRY_LOG` : log level, default `info`.
- `QUARRY_SOCKET` : override socket path, default per-user runtime path.
- `QUARRY_HEADLESS_IDLE_SECS` : headless idle exit, default 600.
- `QUARRY_DATA_DIR` : override app data folder (tests).
No secrets, nothing in the repo to protect.

## Migrations and seed data
- `project.json` carries `format_version`; `quarry-io` runs migration functions in order on open and saves in the newest version. Opening a newer file than the app knows shows an error, never a lossy load.
- Mesh `.bin` files carry their own small header version.
- Seed data: `assets/samples/` holds the CC0 16 px tileset and `sample-robot.quarry` (13 bones, idle and walk), built in Quarry itself by M6 and checked in.
- No import from other tools' project formats (Stage 03 out of scope).

---

### Open questions

None.

### Assumptions confirmed

1. **Coordinate system : RESOLVED.** Right-handed, Y up, metres, matching glTF (Stage 02 units).
2. **Bone influences : RESOLVED.** 4 per vertex (Stage 03).
3. **MCP tool names : RESOLVED.** `area.verb` internally, `area_verb` as MCP tool names.
4. **History depth : RESOLVED.** 200 entries or 512 MB, oldest dropped.
5. **Headless idle exit : RESOLVED.** 10 minutes with no client, autosave first.
6. **Agent painting : RESOLVED.** Pixel lists and rectangles, plus `paint.stroke_3d`.
