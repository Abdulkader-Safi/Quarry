# Quarry

Quarry is a desktop low-poly modeler and level builder for macOS, Windows and Linux. You place pixel-art tiles on a 3D grid, edit low-poly meshes, paint pixels, rig with two-bone IK, animate, and export one self-contained `.glb` per asset for the Cairn engine. Every editor action is also an MCP tool, so Claude can edit the same scene, with the same undo history, as you.

It is a personal tool: local only, no accounts, no network.

## Status

Quarry is at milestone M0, which de-risks the stack before any feature work starts. What runs today is a set of spikes, not the editor:

- a GPUI window showing a wgpu render of 100k triangles over 5 skinned meshes, with a pen pressure readout
- our own GLB writer and a 3-bone bending bar that passes the Khronos glTF Validator
- a PSD reader that takes the flattened image Affinity or Photoshop saved

`PROGRESS.md` has the measurements and decisions so far. `docs/spec/06-build-spec.md` is the full plan, and `research/Quarry-Todo/` holds the task board.

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

The viewport spike opens a 1600x1000 window, prints fps once a second and shows the latest pen pressure in the top-left corner. Use a release build; debug builds are much slower.

```bash
cargo run -p quarry --release
```

Flags for the spike:

```bash
cargo run -p quarry --release -- --secs 10          # quit after 10 seconds
cargo run -p quarry --release -- --latency-check    # post 30 clicks, print click-to-frame latency (macOS)
cargo run -p quarry --release -- --bench            # renderer timing with no window, 300 frames
cargo run -p quarry --release -- --bench --frames 1000
cargo run -p quarry --release -- --check            # render one frame offscreen and read it back
cargo run -p quarry --release -- --present-check    # compare the NV12 present path with the scene (macOS)
```

Click in the window to print `click_to_frame_ms` for that click. Draw with a tablet pen, or press hard on a Force Touch trackpad, to see the pressure value change.

`cargo run -p quarry -- --headless` and the `quarry-mcp` server arrive in M1.

## Repo tasks

```bash
cargo run -p xtask -- validate-gltf                      # Khronos validator over tests/goldens
cargo run -p xtask -- psd-check file.psd flattened.png   # compare a PSD's composite with its PNG export
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

## Licence

Private for now; MIT later. Every dependency is on the cargo-deny allow list in `deny.toml`.
