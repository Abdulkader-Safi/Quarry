---
id: "m1-mcp-bridge-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "mcp"]
order: 15
---

# M1 · `quarry-mcp` bridge with generated tools

FR-MCP-01 and FR-MCP-03. Never hand-write the tool list.

**Build**
- `rmcp =3.7.0` stdio server in `quarry-mcp`, bridging to the socket; tool names `area_verb`, descriptions and typed inputs from the registry schema.
- Agent commands pass `Source::Agent` so history labels them and one Cmd+Z undoes them.
- Resource `quarry://commands` (guide content arrives in M8).
- CI test: every registry command has an MCP tool and vice versa.

**Done when**
- [ ] The coverage test fails if a command is added without a tool.
- [ ] Claude Code connects over stdio and `project.new` through MCP appears in the open window.
