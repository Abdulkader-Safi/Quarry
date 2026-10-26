---
id: "m8-agent-permissions-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T09:00:00.000Z"
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
- [x] Export to `/tmp/x.glb` outside the project is refused with a clear error.
- [x] A symlink pointing outside the project is refused too.
- [x] Reads of a provided image path outside the project succeed.

## Done 2026-10-27

- `quarry-server::permissions`: paths canonicalised with symlinks resolved; agent writes (`project.new`, `project.save_as`, `export.run`, and `project.save` to its real target) only inside the project folder or the headless `--dir`; agents may only open projects inside those folders; `project.recover` is human-only; reads of image paths are allowed anywhere.
- No commands exist for deleting files, settings, shortcuts, quitting or external editors.
- Tested: unit tests for outside paths, `..`, relative paths, a symlink out of the folder, open and save targets; the scripted MCP session's export to /tmp/outside.glb is refused naming the allowed folder.
