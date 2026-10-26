---
id: "m1-settings-recents-logging-2026-10-26"
status: "done"
priority: "critical"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T18:35:00.000Z"
labels: ["m1", "settings", "infra"]
order: 13
---

# M1 · Settings, recent files and logging

FR-PROJ-06, FR-UI-05, NFR-06 groundwork.

**Build**
- App settings in the OS config dir via `directories` 6.0.0 + serde_json, atomic writes: UI scale, theme, shortcuts, preset, external editor, autosave minutes, weight ramp, last 10 projects with thumbnails, window state.
- Moved or deleted recent project shows as missing, not a crash.
- Env config: `QUARRY_LOG`, `QUARRY_SOCKET`, `QUARRY_DATA_DIR`, `QUARRY_HEADLESS_IDLE_SECS`.
- `tracing` to a rolling file kept 7 days, plus an in-memory buffer for the in-app console.

**Done when**
- [x] Settings survive a restart; a corrupted settings file falls back to defaults instead of failing to launch.
- [x] Recent list shows three projects most recent first; one moved file is marked missing.

## Done 2026-10-26

- `quarry-io::settings`: `Settings` (every Stage 05 field, `#[serde(default)]` so older files load), atomic save, `Dirs` from `directories` or `QUARRY_DATA_DIR`. A damaged file is moved to `settings.json.bad` and defaults load; out-of-range values clamp.
- Recent list: newest first, deduplicated, 10 max, `recent_with_status` marks moved files missing.
- `quarry-io::logging`: daily `quarry.<date>.log` kept 7 files via `tracing-appender` 0.2 (MIT, new dependency), plus a 1,000-line in-memory tail for the console. `QUARRY_LOG` sets the filter.
- `QUARRY_SOCKET` and `QUARRY_HEADLESS_IDLE_SECS` land with the server card.
- Thumbnails in the recent list need the offscreen document render (viewport card).
