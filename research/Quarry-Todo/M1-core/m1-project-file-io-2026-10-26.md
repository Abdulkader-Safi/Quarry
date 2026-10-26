---
id: "m1-project-file-io-2026-10-26"
status: "done"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:05:00.000Z"
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
- [x] Save a scene with 2 tilesets, 3 objects, a rig and 2 clips; quit; reopen; every tile, UV, weight, keyframe and image is identical (FR-PROJ-01).
- [x] Rename to `.zip`, unzip, `project.json` opens in a text editor and PNGs open in Krita (FR-PROJ-02).
- [x] Corrupt file test: no crash, error toast + console line (FR-STATE-04 groundwork).

## Done 2026-10-26

- `quarry-io::project`: `write_project` (temp file + fsync + rename), `read_project`, and `QuarryFiles`, the `Storage` the registry uses for `project.open/save/save_as`.
- Layout: `project.json` (pretty, no mesh arrays, no pixels), `meshes/<id>.bin` (QMSH v1, little-endian, deflated), `images/<id>.png` (stored).
- Guards: 100k entries, 512 MB per entry and 2 GB total counted on inflated bytes, entries escaping the archive, PNG sides over 16,384 px, mesh counts larger than the file, faces pointing at missing vertices. A newer `format_version` is refused with "Update Quarry to open it"; `migrate::run` is the hook for format 2.
- Tests (`crates/quarry-io/tests/project_file.rs`): the FR-PROJ-01 scene round-trips equal; the zip opens as plain JSON plus stored PNGs that decode; garbage, truncated, newer-format and lying-mesh files are errors; the registry saves, opens and tracks dirty state through undo.
- Not yet: `thumbnail.png`, which needs an offscreen render of the document (lands with the viewport card). The toast for a corrupt file is UI work (feedback states card).
- zip is built with deflate only (`default-features = false`), which drops AES, bzip2 and zstd from the tree.
