# Users and usage

Status: signed off 2026-10-26

> **Changes:** 2026-10-26: removed every reference to the product this was modelled on, at Safi's request. Reference images stay in `references/` but are not linked from the spec. No requirement changed.

Builds on [[Projects/Games/quarry/spec/01-product|Stage 01 product]] (signed off 2026-10-26).

## Personas

### Primary: "Safi," the engine builder
- **Background:** Senior software engineer who wrote his own C++20/Vulkan engine, Cairn, and is making a low-poly twin-stick robot shooter on it. Eight levels sketched, a robot cast in mind. Not a trained 3D artist: knows Blender basics, draws in Affinity, Krita and Photoshop.
- **What drives them:** Owning the whole pipeline, from engine to assets, with nothing closed in the middle. Getting a playable level in front of him fast, then polishing. Using AI where it saves hours, and checking what it did.
- **Current workaround:** Blender for models and rigs, plus the Blender level pipeline note in `game-practice-ideas/`. Every small prop means opening a film-grade tool, and glTF export needs settings checked each time. Closed tile-based pixel-art editors suit the art style but cannot be scripted or driven by Claude.
- **Tech comfort:** High. Installs from source, uses a terminal daily, edits JSON config, reads glTF in a hex viewer if he has to. Low patience for UI that hides features.
- **Relationship to AI:** Wants Claude to do the first 80% (block out, rig, rough animation) while he watches, then take over by hand. Trusts it only when he can see and undo every step.
- **Devices and environment:** Mac (main) with trackpad, mouse and keyboard; a Windows/Linux PC with an NVIDIA card; a pen tablet for painting. English UI. Works offline often.

### Secondary: "Claude," the agent
- **Background:** An AI agent (Claude Code in a terminal, or Claude in the desktop app) connected to Quarry through its MCP server. Gets a task in plain words: "make a 4-legged scout robot, rig it, give it idle and walk".
- **What drives them:** Finishing the task correctly without a human repairing it. Knowing the current state before acting, and checking the result after.
- **Current workaround:** With Blender, writes raw Python through blender-mcp and hopes it runs. With closed tile editors, cannot act at all.
- **Tech comfort:** Reads tool descriptions and JSON perfectly, cannot see the viewport unless given a screenshot, cannot drag a mouse, has a limited context window so huge scene dumps hurt it.
- **Relationship to the editor:** Needs named, single-purpose tools with clear inputs, errors that say what to fix, screenshots from chosen angles, compact scene queries, and an undo it can call. A failed tool must change nothing.
- **Devices and environment:** Runs on the same machine as Quarry over a local MCP connection. No network access to Quarry from elsewhere.
- **Why it matters:** Half of the one-year success goal (Stage 01) depends on assets the agent starts. **Why it is not the beachhead:** every tool still has to make sense to a human first, because the human finishes the work.

### Not the target
- Professional 3D artists doing high-poly, sculpting, PBR texturing or film. Blender and Maya serve them; Quarry will not add a node editor, sculpt brushes or a renderer.
- People wanting one-click AI mesh generation from a prompt. Quarry has no generator model.
- Other engines' users are not excluded (glTF is open), but no Unity, Godot or Unreal-specific export or plugin will be built.

## Roles and permissions

One local user, two drivers of the same editor.

| Driver | Can see | Can do | Cannot do |
|---|---|---|---|
| Human (keyboard, mouse, trackpad, pen) | Everything | Everything | |
| Agent (MCP) | Scene state via queries, screenshots it asks for | Every editor action, save, export inside the project folder | Delete files on disk, write outside the project folder, change app settings or shortcuts, quit the app |

Both drivers write to the same undo history. Commands apply in the order they arrive; there is no locking or turn-taking, because Safi makes sure he and the agent do not edit at the same time. The human can stop the agent's current command at any time.

## Jobs to be done

