//! One undo history for human and agent edits. Entries are whole snapshots;
//! snapshots share every value a command did not touch, so they stay small.

use crate::model::Document;
use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;

pub const MAX_ENTRIES: usize = 200;
pub const MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Human,
    Agent,
}

/// Everything undo restores: the document, where it is saved, and its revision.
#[derive(Clone, Debug)]
pub struct State {
    pub doc: Document,
    pub path: Option<PathBuf>,
    /// Unique per committed change, so "undo back to the saved state" reads as clean.
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct Entry {
    /// The command's verb, such as "extrude".
    pub label: String,
    pub source: Source,
    pub time: SystemTime,
    /// The state on the other side of this entry (before, on the undo stack).
    state: State,
    bytes: usize,
}

impl Entry {
    /// "Agent: extrude" for agent edits, "extrude" for the human's own.
    pub fn display_label(&self) -> String {
        match self.source {
            Source::Agent => format!("Agent: {}", self.label),
            Source::Human => self.label.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl History {
    /// Record a committed change. `before` is the state the command started from.
    pub fn push(&mut self, label: String, source: Source, before: State, after: &State) {
        let bytes = before.doc.bytes_not_shared_with(&after.doc);
        self.undo.push(Entry {
            label,
            source,
            time: SystemTime::now(),
            state: before,
            bytes,
        });
        self.redo.clear();
        self.trim();
    }

    /// Swap `current` with the state before the newest entry.
    pub fn undo(&mut self, current: &mut State) -> Option<&Entry> {
        let entry = self.undo.pop()?;
        self.redo.push(swap(entry, current));
        self.redo.last()
    }

    pub fn redo(&mut self, current: &mut State) -> Option<&Entry> {
        let entry = self.redo.pop()?;
        self.undo.push(swap(entry, current));
        self.undo.last()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Undo entries, oldest first.
    pub fn entries(&self) -> &[Entry] {
        &self.undo
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.undo.iter().chain(&self.redo).map(|e| e.bytes).sum()
    }

    fn trim(&mut self) {
        while self.undo.len() > MAX_ENTRIES || (self.bytes() > MAX_BYTES && self.undo.len() > 1) {
            self.undo.remove(0);
        }
    }
}

fn swap(mut entry: Entry, current: &mut State) -> Entry {
    std::mem::swap(&mut entry.state, current);
    entry.bytes = entry.state.doc.bytes_not_shared_with(&current.doc);
    entry
}
