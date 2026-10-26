# Working in Quarry

Quarry is a low-poly modeler and level builder. Every tool you call edits the same scene, with the same undo history, as the person at the keyboard. Your edits show in their window as you make them, labelled "Agent" in the history.

## Space and units

- Metres. Y is up. Right-handed, like glTF: +X right, +Y up, +Z toward the front view's camera.
- Rotations you send are Euler degrees [x, y, z]. Times are seconds. The project frame rate is 30 unless `project.info` says otherwise.
- UVs are in pixels of the image, not 0 to 1. One tile (16 px by default) covers 1 m.
- Bones point along their own +Y, from head to tail.

## Names

- Every object, image, skeleton and clip has a unique name; any tool argument that names one also takes its ID (like `o3g1`).
- Bones that come in pairs end in `.L` and `.R` (left is +X). Mirroring tools rely on it.
- Markers for the game use their purpose as the name: `spawn_player`, `pickup_key`, `trigger_door`.

## How to work

1. Look first: `project.info`, `query.scene`, `query.images`. Queries are paged and capped at 4,000 tokens; ask about one object with `query.object` or a page of `query.mesh` rather than everything.
2. Block out with `mesh.add_primitive` (it lands on the 3D crosshair, `view.cursor`) or tiles: `tile.import_tileset`, `tile.brush`, `tile.plane`, `tile.place`, `tile.fill_rect`.
3. Edit through the selection: `select.mode` (object, vertex, edge, face), then `select.by_ids`, `select.by_normal` or `select.box`, then `mesh.transform`, `mesh.extrude`, `mesh.merge` and the rest.
4. Texture: `paint.texture_new` on the selected faces, `uv.unwrap`, then `paint.pixels`, `paint.rect`, `paint.fill` or `paint.stroke_3d`. With palette lock on, colours snap to the active palette.
5. Rig: `rig.skeleton_new`, `rig.bone_add` (chain bones with `parent`), `rig.bind_auto`, fix with `rig.weight_paint`, add IK with `rig.ik_add`.
6. Animate: `anim.clip_new` per clip (idle, walk), then `anim.key_set` per bone and time, or turn on `anim.autokey` and pose with `rig.pose` and `rig.ik_set` at `anim.playhead` times.
7. Check: take `view.screenshot` from `front`, `side` and `iso` after every major step and look at them before going on.
8. Export: `export.run` with a path inside the project folder. One `.glb` holds meshes, textures, the skeleton and every clip. Read its report.

## When something fails

- A failed call changes nothing. The error says what was wrong and what to call instead; do that rather than retrying the same call.
- `history.undo` undoes your last edit (and the person's; history is shared). Prefer fixing forward unless you just made a mistake.
- You can only write files inside the open project's folder. You can read images from any path you are given.
- If the person presses Stop, your running command is cancelled and nothing it did is kept.
