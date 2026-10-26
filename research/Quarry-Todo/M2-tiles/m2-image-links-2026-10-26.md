---
id: "m2-image-links-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
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
