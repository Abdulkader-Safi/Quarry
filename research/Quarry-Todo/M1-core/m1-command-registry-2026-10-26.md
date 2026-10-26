---
id: "m1-command-registry-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "core", "mcp"]
order: 10
---

# M1 · Command registry with schemas and one door

The single entry point everything else calls (build spec §3 principle 1).

**Build**
- Typed command structs named `area.verb` (`project.new`, `mesh.extrude` …) with `schemars` 1.2.3 schemas and descriptions.
- `CommandRegistry::execute(command, source)`: run on a working copy, commit as one history entry or discard everything.
- Error codes `invalid_args, not_found, nothing_selected, conflict, out_of_bounds, forbidden, io, cancelled, internal`; messages say what to do next ("No faces selected. Call select first.").
- Selectors take ID or name; expose a `commands` list (name, description, schema) for MCP, command search and shortcuts.

**Done when**
- [ ] A failing command leaves the document byte-identical.
- [ ] A command that panics mid-way does not commit partial state.
- [ ] Every command serialises to a schema and appears in the command list.
