//! Live refresh (FR-ART-02): watch the folders of linked images and report
//! which images may have changed. The owner of the document then runs
//! `art.reload` for each, which compares file hashes, so Quarry's own writes
//! and editors that touch a file without changing it never cause a reload.
//!
//! Folders are watched rather than files because editors often save by
//! writing a new file and renaming it over the old one.

use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use quarry_core::id::Id;
use quarry_core::model::{Document, Image};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub const DEBOUNCE: Duration = Duration::from_millis(100);

pub struct ArtWatcher {
    debouncer: notify_debouncer_full::Debouncer<
        notify::RecommendedWatcher,
        notify_debouncer_full::RecommendedCache,
    >,
    rx: Receiver<Vec<PathBuf>>,
    folders: BTreeSet<PathBuf>,
}

impl ArtWatcher {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = channel();
        let debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
            if let Ok(events) = result {
                let paths = events.into_iter().flat_map(|e| e.event.paths).collect();
                let _ = tx.send(paths);
            }
        })
        .map_err(|e| format!("Could not start watching linked art: {e}."))?;
        Ok(Self {
            debouncer,
            rx,
            folders: BTreeSet::new(),
        })
    }

    /// Watch exactly the folders holding watched, linked images. Call after
    /// each commit; it is cheap when nothing changed.
    pub fn sync(&mut self, doc: &Document) {
        let wanted: BTreeSet<PathBuf> = watched(doc)
            .filter_map(|(_, path)| path.parent().map(Path::to_path_buf))
            .filter(|dir| dir.is_dir())
            .collect();
        for gone in self
            .folders
            .difference(&wanted)
            .cloned()
            .collect::<Vec<_>>()
        {
            let _ = self.debouncer.unwatch(&gone);
            self.folders.remove(&gone);
        }
        for new in wanted
            .difference(&self.folders)
            .cloned()
            .collect::<Vec<_>>()
        {
            match self.debouncer.watch(&new, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.folders.insert(new);
                }
                Err(e) => tracing::warn!("cannot watch {}: {e}", new.display()),
            }
        }
    }

    /// Images whose file was touched since the last call (may be unchanged).
    pub fn changed(&self, doc: &Document) -> Vec<Id<Image>> {
        let touched: BTreeSet<PathBuf> = self.rx.try_iter().flatten().map(canonical).collect();
        if touched.is_empty() {
            return Vec::new();
        }
        watched(doc)
            .filter(|(_, path)| touched.contains(&canonical(path.clone())))
            .map(|(id, _)| id)
            .collect()
    }
}

fn watched(doc: &Document) -> impl Iterator<Item = (Id<Image>, PathBuf)> + '_ {
    doc.images
        .iter()
        .filter(|i| i.watch)
        .filter_map(|i| Some((i.id, PathBuf::from(i.source_path.as_ref()?))))
}

/// Events can name a path through a symlinked folder (macOS /var → /private/var).
fn canonical(path: PathBuf) -> PathBuf {
    match (
        path.parent().and_then(|p| p.canonicalize().ok()),
        path.file_name(),
    ) {
        (Some(dir), Some(name)) => dir.join(name),
        _ => path,
    }
}
