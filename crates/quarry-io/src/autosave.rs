//! Autosave, backups and crash recovery (FR-PROJ-03, FR-PROJ-04, FR-PROJ-05).
//!
//! Autosaves are side copies in the app data folder, never the user's file:
//! `autosave/<key>.quarry` plus `autosave/<key>.json` saying which project it
//! belongs to. Backups are the previous versions of a project file, 10 kept.

use crate::settings::{Dirs, write_atomic};
use quarry_core::model::Document;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const BACKUPS_KEPT: usize = 10;

/// Which project an autosave belongs to, written next to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sidecar {
    /// The user's file, or None for a project never saved.
    pub project: Option<PathBuf>,
    pub name: String,
    /// Seconds since the Unix epoch.
    pub saved_at: u64,
}

/// An autosave newer than its project file, offered on launch.
#[derive(Clone, Debug, PartialEq)]
pub struct Recovery {
    pub autosave: PathBuf,
    pub sidecar: Sidecar,
}

/// Stable short key for a project path (FNV-1a), or one per process when untitled.
fn key(project: Option<&Path>) -> String {
    match project {
        Some(path) => {
            let hash = path
                .to_string_lossy()
                .bytes()
                .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                    (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
                });
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            format!("{stem}-{hash:016x}")
        }
        None => format!("untitled-{}", std::process::id()),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Write the side copy. Safe to call from a worker thread with a document clone.
pub fn write_autosave(
    dirs: &Dirs,
    doc: &Document,
    project: Option<&Path>,
) -> Result<PathBuf, String> {
    let base = dirs.autosaves().join(key(project));
    let file = base.with_extension("quarry");
    let bytes = crate::project::encode(doc)?;
    write_atomic(&file, &bytes).map_err(|e| format!("Autosave failed: {e}."))?;
    let sidecar = Sidecar {
        project: project.map(Path::to_path_buf),
        name: doc.name.clone(),
        saved_at: now_secs(),
    };
    let json = serde_json::to_vec_pretty(&sidecar).map_err(|e| e.to_string())?;
    write_atomic(&base.with_extension("json"), &json)
        .map_err(|e| format!("Autosave failed: {e}."))?;
    Ok(file)
}

/// Drop the side copy once the user's file holds the same work (after a save or
/// a clean close), or when the user declines recovery.
pub fn clear_autosave(dirs: &Dirs, project: Option<&Path>) {
    let base = dirs.autosaves().join(key(project));
    let _ = std::fs::remove_file(base.with_extension("quarry"));
    let _ = std::fs::remove_file(base.with_extension("json"));
}

/// Remove one autosave found by [`recoverable`].
pub fn discard(recovery: &Recovery) {
    let _ = std::fs::remove_file(&recovery.autosave);
    let _ = std::fs::remove_file(recovery.autosave.with_extension("json"));
}

/// Autosaves newer than their project file (or for projects never saved),
/// newest first.
pub fn recoverable(dirs: &Dirs) -> Vec<Recovery> {
    let Ok(entries) = std::fs::read_dir(dirs.autosaves()) else {
        return Vec::new();
    };
    let mut found: Vec<Recovery> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|meta| {
            let sidecar: Sidecar = serde_json::from_slice(&std::fs::read(&meta).ok()?).ok()?;
            let autosave = meta.with_extension("quarry");
            if !autosave.is_file() {
                return None;
            }
            let newer = match &sidecar.project {
                None => true,
                Some(project) => {
                    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
                    match (modified(&autosave), modified(project)) {
                        (Some(a), Some(p)) => a > p,
                        (Some(_), None) => true, // the project file is gone
                        _ => false,
                    }
                }
            };
            newer.then_some(Recovery { autosave, sidecar })
        })
        .collect();
    found.sort_by_key(|r| std::cmp::Reverse(r.sidecar.saved_at));
    found
}

/// Copy the current file into the backups folder before it is overwritten,
/// keeping the newest 10.
pub fn backup_before_save(dirs: &Dirs, project: &Path) -> Result<(), String> {
    if !project.is_file() {
        return Ok(());
    }
    let folder = dirs.backups().join(key(Some(project)));
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Could not make the backup folder: {e}."))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let mut target = folder.join(format!("{stamp}.quarry"));
    let mut n = 1;
    while target.exists() {
        target = folder.join(format!("{stamp}-{n}.quarry"));
        n += 1;
    }
    std::fs::copy(project, &target)
        .map_err(|e| format!("Could not back up {}: {e}.", project.display()))?;

    let mut backups: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "quarry"))
        .collect();
    backups.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    while backups.len() > BACKUPS_KEPT {
        let _ = std::fs::remove_file(backups.remove(0));
    }
    Ok(())
}

/// Decides when the next autosave is due: every `interval`, and only when the
/// document changed since the last one.
pub struct Autosaver {
    pub interval: Duration,
    last_at: Instant,
    last_revision: u64,
}

impl Autosaver {
    pub fn new(minutes: u32, revision: u64) -> Self {
        Self {
            interval: Duration::from_secs(minutes.max(1) as u64 * 60),
            last_at: Instant::now(),
            last_revision: revision,
        }
    }

    /// True when an autosave should start now; it then counts as done.
    pub fn due(&mut self, now: Instant, revision: u64) -> bool {
        if now.duration_since(self.last_at) < self.interval || revision == self.last_revision {
            return false;
        }
        self.last_at = now;
        self.last_revision = revision;
        true
    }
}
