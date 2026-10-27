---
id: "m8-connect-claude-2026-10-26"
status: "review"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:40:00.000Z"
labels: ["m8", "mcp", "ui"]
order: 71
---

# M8 · Connect Claude dialog

FR-MCP-08.

**Build**
- Help → Connect Claude: the resolved `quarry-mcp` path, the Claude Code line `claude mcp add quarry -- <path>`, and the desktop app JSON snippet, both copyable.
- Detect a packaged vs development binary path and say which one is shown.
- Short troubleshooting note (path, restart the client, `/mcp` check).

**Done when**
- [ ] Paste the shown line into Claude Code: `/mcp` lists Quarry as connected.
- [ ] The dialog works on macOS, Windows and Linux paths.
- [ ] Copy buttons write the exact text with no smart quotes.

**UI (2026-10-26)**
Connect Claude dialog with the quarry-mcp command for Claude Code and the desktop config, copy button and live connection state. Checked end to end: a scripted MCP client built a low-poly tree in the open window.
