//! Repo tasks. Usage: cargo run -p xtask -- <task>
//!
//! Tasks: validate-gltf (M5), bench (M9), goldens (M5).

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "" => eprintln!("usage: cargo run -p xtask -- <validate-gltf|bench|goldens>"),
        other => {
            eprintln!("xtask: '{other}' lands in a later milestone");
            std::process::exit(1);
        }
    }
}
