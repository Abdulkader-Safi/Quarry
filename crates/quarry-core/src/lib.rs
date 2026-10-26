//! Document, IDs, CommandRegistry, History.
//!
//! Nothing outside `CommandRegistry::execute` mutates the document.

pub mod camera;
pub mod command;
pub mod commands;
pub mod history;
pub mod id;
pub mod model;

pub use command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
pub use history::Source;
pub use model::Document;
