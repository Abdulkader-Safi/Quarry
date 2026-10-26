---
id: "m1-command-server-headless-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
- [ ] A socket client can `project.new`, `project.save` and read `query.scene`.
- [ ] Headless save works with no window open.
- [ ] Cancel during a long command leaves the document unchanged.
