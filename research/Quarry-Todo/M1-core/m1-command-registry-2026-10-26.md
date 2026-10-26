---
id: "m1-command-registry-2026-10-26"
status: "done"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T17:30:00.000Z"
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
- [x] A failing command leaves the document byte-identical.
- [x] A command that panics mid-way does not commit partial state.
- [x] Every command serialises to a schema and appears in the command list.

## Done 2026-10-26

- `CommandRegistry::execute(name, args, source)` deserialises args into the typed command (schemars schema per command), runs it on a working copy inside `catch_unwind`, and commits one history entry only when the document changed. Queries and saves never enter history.
- Commands so far: project.new/open/save/save_as/info/set_settings, history.undo/redo, control.cancel, query.scene/history, scene.rename/visibility/lock/delete, mesh.add_primitive (box; other shapes in M3).
- `project.open`/`save` reach `quarry-io` through a `Storage` trait set at startup, so core keeps every command struct without depending on io.
- Tests: seven error paths leave the document byte-identical with the right code; a command that edits then panics commits nothing; every command has `area.verb`, a schema and a one-sentence description.
