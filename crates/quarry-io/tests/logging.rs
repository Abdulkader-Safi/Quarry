//! Logging writes the day's file and keeps a tail for the console.

#[test]
fn log_lines_reach_the_file_and_the_console() {
    let dir = std::env::temp_dir().join(format!("quarry-logging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let dirs = quarry_io::settings::Dirs::in_one(dir);
    let (console, guard) = quarry_io::logging::init(&dirs).unwrap();
    tracing::info!(command = "project.save", "saved");
    drop(guard); // flush the file writer

    assert!(
        console
            .lines()
            .iter()
            .any(|l| l.contains("saved") && l.contains("project.save"))
    );
    let files: Vec<_> = std::fs::read_dir(dirs.logs())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    assert!(
        std::fs::read_to_string(&files[0])
            .unwrap()
            .contains("saved")
    );
}