- When **Safi needs a level piece**, he wants to **place tiles from a tileset on a grid and see it in 3D**, so he can **block out a level in minutes**.
- When **Safi needs a character or prop**, he wants to **box-model it from primitives and paint it**, so he can **get a game-ready low-poly model without Blender**.
- When **Safi paints a texture in Affinity, Krita or Photoshop**, he wants to **see it on the model the moment he saves**, so he can **judge the art in 3D, not flat**.
- When **a model needs to move**, he wants to **add bones, get decent auto weights and fix the bad ones with a brush**, so he can **animate without the mesh tearing**.
- When **posing a leg or arm**, he wants to **drag an IK target instead of rotating each bone**, so he can **key a walk cycle in minutes**.
- When **a model is done**, he wants to **export one .glb with mesh, texture, skeleton and named clips**, so he can **drop it into Cairn and play**.
- When **Safi delegates**, he wants to **tell Claude what to build and watch it build in the open window**, so he can **step in, correct, and keep the good parts**.
- When **Claude gets a build task**, it wants to **query the scene, act through named tools, screenshot from several angles and fix what looks wrong**, so it can **hand back an asset that exports cleanly**.
- When **a level needs gameplay data**, Safi wants to **drop named markers and flag collision meshes**, so he can **read spawn points and colliders in Cairn later**.
- When **an old asset needs changing**, he wants to **reopen the project file with tilesets, rig and clips intact**, so he can **edit instead of rebuild**.

## Primary user journey

1. **Discovery:** Safi's own idea. After MIT release, other people find it on GitHub through the pitch "tile-based low-poly editor with rigging and a full MCP".
2. **Setup:** Download the app for Mac, Windows or Linux and open it. No account, no key. To use AI, add Quarry's MCP server to Claude Code or the Claude desktop app with one config line shown in Quarry's Help menu. Under 5 minutes.
3. **First use:** A new project opens with pixel-art defaults, a sample 16 px tileset and the grid. He places a few tiles and orbits the camera.
4. **Aha moment:** He saves a PNG in Krita and the model in Quarry updates under a second later. Or: he asks Claude for a crate and watches it appear, textured, in his window.
5. **Optional power-up:** Rig a character: add bones, auto weights, weight brush, IK targets on the feet, key a walk loop, preview a crossfade from idle to walk.
6. **Finish:** Export `.glb` (or OBJ for static props). Copy into the Cairn project's Assets folder. Cairn shows the mesh, skeleton and clips.
7. **Ongoing use:** Every new asset for the game starts here, by hand or by asking Claude.

## Core loop

**The repeated action:** build (tiles, primitives, vertex edits) -> paint or refresh texture -> rig and animate if it moves -> export .glb -> try it in Cairn -> back to Quarry to fix.

**What pulls users back:**
- Every Cairn game needs a stream of small assets, and Quarry is the shortest path from idea to .glb.
- Claude can do the dull first pass, so starting an asset costs one sentence.
- Live refresh makes painting in his preferred app feel native.

## Language, accessibility and locale

- **Language:** English only. All UI strings in one strings file so translation stays possible. No RTL.
- **Units:** One grid unit = one metre in glTF export (glTF's unit). Tile size in pixels (default 16). Time in seconds on the timeline, with an optional frames view at 30 fps.
- **Input:** Mouse and keyboard (middle-drag orbit), Mac trackpad (two-finger orbit, pinch zoom, Shift+two-finger pan, no middle button needed anywhere), pen tablet with pressure for paint and weight brushes.
- **Shortcuts:** Quarry's own default scheme, every action rebindable, plus a Blender-style preset. Every action also reachable from a menu or command search, so no shortcut is required.
- **Accessibility:** UI scale from 100% to 200%. Minimum UI text 13 px at 100%. Weight colour ramp offered as blue-to-red and a colour-blind-safe option. Visible keyboard focus in panels.
- **Fonts:** One UI font bundled, with a monospace font for numbers and the console.

---

### Open questions

None.

### Assumptions confirmed

1. **Secondary user : RESOLVED.** The AI agent, treated as a real user. Future open-source users are not designed for in v1.
2. **Input devices : RESOLVED.** Mouse + keyboard, Mac trackpad, and pen tablet with pressure all supported.
3. **Shortcuts : RESOLVED.** Own default scheme, fully rebindable, with a Blender-style preset.
4. **Language : RESOLVED.** English only, strings in one file.
5. **Agent connection : RESOLVED.** Local MCP server over stdio that talks to the running Quarry or starts a headless one. Works with Claude Code and the Claude desktop app. No remote access.
6. **Human and agent at once : RESOLVED.** Both can edit. No locking, queue or pause logic: Safi keeps them from working at the same time. Shared undo history, Stop button for the agent's current command.
7. **Agent file access : RESOLVED.** Reads images from paths it is given, writes only inside the open project folder.
8. **Sample content : RESOLVED.** Ship one CC0 16 px tileset and one rigged sample robot.
