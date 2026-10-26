---
id: "m6-mirror-rigging-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m6", "rig"]
order: 55
---

# M6 · Mirror bones and weights .L / .R

FR-RIG-07.

**Build**
- `rig.mirror_bones`: mirror a chain across X with names swapped `.L` ↔ `.R`.
- `rig.mirror_weights`: for matched vertices, map each bone to its mirrored counterpart (`.L` ↔ `.R`), same weights.
- Unmatched names are reported, not silently dropped; mirroring is one undo entry.

**Done when**
- [ ] Paint the left leg weights, mirror: right-leg vertices get matching weights on `.R` bones.
- [ ] Mirror a 3-bone arm chain: right arm bones exist with mirrored head/tail and swapped names.
- [ ] A bone without a mirror partner produces a warning naming it.
