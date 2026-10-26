---
id: "m6-mirror-rigging-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T05:30:00.000Z"
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
- [x] Paint the left leg weights, mirror: right-leg vertices get matching weights on `.R` bones.
- [x] Mirror a 3-bone arm chain: right arm bones exist with mirrored head/tail and swapped names.
- [x] A bone without a mirror partner produces a warning naming it.

## Done 2026-10-27

- `rig.mirror_bones`: every .L (or _L) bone gets a .R partner mirrored across X, parents mapped to their partners, existing partners updated. `rig.mirror_weights(object)`: +X vertices copy their weights to their -X twins with bones swapped; bones without a partner are named in a warning.
- Tested: a 3-bone .L arm mirrors to .R with negated X and mirrored parents; +X weights on lower.L land on the -X vertices as lower.R at 1.0.
