---
id: "m8-agent-permissions-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m8", "mcp", "security"]
order: 69
---

# M8 · Agent file permissions and refusals

FR-MCP-06.

**Build**
- Canonicalise every path, resolve symlinks; writes only inside the open project's folder.
- Reads allowed from paths the agent is given (for image import).
- No commands exist for deleting files, changing settings or shortcuts, quitting, or opening external editors.
- Refusal errors explain the rule and the allowed folder.

**Done when**
- [ ] Export to `/tmp/x.glb` outside the project is refused with a clear error.
- [ ] A symlink pointing outside the project is refused too.
- [ ] Reads of a provided image path outside the project succeed.
