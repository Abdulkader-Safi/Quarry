//! M2 tiles and linked art, through the registry with real PNG files.

use glam::Vec3;
use quarry_core::model::{Document, Mesh};
use quarry_core::{CommandRegistry, ErrorCode, Source};
use quarry_io::project::{QuarryFiles, read_project, write_project};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("quarry-tiles-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A tileset where tile n is filled with colour (n, 255 - n, 7).
fn write_tileset(path: &Path, side: u32, tile: u32) {
    let per_row = side / tile;
    let img = image::RgbaImage::from_fn(side, side, |x, y| {
        let n = ((y / tile) * per_row + x / tile) as u8;
        image::Rgba([n, 255 - n, 7, 255])
    });
    img.save(path).unwrap();
}

fn registry() -> CommandRegistry {
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles::default()));
    r
}

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

fn tiles(doc: &Document) -> &Mesh {
    doc.objects.by_name("Tiles").unwrap().mesh.as_deref().unwrap()
}

fn face_normal(mesh: &Mesh, face: usize) -> Vec3 {
    let c = &mesh.faces[face].corners;
    let p = |k: usize| mesh.positions[c[k].vertex as usize];
    (p(1) - p(0)).cross(p(2) - p(0)).normalize()
}

#[test]
fn import_slices_into_a_grid_and_undo_removes_it() {
    let dir = folder("import");
    let png = dir.join("sheet.png");
    write_tileset(&png, 256, 16);
    let mut r = registry();
    let reply = run(&mut r, "tile.import_tileset", json!({ "path": png, "tile_px": 16 }));
    assert_eq!((reply["columns"].as_u64(), reply["rows"].as_u64()), (Some(16), Some(16)));

    let file = dir.join("p.quarry");
    write_project(r.document(), &file).unwrap();
    let back = read_project(&file).unwrap();
    let (a, b) = (r.document().images.iter().next().unwrap(), back.images.iter().next().unwrap());
    assert!(a.pixels == b.pixels, "imported art survives save and reopen");

    run(&mut r, "history.undo", json!({}));
    assert!(r.document().images.is_empty(), "undo removes the tileset");
}

fn room(test: &str) -> (PathBuf, CommandRegistry) {
    let dir = folder(test);
    let png = dir.join("sheet.png");
    write_tileset(&png, 64, 16);
    let mut r = registry();
    run(&mut r, "tile.import_tileset", json!({ "path": png }));
    (dir, r)
}

#[test]
fn a_five_cell_drag_is_five_quads_in_one_step() {
    let (_, mut r) = room("drag");
    run(&mut r, "tile.brush", json!({ "tile": 5 }));
    let before = r.history().entries().len();
    let cells: Vec<[i32; 2]> = (0..5).map(|i| [i, 0]).collect();
    run(&mut r, "tile.place", json!({ "cells": cells }));
    assert_eq!(r.history().entries().len(), before + 1);
    let mesh = tiles(r.document());
    assert_eq!(mesh.faces.len(), 5);
    assert_eq!(mesh.positions.len(), 12, "neighbouring tiles share vertices");
    assert!(mesh.faces.iter().all(|f| f.tile == Some(5)));
    // Tile 5 is column 1, row 1 of a 4-wide sheet: pixels 16..32.
    let xs: Vec<f32> = mesh.faces[0].corners.iter().map(|c| c.uv.x).collect();
    assert!(xs.iter().all(|x| *x == 16.0 || *x == 32.0), "{xs:?}");
    assert!(face_normal(mesh, 0).distance(Vec3::Y) < 1e-5, "floor tiles face up");
}

