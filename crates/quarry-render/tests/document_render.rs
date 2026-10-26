//! Renders a textured box offscreen. Skips where no GPU adapter exists.

use quarry_core::id::Id;
use quarry_core::model::DisplayToggles as Display;
use quarry_core::model::{Filter, Image, ImageRole};
use quarry_core::{CommandRegistry, Source};
use quarry_render::camera::{OrbitCamera, Preset};
use quarry_render::document::DocumentRenderer;
use quarry_render::snapshot::snapshot;
use serde_json::json;
use std::sync::Arc;

#[test]
fn textured_box_renders_with_nearest_filtering() {
    let renderer = match DocumentRenderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let mut r = CommandRegistry::new();
    r.execute(
        "mesh.add_primitive",
        json!({ "kind": "box" }),
        Source::Human,
    )
    .unwrap();
    // Test fixture: give every face a red 16x16 image, as tile.place will in M2.
    let mut doc = r.document().clone();
    let image = doc.images.insert(Image {
        id: Id::new(0, 0),
        name: "red".into(),
        role: ImageRole::Texture,
        size: [16, 16],
        tile_px: 16,
        padding: 0,
        zip_path: String::new(),
        source_path: None,
        source_hash: 0,
        filter: Filter::Nearest,
        pixels: Arc::from([255u8, 0, 0, 255].repeat(256)),
    });
    let id = doc.objects.ids().next().unwrap();
    let object = doc.objects.get_mut(id).unwrap();
    for face in &mut Arc::make_mut(object.mesh.as_mut().unwrap()).faces {
        face.image = Some(image);
    }

    let snap = snapshot(&doc);
    let mut camera = OrbitCamera::preset(Preset::Front);
    let (lo, hi) = snap.bounds.unwrap();
    camera.frame(lo, hi);
    let pixels = renderer.render(&snap, &camera, Display::default(), 64, 64);
    let at = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
    let centre = at(32, 32);
    assert!(
        centre[0] > 150 && centre[1] < 40 && centre[2] < 40,
        "centre is the red face: {centre:?}"
    );
    let corner = at(0, 0);
    assert!(corner[0] < 60, "corner is background: {corner:?}");

    if let Some(dir) = std::env::var_os("QUARRY_RENDER_OUT") {
        let mut iso = OrbitCamera::preset(Preset::Iso);
        iso.frame(lo, hi);
        let png = renderer
            .render_png(&snap, &iso, Display::default(), 256, 256)
            .unwrap();
        std::fs::write(std::path::Path::new(&dir).join("box-iso.png"), png).unwrap();
    }
}
