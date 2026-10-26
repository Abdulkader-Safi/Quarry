---
id: "m8-screenshot-tool-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m8", "mcp", "render"]
order: 68
---

# M8 · Viewport screenshots from MCP

FR-VIEW-06. The agent's eyes.

**Build**
- `view.screenshot`: camera preset or explicit position, size (default 768x768), PNG.
- Returned as MCP image content, not a file path.
- Uses the same renderer and display state as the viewport; works headless.
- Agent guide tells the model to screenshot after each major step.

**Done when**
- [ ] MCP calls for front, side and iso at 768x768 return three PNGs matching the view.
- [ ] Headless screenshots work with no window.
- [ ] Cost stays sane: one image per call, default size respected.
