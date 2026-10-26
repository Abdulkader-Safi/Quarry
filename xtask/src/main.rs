//! Repo tasks. Usage: cargo run -p xtask -- <task>
//!
//! Tasks: validate-gltf, psd-check <file.psd> <flattened.png>, agent-session, samples, no-network,
//! bench, goldens (QUARRY_UPDATE_GOLDENS=1 cargo test).

mod samples;

use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "validate-gltf" => validate_gltf(),
        "agent-session" => agent_session(),
        "no-network" => no_network(),
        "bench" => bench(),
        "samples" => samples::build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..")),
        "psd-check" => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let [psd, png] = args.as_slice() else {
                eprintln!("usage: cargo run -p xtask -- psd-check <file.psd> <flattened.png>");
                exit(1);
            };
            psd_check(Path::new(psd), Path::new(png));
        }
        "" => eprintln!(
            "usage: cargo run -p xtask -- <validate-gltf|psd-check|agent-session|samples|no-network|bench|goldens>"
        ),
        other => {
            eprintln!("xtask: '{other}' lands in a later milestone");
            exit(1);
        }
    }
}

/// Run the Khronos glTF Validator (npm, dev and CI only) over every golden.
fn validate_gltf() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let validator = root.join("xtask/validator");
    if !validator.join("node_modules").exists() {
        run(Command::new("npm").arg("ci").current_dir(&validator));
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("tests/goldens"))
        .expect("tests/goldens exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "glb" || x == "gltf"))
        .collect();
    files.sort();
    run(Command::new("node")
        .arg(validator.join("validate.mjs"))
        .args(&files));
}

