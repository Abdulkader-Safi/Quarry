//! M1 autosave, backups and recovery.

use quarry_core::{CommandRegistry, Source};
use quarry_io::autosave::{Autosaver, BACKUPS_KEPT, recoverable, write_autosave};
use quarry_io::project::{QuarryFiles, read_project};
use quarry_io::settings::Dirs;
use serde_json::json;
use std::time::{Duration, Instant};

fn setup(test: &str) -> (Dirs, std::path::PathBuf, CommandRegistry) {
    let root = std::env::temp_dir().join(format!("quarry-autosave-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dirs = Dirs::in_one(root.join("data"));
    let projects = root.join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles {
        dirs: Some(dirs.clone()),
    }));
    (dirs, projects, r)
}

#[test]
fn crash_keeps_the_original_and_recovery_restores_the_edit() {
    let (dirs, projects, mut r) = setup("crash");
    let file = projects.join("room.quarry");
    r.execute("project.save_as", json!({ "path": file }), Source::Human)
        .unwrap();
    r.execute(
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Crate" }),
        Source::Human,
    )
    .unwrap();

    // The 2-minute timer fires: a clone goes to a worker and lands as a side copy.
    let snapshot = r.document().clone();
    std::thread::spawn({
        let (dirs, file) = (dirs.clone(), file.clone());
        move || write_autosave(&dirs, &snapshot, Some(&file)).unwrap()
    })
    .join()
    .unwrap();
    drop(r); // the crash: nothing else is written

    assert!(
        read_project(&file).unwrap().objects.is_empty(),
        "the user's file is untouched"
    );
    let offers = recoverable(&dirs);
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].sidecar.project.as_deref(), Some(file.as_path()));

    let mut relaunched = relaunch(&dirs);
    relaunched
        .execute(
            "project.recover",
            json!({ "autosave": offers[0].autosave }),
            Source::Human,
        )
        .unwrap();
    assert!(relaunched.document().objects.by_name("Crate").is_some());
    assert_eq!(relaunched.state().path.as_deref(), Some(file.as_path()));
    assert!(
        relaunched.is_dirty(),
        "recovered work is not yet in the user's file"
    );

    relaunched
        .execute("project.save", json!({}), Source::Human)
        .unwrap();
    assert!(recoverable(&dirs).is_empty(), "saving clears the side copy");
    assert!(
        read_project(&file)
            .unwrap()
            .objects
            .by_name("Crate")
            .is_some()
    );
}

/// A fresh registry over the same data folder.
fn relaunch(dirs: &Dirs) -> CommandRegistry {
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles {
        dirs: Some(dirs.clone()),
    }));
    r
}

#[test]
fn untitled_work_is_recoverable_too() {
    let (dirs, _, mut r) = setup("untitled");
    r.execute(
        "mesh.add_primitive",
        json!({ "kind": "box" }),
        Source::Agent,
    )
    .unwrap();
    write_autosave(&dirs, r.document(), None).unwrap();
    let offers = recoverable(&dirs);
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].sidecar.project, None);
    assert_eq!(offers[0].sidecar.name, "Untitled");
}

#[test]
fn eleven_saves_keep_ten_backups() {
    let (dirs, projects, mut r) = setup("backups");
    let file = projects.join("level.quarry");
    r.execute("project.save_as", json!({ "path": file }), Source::Human)
        .unwrap();
    for i in 0..11 {
        r.execute(
            "project.set_settings",
            json!({ "fps": 10 + i }),
            Source::Human,
        )
        .unwrap();
        r.execute("project.save", json!({}), Source::Human).unwrap();
    }
    let folder = std::fs::read_dir(dirs.backups())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), BACKUPS_KEPT);
}

#[test]
fn autosave_runs_on_the_interval_and_only_after_changes() {
    let start = Instant::now();
    let mut timer = Autosaver::new(2, 0);
    assert!(!timer.due(start + Duration::from_secs(60), 1), "too soon");
    assert!(
        !timer.due(start + Duration::from_secs(121), 0),
        "nothing changed"
    );
    assert!(timer.due(start + Duration::from_secs(121), 1));
    assert!(
        !timer.due(start + Duration::from_secs(122), 2),
        "just autosaved"
    );
    assert!(timer.due(start + Duration::from_secs(250), 2));
}
