//! Repo tasks. Usage: cargo run -p xtask -- <task>
//!
//! Tasks: validate-gltf, psd-check <file.psd> <flattened.png>, agent-session, samples,
//! bench (M9), goldens (M5).

mod samples;

use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "validate-gltf" => validate_gltf(),
        "agent-session" => agent_session(),
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
            "usage: cargo run -p xtask -- <validate-gltf|psd-check|agent-session|samples|bench|goldens>"
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
