//! FR-EXP-07: a turntable sprite sheet, rendered for real.

use quarry_core::{CommandRegistry, Source};
use quarry_io::project::QuarryFiles;
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

#[test]
fn a_turntable_is_eight_frames_in_a_row_starting_from_the_preset() {
    let dir = std::env::temp_dir().join(format!("quarry-sprite-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles {
        dirs: None,
        thumbnail: None,
        screenshot: Some(quarry_render::screenshot_png),
    }));
    run(&mut r, "mesh.add_primitive", json!({ "kind": "wedge" }));
    let reply = match r.execute(
        "export.sprite_sheet",
        json!({ "path": dir.join("spin"), "size": 64 }),
        Source::Human,
    ) {
        Ok(v) => v,
        // CI runners without a GPU adapter cannot render; anything else fails.
        Err(e) if e.message.contains("could not be rendered") => {
            eprintln!("no GPU adapter here; skipping: {}", e.message);
            return;
        }
        Err(e) => panic!("{}", e.message),
    };
    assert_eq!(
        (reply["width"].as_u64(), reply["height"].as_u64()),
        (Some(512), Some(64))
    );
    let sheet = image::open(dir.join("spin.png")).unwrap().to_rgba8();
    let frame = |i: u32| image::imageops::crop_imm(&sheet, i * 64, 0, 64, 64).to_image();
    assert_ne!(frame(0), frame(2), "the camera turns between frames");
    run(
        &mut r,
        "export.sprite_sheet",
        json!({ "path": dir.join("one"), "size": 64, "frames": 1 }),
    );
    let one = image::open(dir.join("one.png")).unwrap().to_rgba8();
    assert_eq!(frame(0), one, "frame 0 is the preset view");
    let _ = std::fs::remove_dir_all(&dir);
}
