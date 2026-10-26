//! M4 painting through the registry.

use glam::Vec2;
use quarry_core::{CommandRegistry, Source};
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

/// A unit box at the origin carrying a fresh 32x32 white texture.
fn painted_box() -> CommandRegistry {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "B" }),
    );
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "face", "object": "B" }),
    );
    run(&mut r, "paint.palette_lock", json!({ "lock": false }));
    run(
        &mut r,
        "paint.texture_new",
        json!({ "width": 32, "height": 32, "name": "skin" }),
    );
    r
}

fn pixels(r: &CommandRegistry) -> Vec<u8> {
    r.document().images.by_name("skin").unwrap().pixels.to_vec()
}

fn changed(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

#[test]
fn a_new_texture_covers_every_face() {
    let r = painted_box();
    let doc = r.document();
    let id = doc.images.by_name("skin").unwrap().id;
    let mesh = doc.objects.by_name("B").unwrap().mesh.as_deref().unwrap();
    assert!(mesh.faces.iter().all(|f| f.image == Some(id)));
    let max = mesh
        .faces
        .iter()
        .flat_map(|f| &f.corners)
        .fold(Vec2::ZERO, |m, c| m.max(c.uv));
    assert_eq!(max, Vec2::splat(32.0), "UVs span the whole 32x32 texture");
}

#[test]
fn one_pencil_pixel_changes_one_texel_and_undoes() {
    let mut r = painted_box();
    let before = pixels(&r);
    run(
        &mut r,
        "paint.pixels",
        json!({ "image": "skin", "pixels": [{ "x": 5, "y": 7, "rgba": [255, 0, 0, 255] }] }),
    );
    let after = pixels(&r);
    assert_eq!(changed(&before, &after), 1);
    assert_eq!(
        &after[(7 * 32 + 5) * 4..(7 * 32 + 5) * 4 + 4],
        &[255, 0, 0, 255]
    );
    run(&mut r, "history.undo", json!({}));
    assert_eq!(pixels(&r), before);
    let err = r
        .execute(
            "paint.pixels",
            json!({ "image": "skin", "pixels": [{ "x": 40, "y": 0, "rgba": [0, 0, 0, 255] }] }),
            Source::Agent,
        )
        .unwrap_err();
    assert_eq!(err.code, quarry_core::ErrorCode::OutOfBounds);
}

#[test]
fn palette_lock_snaps_to_the_nearest_of_sixteen() {
    let mut r = painted_box();
    let palette: Vec<[u8; 4]> = (0..16u8)
        .map(|i| [i * 16, 255 - i * 16, 128, 255])
        .collect();
    run(
        &mut r,
        "paint.palette_set",
        json!({ "name": "sixteen", "colors": palette }),
    );
    run(&mut r, "paint.palette_lock", json!({ "lock": true }));
    run(
        &mut r,
        "paint.pixels",
        json!({ "image": "skin", "pixels": [{ "x": 0, "y": 0, "rgba": [37, 214, 130, 255] }] }),
    );
    let p = &pixels(&r)[0..4];
    assert!(palette.iter().any(|c| c == p), "{p:?} is a palette colour");
    assert_eq!(p, &[32, 223, 128, 255]);
}

#[test]
fn a_stroke_across_an_edge_is_continuous_on_both_faces() {
    let mut r = painted_box();
    run(&mut r, "uv.unwrap", json!({}));
    let before = pixels(&r);
    // Across the top face, over the front edge, down the front face.
    let pts = [
        [0.0, 0.5, -0.3],
        [0.0, 0.5, 0.5],
        [0.0, 0.2, 0.5],
        [0.0, -0.3, 0.5],
    ];
    run(
        &mut r,
        "paint.stroke_3d",
        json!({ "points": pts, "rgba": [0, 0, 0, 255] }),
    );
    let after = pixels(&r);
    // Both faces got paint inside their own UV areas.
    let mesh = r
        .document()
        .objects
        .by_name("B")
        .unwrap()
        .mesh
        .clone()
        .unwrap();
    let rect = |normal: glam::Vec3| {
        let f = mesh
            .faces
            .iter()
            .find(|f| {
                let p: Vec<glam::Vec3> = f
                    .corners
                    .iter()
                    .map(|c| mesh.positions[c.vertex as usize])
                    .collect();
                (p[1] - p[0])
                    .cross(p[2] - p[0])
                    .normalize()
                    .distance(normal)
                    < 1e-4
            })
            .unwrap();
        f.corners
            .iter()
            .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), c| {
                (lo.min(c.uv), hi.max(c.uv))
            })
    };
    let painted: Vec<Vec2> = (0..32 * 32)
        .filter(|i| before[i * 4..i * 4 + 4] != after[i * 4..i * 4 + 4])
        .map(|i| Vec2::new((i % 32) as f32 + 0.5, (i / 32) as f32 + 0.5))
        .collect();
    for (name, n) in [("top", glam::Vec3::Y), ("front", glam::Vec3::Z)] {
        let (lo, hi) = rect(n);
        let hits = painted
            .iter()
            .filter(|p| p.cmpge(lo).all() && p.cmple(hi).all())
            .count();
        assert!(hits >= 5, "{hits} painted texels on the {name} face");
    }

    run(&mut r, "history.undo", json!({}));
    assert_eq!(pixels(&r), before, "undo removes the whole stroke");
}

#[test]
fn pressure_picks_whole_pixel_brush_sizes() {
    for (pressure, expected) in [(0.15, 1), (1.0, 9)] {
        let mut r = painted_box();
        let before = pixels(&r);
        run(
            &mut r,
            "paint.stroke_3d",
            json!({ "points": [[0.05, 0.5, 0.05]], "rgba": [0, 0, 0, 255], "size": 3, "pressures": [pressure] }),
        );
        assert_eq!(
            changed(&before, &pixels(&r)),
            expected,
            "pressure {pressure}"
        );
    }
}
