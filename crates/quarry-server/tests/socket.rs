//! M1 command server over a real local socket.

use quarry_core::{Command, CommandRegistry, CommandResult, Context};
use quarry_io::project::{QuarryFiles, read_project};
use quarry_io::settings::Dirs;
use quarry_server::headless::{self, Options};
use quarry_server::{Client, Server, SocketAddr};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qs-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn addr(test: &str) -> SocketAddr {
    if cfg!(windows) {
        SocketAddr(format!("quarry-test-{test}-{}", std::process::id()))
    } else {
        SocketAddr(folder(test).join("q.sock").to_string_lossy().into_owned())
    }
}

/// A long command that edits the document, then works in steps until cancelled.
#[derive(serde::Deserialize, schemars::JsonSchema)]
struct Slow {}

impl Command for Slow {
    const NAME: &'static str = "test.slow";
    const DESCRIPTION: &'static str = "Edits, then takes a while.";
    const LONG: bool = true;
    fn run(self, cx: &mut Context) -> CommandResult {
        cx.doc().name = "half-baked".into();
        for i in 0..500 {
            cx.step(i as f32 / 500.0)?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(json!({}))
    }
}

/// Owner thread: serves requests until `stop`, then hands the registry back.
fn owner(
    addr: SocketAddr,
    roots: Vec<PathBuf>,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<CommandRegistry> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut registry = CommandRegistry::new();
        registry.register::<Slow>();
        registry.set_storage(Box::new(QuarryFiles::default()));
        let server = Server::start(addr, registry.cancel_flag()).unwrap();
        ready_tx.send(()).unwrap();
        while !stop.load(Ordering::SeqCst) {
            if let Some(incoming) = server.recv_timeout(Duration::from_millis(20)) {
                let _ = server.handle(incoming, &mut registry, &roots);
            }
        }
        registry
    });
    ready_rx.recv().unwrap();
    handle
}

fn ok(reply: Value) -> Value {
    assert_eq!(reply["ok"], true, "{reply}");
    reply["result"].clone()
}

#[test]
fn client_creates_saves_and_queries() {
    let dir = folder("roundtrip");
    let addr = addr("roundtrip");
    let stop = Arc::new(AtomicBool::new(false));
    let handle = owner(addr.clone(), vec![dir.clone()], stop.clone());

    let mut c = Client::connect(&addr).unwrap();
    let file = dir.join("room.quarry");
    ok(c.call(
        "project.new",
        json!({ "name": "Room", "path": file }),
        &mut |_| {},
    )
    .unwrap());
    ok(
        c.call("mesh.add_primitive", json!({ "kind": "box" }), &mut |_| {})
            .unwrap(),
    );
    let mut progress = Vec::new();
    ok(c.call("project.save", json!({}), &mut |p| progress.push(p))
        .unwrap());
    assert_eq!(
        progress.first(),
        Some(&0.0),
        "long commands stream progress"
    );
    let scene = ok(c.call("query.scene", json!({}), &mut |_| {}).unwrap());
    assert_eq!(scene["objects"][0]["name"], "Box");

    let bad = c
        .call(
            "scene.rename",
            json!({ "object": "nope", "name": "x" }),
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["error"]["code"], "not_found");

    let outside = folder("roundtrip-outside").join("x.quarry");
    let refused = c
        .call("project.save_as", json!({ "path": outside }), &mut |_| {})
        .unwrap();
    assert_eq!(refused["error"]["code"], "forbidden", "{refused}");

    stop.store(true, Ordering::SeqCst);
    let registry = handle.join().unwrap();
    let last = registry.history().entries().last().unwrap();
    assert_eq!(last.display_label(), "Agent: add primitive");
    assert_eq!(read_project(&file).unwrap().objects.len(), 1);
}

#[test]
fn cancel_leaves_the_document_unchanged() {
    let addr = addr("cancel");
    let stop = Arc::new(AtomicBool::new(false));
    let handle = owner(addr.clone(), vec![], stop.clone());

    let slow = std::thread::spawn({
        let addr = addr.clone();
        move || {
            let mut c = Client::connect(&addr).unwrap();
            c.call("test.slow", json!({}), &mut |_| {}).unwrap()
        }
    });
    let mut other = Client::connect(&addr).unwrap();
    let cancelled = loop {
        std::thread::sleep(Duration::from_millis(50));
        let reply = ok(other
            .call("control.cancel", json!({}), &mut |_| {})
            .unwrap());
        if reply["cancelled"] == true {
            break reply;
        }
    };
    assert_eq!(cancelled["cancelled"], true);
    let reply = slow.join().unwrap();
    assert_eq!(reply["error"]["code"], "cancelled", "{reply}");

    stop.store(true, Ordering::SeqCst);
    let registry = handle.join().unwrap();
    assert_eq!(registry.document().name, "Untitled");
    assert!(!registry.history().can_undo());
}

#[test]
fn headless_saves_with_no_window_and_exits_when_the_client_leaves() {
    let dir = folder("headless");
    let addr = addr("headless");
    let options = Options {
        socket: Some(addr.clone()),
        dir: dir.clone(),
        commands: None,
        dirs: Dirs::in_one(dir.join("data")),
    };
    let runner = std::thread::spawn(move || headless::run(options));

    let file = dir.join("batch.quarry");
    let mut c = (0..100)
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(20));
            Client::connect(&addr).ok()
        })
        .expect("headless Quarry is listening");
    ok(c.call("project.new", json!({ "path": file }), &mut |_| {})
        .unwrap());
    ok(
        c.call("mesh.add_primitive", json!({ "kind": "box" }), &mut |_| {})
            .unwrap(),
    );
    ok(c.call("project.save", json!({}), &mut |_| {}).unwrap());
    drop(c);

    runner.join().unwrap().unwrap();
    assert_eq!(read_project(&file).unwrap().objects.len(), 1);
}