/// Build Quarry and quarry-mcp, then run a scripted agent session that
/// models, paints, rigs, animates, screenshots and exports a crate.
fn agent_session() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    run(Command::new(env!("CARGO"))
        .args(["build", "-p", "quarry", "-p", "quarry-mcp"])
        .current_dir(&root));
    let work = std::env::temp_dir().join(format!("quarry-agent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("temp folder");
    let exe = root.join("target/debug").join(if cfg!(windows) {
        "quarry-mcp.exe"
    } else {
        "quarry-mcp"
    });
    run(Command::new("python3")
        .arg(root.join("xtask/agent/agent_session.py"))
        .arg(exe)
        .arg(&work));
    println!("output in {}", work.display());
}

/// NFR-03: no network. Fails if an HTTP client, TLS stack or WebSocket crate
/// is in a shipped binary, or if our code opens a TCP or UDP socket. (GPUI
/// brings the `http` types crate for its client trait; no client is linked.)
fn no_network() {
    const CLIENTS: &[&str] = &[
        "reqwest",
        "ureq",
        "hyper",
        "curl",
        "isahc",
        "surf",
        "attohttpc",
        "tungstenite",
        "async-tungstenite",
        "native-tls",
        "rustls",
        "openssl",
        "h2",
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut failed = false;
    for bin in ["quarry", "quarry-mcp"] {
        let out = Command::new(env!("CARGO"))
            .args(["tree", "-p", bin, "-e", "normal", "--prefix", "none"])
            .current_dir(&root)
            .output()
            .expect("cargo tree runs");
        let tree = String::from_utf8_lossy(&out.stdout);
        let found: std::collections::BTreeSet<&str> = tree
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .filter(|name| CLIENTS.contains(name))
            .collect();
        if !found.is_empty() {
            eprintln!("{bin} links network crates: {found:?}");
            failed = true;
        }
    }
    for entry in walk(&root.join("crates")) {
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        for needle in [
            "TcpListener",
            "TcpStream",
            "UdpSocket",
            "std::net",
            "ToSocketAddrs",
        ] {
            if text.contains(needle) {
                eprintln!("{} uses {needle}", entry.display());
                failed = true;
            }
        }
    }
    if failed {
        exit(1);
    }
    println!(
        "no network: no HTTP, TLS or WebSocket crates in quarry or quarry-mcp, and no sockets in our code"
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// NFR-01: open, save and export of a 100k-triangle project, each under 2 s.
/// Builds the scene through the registry (450 spheres of 224 triangles, a
/// 16-colour textured floor), then times each step. Use a release build:
/// `cargo run --release -p xtask -- bench`.
fn bench() {
    use quarry_core::{CommandRegistry, Source};
    use serde_json::json;
    use std::time::Instant;
    let dir = std::env::temp_dir().join(format!("quarry-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("bench folder");
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(quarry_io::project::QuarryFiles::default()));
    let timed = |r: &mut CommandRegistry, name: &str, args: serde_json::Value| {
        let started = Instant::now();
        r.execute(name, args, Source::Human)
            .unwrap_or_else(|e| panic!("{name}: {}", e.message));
        started.elapsed()
    };
    let started = Instant::now();
    for i in 0..450 {
        let (x, z) = ((i % 30) as f32 * 1.2, (i / 30) as f32 * 1.2);
        timed(
            &mut r,
            "mesh.add_primitive",
            json!({ "kind": "sphere", "segments": 16, "position": [x, 0.5, z] }),
        );
    }
    let build = started.elapsed();
    let triangles = r
        .document()
        .objects
        .iter()
        .filter_map(|o| o.mesh.as_deref())
        .map(|m| m.triangle_count())
        .sum::<usize>();
    let file = dir.join("bench.quarry");
    let save = timed(&mut r, "project.save_as", json!({ "path": file }));
    let open = timed(&mut r, "project.open", json!({ "path": file }));
    let export = timed(
        &mut r,
        "export.run",
        json!({ "path": dir.join("bench.glb") }),
    );
    let size = std::fs::metadata(&file).map_or(0, |m| m.len());
    let glb = std::fs::metadata(dir.join("bench.glb")).map_or(0, |m| m.len());
    println!("triangles  {triangles}");
    println!("build      {build:?} (450 add_primitive commands)");
    println!("save       {save:?}  ({size} bytes)   target < 2 s");
    println!("open       {open:?}   target < 2 s");
    println!("export     {export:?}  ({glb} bytes)  target < 2 s");
    let _ = std::fs::remove_dir_all(&dir);
}

fn run(command: &mut Command) {
    let status = command.status().unwrap_or_else(|e| {
        eprintln!(
            "could not start {:?}: {e}. Is Node.js installed?",
            command.get_program()
        );
        exit(1);
    });
    if !status.success() {
        exit(status.code().unwrap_or(1));
    }
}

/// M0 PSD spike: compare a PSD's composite with the editor's flattened PNG export.
fn psd_check(psd: &Path, png: &Path) {
    let bytes = std::fs::read(psd).unwrap_or_else(|e| {
        eprintln!("could not read {}: {e}", psd.display());
        exit(1);
    });
    let composite = quarry_io::psd::read_psd(&bytes).unwrap_or_else(|e| {
        eprintln!("{}: {e}", psd.display());
        exit(1);
    });
    let flat = image::open(png)
        .unwrap_or_else(|e| {
            eprintln!("could not read {}: {e}", png.display());
            exit(1);
        })
        .to_rgba8();
    if composite.dimensions() != flat.dimensions() {
        eprintln!(
            "size differs: PSD {:?}, PNG {:?}",
            composite.dimensions(),
            flat.dimensions()
        );
        exit(1);
    }
    let pixels = composite.pixels().zip(flat.pixels());
    let (mut differ, mut worst) = (0usize, 0u8);
    for (a, b) in pixels {
        let channel =
            a.0.iter()
                .zip(b.0)
                .map(|(x, y)| x.abs_diff(y))
                .max()
                .unwrap();
        if channel > 0 {
            differ += 1;
            worst = worst.max(channel);
        }
    }
    let total = composite.width() as usize * composite.height() as usize;
    println!(
        "{}: {differ} of {total} pixels differ, worst channel error {worst}/255",
        psd.display()
    );
    if differ > 0 {
        exit(1);
    }
}
