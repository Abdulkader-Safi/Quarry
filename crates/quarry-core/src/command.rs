//! The one door to the document: `CommandRegistry::execute`. Every command runs on
//! a working copy and either commits as one history entry or changes nothing.

use crate::history::{History, Source, State};
use crate::model::Document;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidArgs,
    NotFound,
    NothingSelected,
    Conflict,
    OutOfBounds,
    Forbidden,
    Io,
    Cancelled,
    Internal,
}

/// What went wrong and what to do about it, in plain words.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgs, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Io, message)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

pub type CommandResult = Result<Value, CommandError>;

/// A typed user-facing action. Its schema becomes the MCP tool's input.
pub trait Command: DeserializeOwned + JsonSchema + 'static {
    /// `area.verb`; the MCP tool is `area_verb`.
    const NAME: &'static str;
    const DESCRIPTION: &'static str;
    /// Long commands report progress and honour cancel.
    const LONG: bool = false;
    /// False for commands whose changes are not edits (save sets the path).
    const UNDOABLE: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult;
}

/// Reads and writes `.quarry` files. Lives in `quarry-io`, installed at startup.
pub trait Storage {
    fn open(&self, path: &Path) -> Result<Document, CommandError>;
    fn save(&self, doc: &Document, path: &Path) -> Result<(), CommandError>;
    /// Load an autosave and the project file it belongs to (None if never saved).
    fn recover(&self, autosave: &Path) -> Result<(Document, Option<PathBuf>), CommandError>;
}

/// What a command can see and change. Edits go to `state`, a working copy.
pub struct Context<'a> {
    pub state: &'a mut State,
    pub source: Source,
    pub history: &'a History,
    pub storage: Option<&'a dyn Storage>,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(f32),
    request: Option<Request>,
    saved: bool,
}

enum Request {
    Undo,
    Redo,
}

impl Context<'_> {
    pub fn doc(&mut self) -> &mut Document {
        &mut self.state.doc
    }

    /// Long commands call this between steps; it reports 0..1 progress and
    /// returns `cancelled` once a cancel arrives.
    pub fn step(&mut self, progress: f32) -> Result<(), CommandError> {
        (self.progress)(progress.clamp(0.0, 1.0));
        if self.cancel.load(Ordering::Relaxed) {
            Err(CommandError::new(
                ErrorCode::Cancelled,
                "Cancelled. Nothing was changed.",
            ))
        } else {
            Ok(())
        }
    }

    pub fn storage(&self) -> Result<&dyn Storage, CommandError> {
        self.storage
            .ok_or_else(|| CommandError::new(ErrorCode::Internal, "No file storage is set up."))
    }

    pub(crate) fn request_undo(&mut self) {
        self.request = Some(Request::Undo);
    }
    pub(crate) fn request_redo(&mut self) {
        self.request = Some(Request::Redo);
    }
    /// The document now matches the file at `state.path`.
    pub fn mark_saved(&mut self) {
        self.saved = true;
    }
}

type Runner = fn(Value, &mut Context) -> CommandResult;

#[derive(Clone, Debug, Serialize)]
pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub long: bool,
    pub schema: Value,
}

struct Entry {
    info: CommandInfo,
    undoable: bool,
    run: Runner,
}

/// Owns the document and its history. The only place the document changes.
pub struct CommandRegistry {
    commands: BTreeMap<&'static str, Entry>,
    state: State,
    history: History,
    storage: Option<Box<dyn Storage>>,
    cancel: Arc<AtomicBool>,
    next_revision: u64,
    saved_revision: u64,
}

impl CommandRegistry {
    /// An empty untitled project with every core command registered.
    pub fn new() -> Self {
        let mut registry = Self {
            commands: BTreeMap::new(),
            state: State {
                doc: Document::new("Untitled"),
                path: None,
                revision: 0,
            },
            history: History::default(),
            storage: None,
            cancel: Arc::default(),
            next_revision: 1,
            saved_revision: 0,
        };
        crate::commands::register_all(&mut registry);
        registry
    }

