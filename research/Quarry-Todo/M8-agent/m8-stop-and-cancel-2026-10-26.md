---
id: "m8-stop-and-cancel-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m8", "mcp", "ui"]
order: 70
---

# M8 · Stop button and visible agent state

FR-MCP-07.

**Build**
- Agent status pill shows idle, working and the current tool name; Stop cancels the running command.
- Cancel restores the pre-command state (M1 `control.cancel`), and the history shows nothing for the cancelled command.
- Agent edits appear live in the viewport; the human can keep orbiting while the agent works.

**Done when**
- [ ] Start a long bake over MCP, press Stop: the bake stops and the scene matches its state before the bake.
- [ ] Stop mid-export leaves no partial file on disk.
- [ ] Two long commands cannot run at once; the second waits or is refused with a clear message.
