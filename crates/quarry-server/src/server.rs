//! The command server: JSON lines over the local socket.
//!
//! In:  `{"id":1,"command":"mesh.add_primitive","args":{...}}`
//! Out: `{"id":1,"ok":true,"result":{...}}` or `{"id":1,"ok":false,"error":{"code","message"}}`,
//! plus `{"id":1,"progress":0.4}` lines while a long command runs.
//!
//! Connection threads only parse and forward. Every command runs on the thread
//! that owns the document, strictly in arrival order. `control.cancel` is
//! answered straight from the connection thread so it can stop the running command.

use crate::permissions;
use crate::socket::SocketAddr;
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use quarry_core::{CommandError, CommandRegistry, ErrorCode, Source};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// One request waiting for the document owner.
pub struct Incoming {
    pub id: Value,
    pub command: String,
    pub args: Value,
    reply: Sender<String>,
}

/// Running listener. Poll it from the thread that owns the registry.
pub struct Server {
    rx: Receiver<Incoming>,
    busy: Arc<AtomicBool>,
    clients: Arc<AtomicUsize>,
    pub addr: SocketAddr,
}

impl Server {
    /// Listen on `addr`. Fails with `AddrInUse` if another Quarry is serving it.
    pub fn start(addr: SocketAddr, cancel: Arc<AtomicBool>) -> io::Result<Self> {
        let listener = listen(&addr)?;
        let (tx, rx) = channel();
        let busy = Arc::new(AtomicBool::new(false));
        let clients = Arc::new(AtomicUsize::new(0));
        let shared = (busy.clone(), clients.clone());
        std::thread::Builder::new()
            .name("quarry-server".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    match stream {
                        Ok(stream) => {
                            let (tx, cancel) = (tx.clone(), cancel.clone());
                            let (busy, clients) = (shared.0.clone(), shared.1.clone());
                            clients.fetch_add(1, Ordering::SeqCst);
                            let _ = std::thread::Builder::new()
                                .name("quarry-client".into())
                                .spawn(move || {
                                    serve(stream, tx, cancel, busy);
                                    clients.fetch_sub(1, Ordering::SeqCst);
                                });
                        }
                        Err(e) => tracing::warn!("socket connection failed: {e}"),
                    }
                }
            })?;
        tracing::info!(socket = %addr.0, "command server listening");
        Ok(Self {
            rx,
            busy,
            clients,
            addr,
        })
    }

    pub fn try_recv(&self) -> Option<Incoming> {
        self.rx.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<Incoming> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Connected socket clients right now.
    pub fn clients(&self) -> usize {
        self.clients.load(Ordering::SeqCst)
    }

    /// Run one request against the registry as the agent, streaming progress.
    /// `roots` are the folders the agent may write into. Returns the reply, so
    /// a window can show it in its agent log.
    pub fn handle(
        &self,
        incoming: Incoming,
        registry: &mut CommandRegistry,
        roots: &[PathBuf],
    ) -> Result<Value, CommandError> {
        let Incoming {
            id,
            command,
            args,
            reply,
        } = incoming;
        self.busy.store(true, Ordering::SeqCst);
        let saving_to = registry.state().path.clone();
        let result = permissions::check_agent(&command, &args, roots, saving_to.as_deref())
            .and_then(|()| {
                let progress_id = id.clone();
                let progress_reply = reply.clone();
                registry.execute_with_progress(&command, args, Source::Agent, &mut |p| {
                    let _ = progress_reply
                        .send(json!({ "id": progress_id, "progress": p }).to_string());
                })
            });
        self.busy.store(false, Ordering::SeqCst);
        // The registry logs every command it runs; only refusals stop short of it.
        if let Err(e) = &result
            && e.code == ErrorCode::Forbidden
        {
            tracing::warn!(%command, source = "agent", "refused: {}", e.message);
        }
        let _ = reply.send(response(&id, result.clone()).to_string());
        result
    }
}

