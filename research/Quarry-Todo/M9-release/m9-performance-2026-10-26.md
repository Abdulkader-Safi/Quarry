---
id: "m9-performance-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m9", "nfr"]
order: 75
---

# M9 · Performance pass

NFR-01 and NFR-10, measured on the M1 MacBook Air 8 GB.

**Do**
- 60 fps with 100k triangles, 50 bones and 5 skinned meshes playing.
- Open and save a 100k-triangle project in under 2 s; export .glb in under 2 s.
- Cold start to an empty scene in under 2 s.
- Profile before optimising; record before/after numbers. No micro-optimising below the target.

**Done when**
- [ ] Every number is in `PROGRESS.md` with the machine named.
- [ ] A benchmark command in `xtask` reproduces them.
- [ ] Startup does no file I/O beyond settings and the recent list.
