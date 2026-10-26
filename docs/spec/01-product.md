# Quarry

Status: signed off 2026-10-26

> **Changes:** 2026-10-26: removed every reference to the product this was modelled on, at Safi's request. Reference images stay in `references/` but are not linked from the spec. No requirement changed.

## Mission

Quarry is where the stones for a Cairn come from. Safi is building his own game engine, Cairn, and his own games on it (first up: the low-poly twin-stick robot shooter in [[Projects/Games/index|Games]]). Every model, level piece, rig and animation those games need has to come from somewhere. Today that means Blender, which is heavy for PS1-style low-poly work, or closed tile-based pixel-art editors that have no API and cannot be driven by an AI.

Quarry is a personal tool first. It is built privately and goes open source under MIT once it works, the same path as Cairn. There is no business model. That shapes everything downstream: no accounts, no licence server, no cloud, no telemetry, and features are chosen by what Safi's games need, not what sells.

## One-liner

A desktop low-poly modeler and level builder for Mac, Windows and Linux that combines tile placement (pixel-art tiles on a 3D grid) with free mesh editing, painting, weighted rigging, IK and keyframe animation, exports clean glTF for Cairn, and exposes every editor action to AI agents through an MCP server.

## The problem

Safi needs a steady supply of small game assets: robots with 10 to 20 bones, enemies, props, and modular level pieces for eight sketched levels. The tools that exist split the job badly:

- **Closed tile-based pixel-art editors** have the right workflow for this art style (place tiles, paint, bone, animate) but no scripting, plugin API or command interface, so an AI cannot touch them.
- **Blockbench** (free, GPL) models, paints, rigs and animates, and has a community MCP plugin, but that MCP has no rig or animation tools and Blockbench has no tile or level workflow.
- **Blender** (free, GPL) does everything, and dcc-mcp-blender can create armatures, bind meshes and set keyframes over MCP. But Blender is a general 3D suite: pixel-art tiles need the stale Sprytile add-on, and every small prop means fighting a tool built for film.

The shared flaw: the tools that fit low-poly pixel-art work are closed to AI, and the tools open to AI are not built for low-poly pixel-art work.

The gap: one lightweight editor where tiles, meshes, paint, rigs and animation live together, where art from Affinity, Krita or Photoshop refreshes live on the model, and where an AI agent can do every one of those things through named tools and check its own work with screenshots.

## The value

The "after" picture. Safi asks Claude: "Build a scout robot for level 3, four legs, rig it, give it idle and walk clips." Claude calls Quarry's MCP tools: lays out the body with box and wedge primitives, paints a 32x32 texture from the robot palette, adds a 13-bone skeleton, runs auto weights, fixes the hip weights with the weight brush, sets IK targets on the feet, keys a 1-second walk loop, takes a screenshot from three angles to check it, and exports `scout.glb`. Safi watches it happen in the open Quarry window, tweaks one antenna by hand, saves, and drops the .glb into his Cairn project, where the skeleton and both clips appear as assets.

The interaction difference from today's tools: they are either hand-only or need the AI to write raw scripts. In Quarry the human and the AI drive the same editor, through the same actions, into the same undo history.

Versus Blockbench, the closest free tool, Safi gets:
- An MCP server where every editor action is a tool, rigging and animation included : Blockbench's MCP plugin has no rig or animation tools.
- Tile placement next to free low-poly mesh editing : levels and props in one tool, where Blockbench has no tile or level workflow.
- Weight painting and IK : smooth deformation and fast posing instead of rigid parts.
- Reliable live refresh of PNG and PSD from Affinity, Krita and Photoshop : paint in the app you like, see it on the model on save.
- glTF output tested against Cairn's importer : what Quarry exports is what Cairn plays.

What it deliberately is not:
- Not a sculpting or high-poly tool. Over about 20,000 triangles per model, use Blender.
- Not a PBR texture painter. Textures are small, flat, usually pixel art. No normal, roughness or metal maps in v1.
- Not a game engine. No physics, scripting or play mode. That is Cairn's job.
- Not a renderer for stills or film. Viewport preview and screenshots only.
- Not a drawing app replacement. The paint panel covers pixel touch-ups; serious art happens in Affinity, Krita or Photoshop.
- Not an AI image or mesh generator. The AI works through editor tools; no diffusion model is built in.

