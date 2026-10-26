---
id: "m2-image-links-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T22:30:00.000Z"
labels: ["m2", "images", "art"]
order: 25
---

# M2 · Link images, embed copies, handle resizes

FR-ART-01, FR-ART-03, FR-ART-04. Live art from Krita or Affinity without losing work.

**Build**
- `art.link` / `art.reload` / `art.import`: remember `source_path` + `source_hash`, always keep the embedded PNG. Missing source shows a badge, project still opens.
- PSD reads the flattened composite; the M0 message appears if the composite is absent.
- If the new image size differs, keep UVs in pixel positions and warn instead of stretching.
- Unlink keeps the embedded copy.

**Done when**
- [ ] Move the source away: the project opens from the embedded copy with a missing-link badge.
- [ ] A 3-layer Affinity PSD matches its flattened PNG export.
- [ ] Canvas-resize 64→128 in the editor: existing faces show the same pixels and a warning appears.

## Backend done 2026-10-26 (UI waits for the Figma design)

- `art.import(path, as)`, `art.link`, `art.unlink`, `art.reload` (no-op when the file hash is unchanged), `art.watch` (new: per-image live refresh toggle, saved), `query.images` (link path, missing flag).
- Size changes keep UVs in pixels and return a warning; undo restores the old pixels.
- Tested: moved source still opens from the embedded copy with missing = true; 64 to 128 resize keeps UVs and warns.
- Left: the missing-link badge UI, and the Affinity 3-layer PSD check (with the M0 PSD review).
