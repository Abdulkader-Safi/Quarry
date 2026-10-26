---
id: "m1-mcp-bridge-2026-10-26"
status: "review"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T20:30:00.000Z"
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
- [x] The coverage test fails if a command is added without a tool.
- [ ] Claude Code connects over stdio and `project.new` through MCP appears in the open window.

## Progress 2026-10-26

- `quarry-mcp` (rmcp 3.7.0, stdio): `tools/list` is generated from `CommandRegistry::commands()` (`area.verb` → `area_verb`, description and JSON schema straight from the command); `tools/call` goes over the local socket, so calls run as `Source::Agent` in the open window's history. If nothing answers, it starts `quarry --headless` from its own folder.
- Resource `quarry://commands` lists every tool with its description.
- Coverage test fails if a command has no tool or a tool has no command.
- Checked here with a scripted stdio client: 17 tools listed; project_new + mesh_add_primitive + project_save write the file through an auto-started headless Quarry; an outside write is refused; history reads "Agent: new", "Agent: add primitive"; headless exits when the bridge leaves.
- Left for review: `claude mcp add quarry -- <repo>/target/release/quarry-mcp`, then `/mcp` shows Quarry connected. The "appears in the open window" half needs the app shell to serve the socket (app shell card).
- Note: Unix socket paths are limited to about 104 bytes; a long `QUARRY_SOCKET` fails with that reason.