#[test]
fn fill_then_erase_a_row_leaves_six() {
    let (_, mut r) = room("fill");
    run(&mut r, "tile.fill_rect", json!({ "a": [0, 0], "b": [2, 2] }));
    assert_eq!(tiles(r.document()).faces.len(), 9);
    run(&mut r, "tile.erase_rect", json!({ "a": [0, 1], "b": [2, 1] }));
    let mesh = tiles(r.document());
    assert_eq!(mesh.faces.len(), 6);
    assert_eq!(mesh.positions.len(), 16, "orphan vertices are dropped");
    // Placing on a filled cell replaces rather than stacks.
    run(&mut r, "tile.brush", json!({ "tile": 3 }));
    run(&mut r, "tile.place", json!({ "cells": [[0, 0]] }));
    assert_eq!(tiles(r.document()).faces.len(), 6);
}

#[test]
fn locking_to_x_makes_walls_facing_x() {
    let (_, mut r) = room("lock");
    run(&mut r, "tile.plane", json!({ "axis": "x" }));
    run(&mut r, "tile.fill_rect", json!({ "a": [0, 0], "b": [1, 1] }));
    let mesh = tiles(r.document());
    for f in 0..mesh.faces.len() {
        assert!(face_normal(mesh, f).distance(Vec3::X) < 1e-5);
    }
}

#[test]
fn rotating_the_brush_rotates_uvs_and_eyedrop_reads_it_back() {
    let (_, mut r) = room("rotate");
    run(&mut r, "tile.brush", json!({ "tile": 2 }));
    run(&mut r, "tile.place", json!({ "cells": [[0, 0]] }));
    run(&mut r, "tile.brush", json!({ "rotate": 90 }));
    run(&mut r, "tile.place", json!({ "cells": [[1, 0]] }));
    let mesh = tiles(r.document());
    let (plain, turned) = (&mesh.faces[0].corners, &mesh.faces[1].corners);
    assert_eq!(turned[0].uv, plain[1].uv, "a quarter turn shifts the UV corners by one");

    run(&mut r, "tile.brush", json!({ "tile": 0, "rotate": 0 }));
    let brush = run(&mut r, "tile.eyedrop", json!({ "object": "Tiles", "face": 1 }))["brush"].clone();
    assert_eq!((brush["col"].as_u64(), brush["row"].as_u64()), (Some(2), Some(0)));
    assert_eq!(brush["rotate"], 1);
}

#[test]
fn placing_without_a_brush_says_what_to_do() {
    let mut r = registry();
    let err = r.execute("tile.place", json!({}), Source::Agent).unwrap_err();
    assert_eq!(err.code, ErrorCode::NothingSelected);
    assert!(err.message.contains("tile.brush"));
    assert!(r.document().objects.is_empty(), "a failed place creates nothing");
}

#[test]
fn resize_keeps_pixel_uvs_and_warns_and_missing_links_still_open() {
    let (dir, mut r) = room("resize");
    run(&mut r, "tile.place", json!({}));
    let uv_before = tiles(r.document()).faces[0].corners[2].uv;

    let unchanged = run(&mut r, "art.reload", json!({ "image": "sheet" }));
    assert_eq!(unchanged["changed"], false);

    write_tileset(&dir.join("sheet.png"), 128, 16);
    let reloaded = run(&mut r, "art.reload", json!({ "image": "sheet" }));
    assert!(reloaded["warning"].as_str().unwrap().contains("64x64 to 128x128"));
    assert_eq!(tiles(r.document()).faces[0].corners[2].uv, uv_before);

    run(&mut r, "history.undo", json!({}));
    assert_eq!(r.document().images.iter().next().unwrap().size, [64, 64], "undo restores the old pixels");

    let file = dir.join("room.quarry");
    write_project(r.document(), &file).unwrap();
    std::fs::rename(dir.join("sheet.png"), dir.join("moved.png")).unwrap();
    let mut reopened = registry();
    run(&mut reopened, "project.open", json!({ "path": file }));
    let listed = run(&mut reopened, "query.images", json!({}));
    assert_eq!(listed["images"][0]["missing"], true);
    assert_eq!(listed["images"][0]["size"], json!([64, 64]), "the embedded copy is used");
}
