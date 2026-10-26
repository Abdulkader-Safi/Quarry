---
id: "m1-project-file-io-2026-10-26"
status: "todo"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m1", "io", "project"]
order: 11
---

# M1 · `.quarry` zip read/write with atomic save

FR-PROJ-01 and FR-PROJ-02. The file format is the promise that work is never lost.

**Build**
- Zip layout: `project.json`, `meshes/<object>.bin`, `images/<image>.png` (stored, not recompressed), `thumbnail.png`.
- Save, save-as, open, new; atomic write (temp file + rename); `format_version` plus a migration hook; zip-bomb guards (entry count, uncompressed size, path traversal).
- Corrupt or hostile file returns a plain error naming the file; the app keeps running.

**Done when**
- [ ] Save a scene with 2 tilesets, 3 objects, a rig and 2 clips; quit; reopen; every tile, UV, weight, keyframe and image is identical (FR-PROJ-01).
- [ ] Rename to `.zip`, unzip, `project.json` opens in a text editor and PNGs open in Krita (FR-PROJ-02).
- [ ] Corrupt file test: no crash, error toast + console line (FR-STATE-04 groundwork).
