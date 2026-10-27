---
id: "m9-performance-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T15:50:00.000Z"
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
- [x] A benchmark command in `xtask` reproduces them.
- [x] Startup does no file I/O beyond settings and the recent list (plus the autosave listing for crash recovery).

## Progress 2026-10-27

- `cargo run --release -p xtask -- bench`: 100,800 triangles; save 66 ms, open 13 ms, export 52 ms on the M2 Pro (table in PROGRESS.md).
- Left: the same on the M1 Air 8 GB; 60 fps with 5 skinned meshes playing in the app viewport; cold start once the app shell exists.

## Progress 2026-10-27 (afternoon)

- `xtask bench` now also times the editor's viewport path with 100,800 triangles, 50 bones and 5 skinned meshes playing: about 70 fps at 1600x1000 and at 2360x1600 on the M2 Pro.
- The app logs `first frame N ms after launch`; release cold start 223 to 466 ms here.
- Numbers in PROGRESS.md. Left: the same run on the M1 Air 8 GB (needs that machine).
