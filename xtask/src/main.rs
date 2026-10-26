//! Repo tasks. Usage: cargo run -p xtask -- <task>
//!
//! Tasks: validate-gltf, bench (M9), goldens (M5).

use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "validate-gltf" => validate_gltf(),
        "" => eprintln!("usage: cargo run -p xtask -- <validate-gltf|bench|goldens>"),
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
