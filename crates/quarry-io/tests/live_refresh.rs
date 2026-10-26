//! M2 live refresh: an editor-style save reaches art.reload in under a second.

use quarry_core::{CommandRegistry, Source};
use quarry_io::project::QuarryFiles;
use quarry_io::watcher::ArtWatcher;
use serde_json::json;
use std::path::Path;
use std::time::{Duration, Instant};

fn save_png(path: &Path, rgba: [u8; 4]) {
    // Like Krita or Affinity: write beside, then rename over the old file.
    let temp = path.with_extension("tmp.png");
    image::RgbaImage::from_pixel(32, 32, image::Rgba(rgba))
        .save(&temp)
        .unwrap();
    std::fs::rename(&temp, path).unwrap();
}

/// Poll like the app's frame loop until the watcher reports something.
fn wait_for_change(watcher: &ArtWatcher, r: &CommandRegistry) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        if !watcher.changed(r.document()).is_empty() {
            return Some(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn saving_a_linked_png_refreshes_it_under_a_second() {
    let dir = std::env::temp_dir().join(format!("quarry-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("crate.png");
    save_png(&png, [255, 0, 0, 255]);

    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles::default()));
    r.execute(
        "art.import",
        json!({ "path": png, "as": "texture" }),
        Source::Human,
    )
    .unwrap();
    let mut watcher = ArtWatcher::new().unwrap();
    watcher.sync(r.document());
    std::thread::sleep(Duration::from_millis(200)); // let the OS watch settle

    let saved = Instant::now();
    save_png(&png, [0, 0, 255, 255]);
    let seen = wait_for_change(&watcher, &r).expect("the watcher saw the save");
    let id = r.document().images.ids().next().unwrap().to_string();
    r.execute("art.reload", json!({ "image": id }), Source::Human)
        .unwrap();
    let total = saved.elapsed();
    let image = r.document().images.iter().next().unwrap();
    assert_eq!(&image.pixels[..4], &[0, 0, 255, 255], "new pixels are in");
    assert!(
        total < Duration::from_secs(1),
        "refresh took {total:?} (watcher {seen:?})"
    );
    eprintln!("live refresh: watcher {seen:?}, total {total:?}");
    let entries = r.history().entries().len();

    // A project save in the same folder is not a reload.
    r.execute(
        "project.save_as",
        json!({ "path": dir.join("p.quarry") }),
        Source::Human,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        watcher.changed(r.document()).is_empty(),
        "saving the project does not trigger a refresh"
    );

    // Undo puts the old pixels back.
    r.execute("history.undo", json!({}), Source::Human).unwrap();
    let image = r.document().images.iter().next().unwrap();
    assert_eq!(&image.pixels[..4], &[255, 0, 0, 255]);
    assert_eq!(r.history().entries().len(), entries - 1);
}
