//! A dropped texture goes on the edited mesh when asked (FR-ART-06).

use quarry_core::{CommandRegistry, Source};
use quarry_io::project::QuarryFiles;
use serde_json::json;

#[test]
fn an_imported_texture_can_go_on_the_edited_mesh() {
    let dir = std::env::temp_dir().join(format!("quarry-assign-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("crate.png");
    image::RgbaImage::from_pixel(32, 16, image::Rgba([200, 90, 40, 255]))
        .save(&png)
        .unwrap();

    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles::default()));
    let made = r
        .execute(
            "mesh.add_primitive",
            json!({ "kind": "box" }),
            Source::Human,
        )
        .unwrap();
    let id = made["object"]
        .as_str()
        .or(made["id"].as_str())
        .unwrap()
        .to_string();
    r.execute("select.by_ids", json!({ "objects": [id] }), Source::Human)
        .unwrap();
    // Without assign the texture is only added.
    let plain = r
        .execute(
            "art.import",
            json!({ "path": png, "as": "texture" }),
            Source::Human,
        )
        .unwrap();
    assert_eq!(plain["faces"], 0);
    let put = r
        .execute(
            "art.import",
            json!({ "path": png, "as": "texture", "assign": true }),
            Source::Human,
        )
        .unwrap();
    assert_eq!(put["faces"], 6);
    let doc = r.document();
    let object = doc.objects.iter().next().unwrap();
    let id = doc.images.iter().nth(1).unwrap().id;
    assert!(
        object
            .mesh
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .all(|f| f.image == Some(id))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
