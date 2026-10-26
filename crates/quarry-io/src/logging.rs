//! Logging: `tracing` to a daily file kept 7 days, plus an in-memory tail for
//! the in-app console. `QUARRY_LOG` sets the level (default `info`).

use crate::settings::Dirs;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

const CONSOLE_LINES: usize = 1000;

/// The newest log lines, for the console panel.
#[derive(Clone, Default)]
pub struct Console(Arc<Mutex<VecDeque<String>>>);

impl Console {
    pub fn lines(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl Write for Console {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut lines) = self.0.lock() {
            for line in String::from_utf8_lossy(buf)
                .lines()
                .filter(|l| !l.is_empty())
            {
                if lines.len() == CONSOLE_LINES {
                    lines.pop_front();
                }
                lines.push_back(line.to_string());
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> fmt::MakeWriter<'a> for Console {
    type Writer = Console;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Start logging. Keep the guard alive for the whole run so the file is flushed.
pub fn init(dirs: &Dirs) -> Result<(Console, WorkerGuard), String> {
    let file = Builder::new()
        .rotation(Rotation::DAILY)
        .max_log_files(7)
        .filename_prefix("quarry")
        .filename_suffix("log")
        .build(dirs.logs())
        .map_err(|e| {
            format!(
                "Could not start the log file in {}: {e}.",
                dirs.logs().display()
            )
        })?;
    let (file, guard) = tracing_appender::non_blocking(file);
    let console = Console::default();
    let filter = EnvFilter::try_from_env("QUARRY_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(file).with_ansi(false))
        .with(
            fmt::layer()
                .with_writer(console.clone())
                .with_ansi(false)
                .without_time(),
        )
        .try_init()
        .map_err(|e| format!("Logging was already started: {e}."))?;
    Ok((console, guard))
}