    pub fn register<C: Command>(&mut self) {
        let schema = serde_json::to_value(schemars::schema_for!(C)).expect("schemas serialise");
        let run: Runner = |args, cx| {
            let command: C = serde_json::from_value(args).map_err(|e| {
                CommandError::invalid(format!(
                    "Invalid arguments for {}: {e}. Check the command's schema.",
                    C::NAME
                ))
            })?;
            command.run(cx)
        };
        let previous = self.commands.insert(
            C::NAME,
            Entry {
                info: CommandInfo {
                    name: C::NAME,
                    description: C::DESCRIPTION,
                    long: C::LONG,
                    schema,
                },
                undoable: C::UNDOABLE,
                run,
            },
        );
        assert!(previous.is_none(), "{} registered twice", C::NAME);
    }

    pub fn set_storage(&mut self, storage: Box<dyn Storage>) {
        self.storage = Some(storage);
    }

    pub fn commands(&self) -> impl Iterator<Item = &CommandInfo> {
        self.commands.values().map(|e| &e.info)
    }

    pub fn document(&self) -> &Document {
        &self.state.doc
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    /// Unsaved changes since the last save or open.
    pub fn is_dirty(&self) -> bool {
        self.state.revision != self.saved_revision
    }

    /// Set from any thread to stop the running long command.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn execute(&mut self, name: &str, args: Value, source: Source) -> CommandResult {
        self.execute_with_progress(name, args, source, &mut |_| {})
    }

    /// Run one command. On error or panic nothing changes; on success the change
    /// becomes one history entry labelled with its source.
    pub fn execute_with_progress(
        &mut self,
        name: &str,
        args: Value,
        source: Source,
        progress: &mut dyn FnMut(f32),
    ) -> CommandResult {
        let entry = self.commands.get(name).ok_or_else(|| {
            CommandError::not_found(format!(
                "There is no command called '{name}'. List commands to see what exists."
            ))
        })?;
        let (run, undoable) = (entry.run, entry.undoable);
        self.cancel.store(false, Ordering::Relaxed);

        let mut working = self.state.clone();
        let mut cx = Context {
            state: &mut working,
            source,
            history: &self.history,
            storage: self.storage.as_deref(),
            cancel: &self.cancel,
            progress,
            request: None,
            saved: false,
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| run(args, &mut cx)));
        let (request, saved) = (cx.request.take(), cx.saved);
        let result = match outcome {
            Ok(result) => result?,
            Err(_) => {
                return Err(CommandError::new(
                    ErrorCode::Internal,
                    format!(
                        "{name} failed inside Quarry. Nothing was changed; the log has details."
                    ),
                ));
            }
        };

        match request {
            Some(Request::Undo) => {
                self.history.undo(&mut self.state);
            }
            Some(Request::Redo) => {
                self.history.redo(&mut self.state);
            }
            None if undoable && !same_state(&working, &self.state) => {
                working.revision = self.next_revision;
                self.next_revision += 1;
                let before = std::mem::replace(&mut self.state, working);
                let verb = name.split_once('.').map_or(name, |(_, verb)| verb);
                let label = verb.replace('_', " ");
                self.history.push(label, source, before, &self.state);
            }
            None => self.state = working,
        }
        if saved {
            self.saved_revision = self.state.revision;
        }
        Ok(result)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn same_state(a: &State, b: &State) -> bool {
    let (x, y) = (&a.doc, &b.doc);
    a.path == b.path
        && x.objects.shares_all(&y.objects)
        && x.images.shares_all(&y.images)
        && x.skeletons.shares_all(&y.skeletons)
        && x.clips.shares_all(&y.clips)
        && x.format_version == y.format_version
        && x.name == y.name
        && x.settings == y.settings
        && x.palettes == y.palettes
        && x.export_presets == y.export_presets
}
