---
id: "m8-screenshot-tool-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T09:00:00.000Z"
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
- [x] MCP calls for front, side and iso at 768x768 return three PNGs matching the view.
- [x] Headless screenshots work with no window.
- [x] Cost stays sane: one image per call, default size respected.

## Done 2026-10-27

- `view.screenshot(preset?, position?, target?, width?, height?)`, 768x768 by default, 16 to 4096 px: presets frame the whole visible scene. Rendering goes through `Storage::screenshot` to `quarry_render::screenshot_png` (the shared offscreen renderer, the document's display toggles), so it works headless.
- The reply carries the PNG as base64; `quarry-mcp` turns it into MCP image content plus a short text block, so the model sees the picture.
- Checked with the scripted session: front, side and iso each came back as one image/png (13 to 36 KB).