/// The reply line for a finished command.
pub fn response(id: &Value, result: Result<Value, CommandError>) -> Value {
    match result {
        Ok(result) => json!({ "id": id, "ok": true, "result": result }),
        Err(error) => json!({ "id": id, "ok": false, "error": error }),
    }
}

#[cfg(unix)]
fn listen(addr: &SocketAddr) -> io::Result<interprocess::local_socket::Listener> {
    match ListenerOptions::new().name(addr.name()?).create_sync() {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            // A live Quarry answers; a leftover file from a crash does not.
            if Stream::connect(addr.name()?).is_ok() {
                return Err(e);
            }
            std::fs::remove_file(&addr.0)?;
            ListenerOptions::new().name(addr.name()?).create_sync()
        }
        other => other,
    }
}

#[cfg(windows)]
fn listen(addr: &SocketAddr) -> io::Result<interprocess::local_socket::Listener> {
    ListenerOptions::new().name(addr.name()?).create_sync()
}

fn serve(stream: Stream, tx: Sender<Incoming>, cancel: Arc<AtomicBool>, busy: Arc<AtomicBool>) {
    let stream = Arc::new(stream);
    let (reply, outbox) = channel::<String>();
    let writer = std::thread::spawn({
        let stream = stream.clone();
        move || {
            let mut send = &*stream;
            for line in outbox {
                if send.write_all(format!("{line}\n").as_bytes()).is_err() {
                    break;
                }
            }
        }
    });

    for line in BufReader::new(&*stream).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let error = CommandError::invalid(format!(
                    "That line is not JSON ({e}). Send one {{\"id\",\"command\",\"args\"}} object per line."
                ));
                let _ = reply.send(response(&Value::Null, Err(error)).to_string());
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let Some(command) = request.get("command").and_then(Value::as_str) else {
            let error =
                CommandError::invalid("Missing \"command\". Send {\"id\",\"command\",\"args\"}.");
            let _ = reply.send(response(&id, Err(error)).to_string());
            continue;
        };
        let args = request.get("args").cloned().unwrap_or_else(|| json!({}));

        if command == "control.cancel" {
            let running = busy.load(Ordering::SeqCst);
            if running {
                cancel.store(true, Ordering::SeqCst);
            }
            let result = if running {
                json!({ "cancelled": true })
            } else {
                json!({ "cancelled": false, "reason": "Nothing is running." })
            };
            let _ = reply.send(response(&id, Ok(result)).to_string());
            continue;
        }
        let incoming = Incoming {
            id: id.clone(),
            command: command.to_string(),
            args,
            reply: reply.clone(),
        };
        if tx.send(incoming).is_err() {
            let error = CommandError::new(
                ErrorCode::Internal,
                "Quarry is shutting down. Try again once it restarts.",
            );
            let _ = reply.send(response(&id, Err(error)).to_string());
            break;
        }
    }
    drop(reply);
    let _ = writer.join();
}

/// Client side, for tests, the MCP bridge and handing work to a running Quarry.
pub struct Client {
    stream: BufReader<Stream>,
    next_id: u64,
}

impl Client {
    pub fn connect(addr: &SocketAddr) -> io::Result<Self> {
        Ok(Self {
            stream: BufReader::new(Stream::connect(addr.name()?)?),
            next_id: 1,
        })
    }

    /// Send one command and wait for its final line. Progress lines go to `progress`.
    pub fn call(
        &mut self,
        command: &str,
        args: Value,
        progress: &mut dyn FnMut(f32),
    ) -> io::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({ "id": id, "command": command, "args": args }).to_string();
        let mut send = self.stream.get_ref();
        send.write_all(format!("{line}\n").as_bytes())?;
        loop {
            let mut buf = String::new();
            if self.stream.read_line(&mut buf)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Quarry closed the connection",
                ));
            }
            let value: Value = serde_json::from_str(&buf).map_err(io::Error::other)?;
            if value["id"] != json!(id) {
                continue;
            }
            if let Some(p) = value.get("progress").and_then(Value::as_f64) {
                progress(p as f32);
                continue;
            }
            return Ok(value);
        }
    }
}
