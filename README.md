# Quarry

Quarry is a desktop low-poly modeler and level builder for macOS, Windows and Linux. You place pixel-art tiles on a 3D grid, edit low-poly meshes, paint pixels, rig with two-bone IK, animate, and export one self-contained `.glb` per asset for the Cairn engine. Every editor action is also an MCP tool, so Claude can edit the same scene, with the same undo history, as you.

It is a personal tool: local only, no accounts, no network.

## Status

The editor window follows the Figma design: start screen, the editor with its Tileset, UVs, Paint, Scene and Rig tabs, the timeline, history and agent panels, command search, Export, Settings, Connect Claude, the basics overlay and the live-refresh toast, in dark and light themes. The viewport draws whatever is in the document: tiles, primitives you extrude and edit, painted textures, skinned and posed meshes, and any low-poly glTF model you import.

Everything also works headless and through MCP: projects, autosave and crash recovery, tiles and live-linked art, mesh editing, UVs, painting, rigging with IK, animation, and glTF export and import. When the window is open, `quarry-mcp` drives it and you watch the agent work (`cargo run -p xtask -- agent-session` runs a scripted session).

`PROGRESS.md` has the measurements and decisions. `docs/spec/06-build-spec.md` is the full plan, and `research/Quarry-Todo/` holds the task board.

## Requirements

- Rust 1.97.1. `rust-toolchain.toml` pins it, so `rustup` installs it on the first build.
- macOS 13+, Windows 10/11 x64 or Ubuntu 22.04+ x64.
- On Linux, GPUI's system libraries:

  ```bash
  sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
    libx11-xcb-dev libxcb1-dev libfontconfig-dev libvulkan-dev
  ```

- Node.js 22, only for running the glTF validator.

## Run it

```bash
cargo run -p quarry --release                       # the editor, on the start screen
cargo run -p quarry --release -- scene.quarry       # open a project straight away
```

Try the sample robot from the start screen (13 bones, idle and walk clips), or the sample tileset. In the view: Alt-drag or middle-drag orbits, Shift pans, scroll zooms, F frames everything, 1 to 4 switch object, vertex, edge and face selection, G, R and T pick the move, rotate and scale gizmo (the magnet turns snapping on, Shift flips it), E extrudes, ⌘K searches every command, ⌘Z undoes.

`QUARRY_UI="side=rig;bottom=timeline;overlay=export"` opens panels at launch, for screenshots and demos.

The M0 viewport spike and the renderer checks are still there:

```bash
cargo run -p quarry --release -- --spike --secs 10  # the spike window, quit after 10 seconds
cargo run -p quarry --release -- --spike --latency-check    # post 30 clicks, print click-to-frame latency (macOS)
cargo run -p quarry --release -- --bench            # renderer timing with no window, 300 frames
cargo run -p quarry --release -- --bench --frames 1000
cargo run -p quarry --release -- --check            # render one frame offscreen and read it back
cargo run -p quarry --release -- --present-check    # compare the NV12 present path with the scene (macOS)
```

In the spike, click to print `click_to_frame_ms` for that click. Draw with a tablet pen, or press hard on a Force Touch trackpad, to see the pressure value change.

Headless mode runs the same document and commands with no window:

```bash
cargo run -p quarry -- --headless                                  # serve the local socket
cargo run -p quarry -- --headless --dir ~/levels                   # agents may also write in ~/levels
cargo run -p quarry -- --headless --commands jobs.jsonl            # run one JSON command per line, then exit
```

A command line looks like `{"id":1,"command":"mesh.add_primitive","args":{"kind":"box"}}`.

## Connect Claude

`quarry-mcp` is an MCP server over stdio. Every Quarry command is one of its tools. It talks to the running Quarry, or starts a headless one if none is open.

```bash
cargo build --release -p quarry -p quarry-mcp
claude mcp add quarry -- "$PWD/target/release/quarry-mcp"
```

Agents can write files only inside the open project's folder (or the folder a headless Quarry was started in).

## Repo tasks

```bash
cargo run -p xtask -- validate-gltf                      # Khronos validator over tests/goldens
cargo run -p xtask -- psd-check file.psd flattened.png   # compare a PSD's composite with its PNG export
cargo run -p xtask -- agent-session                      # a scripted agent builds and exports a crate over MCP
cargo run -p xtask -- samples                            # rebuild assets/samples (CC0 tileset, sample robot)
cargo run -p xtask -- no-network                         # prove no HTTP, TLS or sockets
cargo run --release -p xtask -- bench                    # time save, open and export of 100k triangles
```

`validate-gltf` installs the validator from npm into `xtask/validator/node_modules` the first time it runs.

## Develop

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check                     # licences, bans, advisories
```

To rewrite the golden files after an intended change to export output:

```bash
QUARRY_UPDATE_GOLDENS=1 cargo test -p quarry-io --test skin_bar
```

CI runs fmt, clippy, tests, cargo-deny and the glTF validator on `macos-14`, `windows-latest` and `ubuntu-22.04`.

## Layout

```
crates/
  quarry-core/     Document, IDs, CommandRegistry, History
  quarry-ops/      mesh, UV, tile, paint, weights, IK, sampling
  quarry-render/   wgpu renderer, offscreen targets, screenshots, picking
  quarry-io/       .quarry zip, glTF in/out, PNG/PSD, watcher, settings
  quarry-server/   local socket server, headless runner, permissions
  quarry-mcp/      MCP stdio server bridging to quarry-server
  quarry/          the GPUI app
xtask/             validate-gltf, psd-check
tests/goldens/     golden .glb exports
docs/spec/         product spec, 01 to 06
```

## Install a release

Releases are built by CI when a `v*` tag is pushed (`.github/workflows/release.yml`): a `.dmg` for Apple Silicon and one for Intel Macs, an NSIS installer for Windows, and an AppImage for Linux. v1 is not code-signed, so each OS asks once:

- **macOS:** open the `.dmg`, drag Quarry to Applications, then right-click Quarry and choose Open, and Open again. Or run `xattr -dr com.apple.quarantine /Applications/Quarry.app`.
- **Windows:** SmartScreen says "Windows protected your PC". Click "More info", then "Run anyway".
- **Linux:** `chmod +x Quarry_*.AppImage`, then run it. Ubuntu 22.04 needs `libfuse2` (`sudo apt install libfuse2`).

To build an installer yourself, for the OS you are on:

```bash
cargo install cargo-packager --version 0.11.8 --locked
cargo packager --release --formats app,dmg      # macOS
cargo packager --release --formats nsis         # Windows
cargo packager --release --formats appimage     # Linux
```

Installers land in `target/packages/`. Both `quarry` and `quarry-mcp` are inside, so `quarry-mcp` can start a headless Quarry when no window is open.

## Licence

Private for now; MIT later. Every dependency is on the cargo-deny allow list in `deny.toml`.
