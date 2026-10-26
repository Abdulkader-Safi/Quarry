---
id: "m8-guide-samples-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T10:00:00.000Z"
labels: ["m8", "mcp", "samples"]
order: 72
---

# M8 · Agent guide resource and shipped samples

FR-MCP-09 and FR-PROJ-08.

**Build**
- `quarry://guide`: units (metres, Y up, right-handed), axes, naming conventions, the sample workflow (block out, paint, rig, animate, check, export), screenshot after every major step, error-recovery habits.
- `quarry://commands`: generated command list with schemas.
- `assets/samples/`: CC0 16 px tileset and `sample-robot.quarry`; "Sample robot" on the start screen opens it with skeleton and idle clip.

**Done when**
- [ ] An agent reading only the guide builds and exports a rigged two-clip prop without asking how.
- [x] The sample tileset licence is CC0 and credited.
- [ ] Start screen opens the sample robot and idle plays.

## Progress 2026-10-27

- `quarry://guide` (`crates/quarry-mcp/src/guide.md`): units, axes, names, the eight-step workflow from looking first to exporting, screenshot after every step, error habits. Listed first in resources; the MCP server instructions point to it. `quarry://commands` was already there.
- `cargo run -p xtask -- agent-session` runs `xtask/agent/agent_session.py`: reads the guide, then builds, paints, unwraps, rigs, keys two clips, takes three screenshots and exports a crate through `quarry-mcp` against an auto-started headless Quarry. 4 to 6 s here; the export passes the Khronos validator with no issues.
- Left: the CC0 16 px tileset and `sample-robot.quarry` in `assets/samples/` (the robot is built in M6 and M7 by hand), and the start screen button.

## Samples done 2026-10-27

- `cargo run -p xtask -- samples` writes `assets/samples/tiles-16px.png` (8x8 tiles of 16 px drawn in code: grass, dirt, stone, brick, wood, metal, water, sand) and `sample-robot.quarry`, built only through registry commands: 11 box parts bound rigidly, a painted 32x32 torso texture, 13 bones (root, spine, chest, neck, head, upper/lower arm and thigh/shin per side, .R mirrored), leg IK, and clips "idle" (2 s breath and head turn) and "walk" (1 s, feet stepping through IK targets, arms swinging, body bobbing). `assets/samples/LICENSE.md`: CC0, no third-party art.
- Checked: walk at 0.25 s renders with knees bent toward the pole; the export passes the validator with no issues; a test opens the file and finds 13 bones and both clips.
- Found on the way: the renderer drew skinned meshes in rest pose. The snapshot now skins on the CPU, so the viewport, thumbnails and agent screenshots show the current pose (test added).
- Left: the "Sample robot" button on the start screen.
