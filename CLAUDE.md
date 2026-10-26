# Quarry

Desktop low-poly modeler and level builder for macOS, Windows and Linux. Pixel-art tiles placed on a 3D grid, free low-poly mesh editing, pixel painting, weighted rigging with two-bone IK, keyframe animation, and one self-contained `.glb` per asset for Cairn. Every editor action is also an MCP tool, so an agent and a human edit the same document and the same undo history.

Personal tool: local only, no accounts, no network, private repo now, MIT later.

## Source of truth

- **Spec:** `docs/spec/06-build-spec.md` is the hand-off document. `01`–`05` are background.
- **Work board:** `research/Quarry-Todo/` — one markdown card per task, grouped by milestone. Read the matching card before starting work.
- **Status:** `PROGRESS.md` — milestone state, M0 measurements, decisions and deviations.

If the spec turns out to be wrong in practice, stop and explain. Never silently swap a stack choice. The only pre-approved swap is GPUI → egui if M0 misses its numbers.

## Build order

M0 de-risk → M1 core, shell and command server → M2 tiles and linked images → M3 mesh, UV and scene → M5 glTF export (POC ends) → M4 paint → M6 rig → M7 animation → M8 agent experience → M9 hardening and release.

Finish M0 and record its numbers before any feature work. Work the cards in order; `optional` cards are dropped first when time runs short. Don't add features outside the spec — note them in `PROGRESS.md` and ask.

## Working a card

Use the `mdk` skill for the board (`research/Quarry-Todo/`). Every card is one task; its frontmatter `status` is the truth.

1. Read the card and the spec section it points at.
2. Set `status: in-progress` and refresh `modified` before writing code.
3. Do the whole change: the callers, tests, fixtures and config it reaches.
4. When finished, move the card:
   - `review` if it needs a visual check — anything a person must look at: UI layout, viewport, paint, rig, animation, export output, packaging.
   - `done` if it does not. Note what was completed in the body.
5. Commit after each step, not just each milestone: one commit per card, or per coherent step inside a large card. Name the card and milestone in the message.
6. Run `cargo fmt --all`, clippy, test — and `cargo deny check` when dependencies change — before committing.
7. Keep `PROGRESS.md` current when a milestone moves or a measurement lands.

## Repository layout

```
Cargo.toml            workspace, exact pins
deny.toml             licence allow list, bans
crates/
  quarry-core/        Document, IDs, CommandRegistry, History
  quarry-ops/         mesh, UV, tile, paint, weights, IK, sampling (pure functions)
  quarry-render/      wgpu renderer, offscreen targets, screenshots, picking
  quarry-io/          .quarry zip, migrations, glTF in/out, PNG/PSD, watcher, settings
  quarry-server/      local socket server, headless runner, permissions
  quarry-mcp/         rmcp stdio binary bridging to quarry-server
  quarry/             GPUI app: views, panels, viewport, input, timeline
xtask/                validate-gltf, bench, golden updates
assets/samples/       CC0 16 px tileset, sample-robot.quarry
tests/goldens/        golden .glb exports and reference screenshots
docs/spec/            this spec (01 to 06)
```

## Commands

```bash
cargo fmt --all                       # format
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check                      # licences, bans, advisories
cargo run -p quarry                   # the app (M1+)
cargo run -p quarry -- --headless     # no window, batch jobs
cargo run -p quarry-mcp               # MCP stdio server
```

CI runs fmt, clippy, test and cargo-deny on `macos-14`, `windows-latest` and `ubuntu-22.04`, plus the Khronos glTF Validator on the export goldens.

## Rules that never bend

- Only `CommandRegistry::execute(command, source)` mutates the `Document`. UI, shortcuts, command search, socket and MCP all go through it.
- Every user-facing action is a typed command with a `schemars` schema; every command is an MCP tool (`area.verb` → `area_verb`). A CI test fails if one is missing.
- A failed command changes nothing: run on a working copy, commit one history entry or discard.
- One undo history for human and agent, labelled by source (`Agent: extrude`).
- No GPUI below the app crate — `core`, `ops`, `render`, `io`, `server` must stay portable.
- Local only: no network calls, no TCP listener, per-user socket. Agent writes only inside the open project folder.
- glTF is the contract: every export passes the Khronos validator and loads in Cairn and Blender.
- Free licences only (see `deny.toml`). Never patch an MPL crate in place.
- Pin GPUI exactly; upgrade only in its own commit.

## Conventions

- Rust stable, edition 2024, Cargo workspace.
- Right-handed, Y up, metres; rotations stored as quaternions; time in seconds; project fps 30 by default.
- UVs are stored in pixels, not 0–1 — snapping and padding stay exact.
- IDs are u32 slot + generation written as short strings; every named thing is addressable by name or ID.
- Errors say what happened and what to do, in plain words.
- User-facing strings live in one file; English only for v1.
- Commit after each step, naming the card and milestone. Keep `PROGRESS.md` current.
