---
id: "m1-command-server-headless-2026-10-26"
status: "done"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T19:50:00.000Z"
labels: ["m1", "server", "mcp"]
order: 14
---

# M1 · Local command server and headless mode

FR-MCP-05. The socket is how the agent reaches the same document the human has open.

**Build**
- `quarry-server` on `interprocess` 2.4.7: per-user Unix socket on macOS/Linux, named pipe on Windows.
- JSON lines in `{"id","command","args"}` out `{"id","ok","result"|"error"}`, plus `{"id","progress"}` lines for long commands.
- `control.cancel` and `history.cancel` stop the running command and restore the pre-command state.
- Single-instance handshake so a second launch can hand work to the open window.
- `quarry --headless` with no window, plus a `--commands` batch file for CI and batch jobs.

**Done when**
- [x] A socket client can `project.new`, `project.save` and read `query.scene`.
- [x] Headless save works with no window open.
- [x] Cancel during a long command leaves the document unchanged.

## Done 2026-10-26

- `quarry-server`: `Server` (interprocess 2.4.7 listener, one thread per client, requests forwarded to the document owner in arrival order), `Client` (tests, MCP bridge), JSON lines with `{"id","progress"}` lines for long commands.
- `control.cancel` is answered on the connection thread: it sets the registry's cancel flag while a command runs; the command returns `cancelled` and its working copy is dropped.
- Socket: Unix socket file in `$XDG_RUNTIME_DIR` / `$TMPDIR` when private (0700), else `/tmp/quarry-<user>/` created 0700; Windows named pipe `quarry-<user>` (default pipe security, noted). `QUARRY_SOCKET` overrides. A stale socket file from a crash is replaced; a live one is refused.
- Agent write check (`permissions.rs`): `project.new` and `project.save_as` paths must resolve (symlinks, `..`) inside the project folder or the headless `--dir`; `forbidden` otherwise. `project.new` gained an optional `path` (spec 05: "path must be in a project folder it was given").
- `quarry --headless [--socket] [--dir] [--commands file.jsonl]`: batch mode prints one reply per line; server mode autosaves on the timer, exits when the last client leaves with nothing unsaved, or after `QUARRY_HEADLESS_IDLE_SECS` (600) idle with unsaved work autosaved.
- Every command is logged by the registry (name, source, ms, error code).
- Tests (`crates/quarry-server/tests/socket.rs`): new/add/save/query over the socket with progress lines and a refused outside write; cancel mid-command leaves the document unchanged; headless saves with no window and exits when the client leaves.
- Moved to the app shell card: the single-instance handoff (a second launch passing a file to the open window), since it needs the app's open-file path.
