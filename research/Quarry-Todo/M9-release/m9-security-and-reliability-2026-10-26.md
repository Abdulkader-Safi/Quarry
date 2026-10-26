---
id: "m9-security-and-reliability-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T11:00:00.000Z"
labels: ["m9", "nfr", "security"]
order: 76
---

# M9 · No-network proof and crash reliability

NFR-03 and NFR-04.

**Do**
- Prove no network: test or trace showing no outbound connections, no telemetry, no update check; MCP local socket only.
- Fuzz or hand-corrupt project files, images and settings: never crash, always a plain error.
- Crash test: kill mid-edit, worst case loses the last 2 minutes, original file intact.
- Confirm the agent cannot write outside the project folder (repeat the M8 checks on a packaged build).

**Done when**
- [x] The no-network check runs in CI or is documented as a release step.
- [x] A corrupt file of each kind opens or fails with a message, never a panic.
- [x] No secret or absolute user path is written into a project file.

## Done 2026-10-27

- No network: `cargo run -p xtask -- no-network` (and the CI job of the same name) fails if `quarry` or `quarry-mcp` links an HTTP client, TLS or WebSocket crate, or if our code uses TCP or UDP sockets. MCP is stdio plus the per-user local socket.
- Corrupt files (`crates/quarry-io/tests/corrupt.rs`): the sample project, a tileset PNG, two golden GLBs, a PSD and settings.json are each truncated 24 ways and bit-flipped 40 ways; every reader returns a result or an error, never a panic.
- No absolute user paths in project files: linked image paths are saved relative to the project folder (`art/tiles.png`, `../shared/robot.psd`) and made absolute again on open; other drives stay absolute. Test checks project.json holds no part of the temp folder path.
- Crash recovery and the agent write limits were tested in M1 and M8 (`autosave.rs`, `permissions.rs`, the scripted session). Repeat on a packaged build at release.
