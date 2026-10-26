//! `quarry --headless`: the same document and registry with no window.
//!
//! With `--commands file.jsonl` it runs each line in order, prints each reply and
//! exits (CI and batch jobs). Otherwise it serves the socket until the last
//! client leaves with nothing unsaved, or until `QUARRY_HEADLESS_IDLE_SECS`
//! (default 600) pass with no request, autosaving unsaved work first.

use crate::server::{Server, response};
use crate::socket::SocketAddr;
use quarry_core::{CommandRegistry, Source};
use quarry_io::autosave::{Autosaver, write_autosave};
use quarry_io::project::QuarryFiles;
use quarry_io::settings::{Dirs, Settings};
use serde_json::{Value, json};
use std::io::BufRead;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Options {
    /// Socket to serve; default `QUARRY_SOCKET` or the per-user path.
    pub socket: Option<SocketAddr>,
    /// Folder the agent may write into besides the open project's folder.
    pub dir: PathBuf,
    /// Run these JSON lines and exit instead of serving.
    pub commands: Option<PathBuf>,
    /// App data folder for autosaves, backups and logs.
    pub dirs: Dirs,
}

impl Options {
    /// Parse `--socket <path> --dir <folder> --commands <file>` after `--headless`.
    pub fn from_args(args: &[String]) -> Result<Self, String> {
        let value = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| {
                    args.get(i + 1)
                        .cloned()
                        .ok_or(format!("{flag} needs a value"))
                })
                .transpose()
        };
        Ok(Self {
            socket: value("--socket")?.map(SocketAddr),
            dir: match value("--dir")? {
                Some(dir) => PathBuf::from(dir),
                None => std::env::current_dir().map_err(|e| e.to_string())?,
            },
            commands: value("--commands")?.map(PathBuf::from),
            dirs: Dirs::from_env()?,
        })
    }
}

fn registry(dirs: &Dirs) -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    registry.set_storage(Box::new(QuarryFiles {
        dirs: Some(dirs.clone()),
        thumbnail: Some(quarry_render::thumbnail_png),
    }));
    registry
}

/// Folders the agent may write into: the runner's folder and the project's.
fn roots(options: &Options, registry: &CommandRegistry) -> Vec<PathBuf> {
    let mut roots = vec![options.dir.clone()];
    if let Some(dir) = registry.state().path.as_ref().and_then(|p| p.parent()) {
        roots.push(dir.to_path_buf());
    }
    roots
}

pub fn run(options: Options) -> Result<(), String> {
    let dirs = options.dirs.clone();
    if let Some(file) = &options.commands {
        return run_batch(&dirs, file);
    }
    let mut registry = registry(&dirs);
    let addr = match options.socket.clone() {
        Some(addr) => addr,
        None => {
            SocketAddr::from_env().map_err(|e| format!("Could not pick a socket path: {e}."))?
        }
    };
    let server = Server::start(addr.clone(), registry.cancel_flag()).map_err(|e| {
        format!(
            "Could not listen on {}: {e}. Is another Quarry already running?",
            addr.0
        )
    })?;
    println!("quarry headless listening on {}", addr.0);

    let idle = std::env::var("QUARRY_HEADLESS_IDLE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or(Duration::from_secs(600), Duration::from_secs);
    let (settings, _) = Settings::load(&dirs);
    let mut autosaver = Autosaver::new(settings.autosave_minutes, registry.state().revision);
    let mut last_request = Instant::now();
    let mut had_client = false;

    loop {
        if let Some(incoming) = server.recv_timeout(Duration::from_millis(200)) {
            let roots = roots(&options, &registry);
            server.handle(incoming, &mut registry, &roots);
            last_request = Instant::now();
        }
        let state = registry.state();
        if autosaver.due(Instant::now(), state.revision) {
            let (doc, path, dirs) = (state.doc.clone(), state.path.clone(), dirs.clone());
            std::thread::spawn(move || {
                if let Err(e) = write_autosave(&dirs, &doc, path.as_deref()) {
                    tracing::warn!("{e}");
                }
            });
        }

        had_client |= server.clients() > 0;
        let alone = had_client && server.clients() == 0;
        if alone && !registry.is_dirty() {
            println!("last client left; exiting");
            return Ok(());
        }
        if last_request.elapsed() >= idle {
            if registry.is_dirty() {
                let state = registry.state();
                write_autosave(&dirs, &state.doc, state.path.as_deref())?;
                println!("idle; unsaved work autosaved; exiting");
            }
            return Ok(());
        }
    }
}

/// One JSON request per line; one reply per line on stdout. Runs as the human
/// at the keyboard, so no agent path limits.
fn run_batch(dirs: &Dirs, file: &PathBuf) -> Result<(), String> {
    let mut registry = registry(dirs);
    let text = std::fs::File::open(file)
        .map_err(|e| format!("Could not open {}: {e}.", file.display()))?;
    let mut failed = false;
    for (n, line) in std::io::BufReader::new(text).lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line)
            .map_err(|e| format!("{} line {}: not JSON ({e}).", file.display(), n + 1))?;
        let id = request.get("id").cloned().unwrap_or(json!(n + 1));
        let command = request["command"].as_str().unwrap_or_default();
        let args = request.get("args").cloned().unwrap_or_else(|| json!({}));
        let result = registry.execute(command, args, Source::Human);
        failed |= result.is_err();
        println!("{}", response(&id, result));
    }
    if failed {
        Err("Some commands failed; see the replies above.".into())
    } else {
        Ok(())
    }
}