## Product category

Low-poly and pixel-art 3D asset editor, in the same shelf as Blockbench and picoCAD. It competes with tile-based editors on workflow, with Blockbench on being free and rig-capable, and with Blender + an MCP on AI control. The white space is the combination of tile workflow, skinned animation and a full AI tool surface in one small app.

## Business model and licence

None. Personal tool, private repo while it is built, MIT once it works (same as Cairn's "private for now, open source soon"). No accounts, no payments, no keys. Dependencies must be MIT-compatible so the switch to MIT stays possible.

## Platforms

macOS, Windows and Linux on day one, as desktop apps. Safi's main machine is a Mac, so macOS is where things get used first; Windows and Linux must build and pass the same tests in CI from the first milestone.

## What success looks like

One year in: every model in the twin-stick robot shooter (player, at least four robot types, props and modular pieces for the eight sketched levels) was made in Quarry, rigged and animated there, and loads in Cairn with no fix-up step. At least half of those assets were started by Claude over MCP and finished by hand. Safi has not opened Blender for a game asset in months.

Measurable signals:
- A 13-bone robot with idle and walk clips exported from Quarry loads in Cairn with the skeleton and both clips intact, first try.
- Claude builds a textured, rigged, two-clip prop from one prompt over MCP in under 10 minutes, with no manual repair needed to export.
- A PNG saved in Krita shows on the model in Quarry within 1 second.
- The repo goes public under MIT.

## UI and product references

- **Layout**: a 3D crosshair as the anchor for placing tiles and bones, a tabbed side panel (Tileset, UVs, Paint, Scene, Rig), selection and triangle counts in the status bar, per-bone animation tracks with a clip dropdown. Large readable UI, a camera that never fights you, and live refresh that is visible, not buried in settings.
- **Blockbench** (blockbench.net): keep the animation timeline with a graph editor for curves, and the clean outliner.
- **Aseprite**: keep the pixel paint panel feel: palette strip, pixel-perfect pencil, nearest-neighbour zoom.
- **Blender**: keep the weight-paint colour ramp (blue to red) and the IK target and pole concept, nothing else of its UI.
- **Cairn editor** (cairn.safi-studio.com): keep the JSON command model where every action is undoable and a failed command changes nothing.

---

### Open questions

None.

### Assumptions confirmed

1. **Name : RESOLVED.** Quarry, chosen over Knap, Chisel and Cobble (2026-10-26).
2. **Cairn link : RESOLVED.** Standalone, glTF/GLB export only. No live link into a Cairn project; Safi copies the file in.
3. **Modeling style : RESOLVED.** Both tile placement and free low-poly mesh editing with painted textures.
4. **Licence : RESOLVED.** Private while built, MIT later.
5. **Platforms : RESOLVED.** macOS, Windows and Linux on day one.
6. **External art : RESOLVED.** Live refresh from Affinity and Krita / Photoshop, so PNG and PSD.
7. **MCP depth : RESOLVED.** Every editor action is an MCP tool.
8. **Animation : RESOLVED.** Weights, IK and keyframes in v1; blending trimmed to a crossfade (see 9).
9. **v1 scope : RESOLVED.** Weights + IK + keyframes in v1. Blend preview is a crossfade between two clips, matching Cairn's Fade. Blend trees out of scope.
10. **Name clash : RESOLVED.** Keep Quarry despite the 2022 game The Quarry. Repo `quarry-editor`, binary `quarry`.
11. **Levels for Cairn : RESOLVED.** Named marker nodes and a collision flag exported as glTF node extras. No Cairn-specific format.
12. **Watching the AI : RESOLVED.** Both. Live into the open window by default, headless for batch jobs.
13. **Pixel-art defaults : RESOLVED.** New projects use nearest filtering, 16 px tiles and palette-locked painting; smooth filtering is one toggle away.
