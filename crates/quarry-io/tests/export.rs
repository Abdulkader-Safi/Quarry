//! M5 export: one .glb, shared vertices, nearest sampler, node names and
//! extras, instances, hierarchy, goldens.
//!
//! `QUARRY_UPDATE_GOLDENS=1 cargo test -p quarry-io --test export` rewrites
//! `tests/goldens/box.glb` and `room.glb`; otherwise they must match.

use quarry_core::{CommandRegistry, Source};
use quarry_io::project::QuarryFiles;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("quarry-export-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// Compare with (or rewrite) a golden, then hand back the bytes.
fn golden(name: &str, file: &Path) -> Vec<u8> {
    let bytes = std::fs::read(file).unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens")
        .join(name);
    if std::env::var_os("QUARRY_UPDATE_GOLDENS").is_some() {
        std::fs::write(&path, &bytes).unwrap();
    }
    let expected = std::fs::read(&path)
        .unwrap_or_else(|_| panic!("{name} golden missing; set QUARRY_UPDATE_GOLDENS=1"));
    assert!(
        bytes == expected,
        "{name} differs from its golden; rerun with QUARRY_UPDATE_GOLDENS=1 if that is intended"
    );
    bytes
}

fn write_tileset(path: &Path) {
    image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([(x * 8) as u8, (y * 8) as u8, 90, 255])
    })
    .save(path)
    .unwrap();
}

#[test]
fn a_box_exports_as_one_glb_with_24_vertices() {
    let dir = folder("box");
    let mut r = registry();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Crate" }),
    );
    let out = dir.join("crate");
    let reply = run(&mut r, "export.run", json!({ "path": out }));
    assert_eq!(
        reply["files"].as_array().unwrap().len(),
        1,
        "exactly one file"
    );
    assert_eq!(reply["report"]["errors"], json!([]));
    assert_eq!(reply["vertices"], 24, "shared corners, not 36");
    let bytes = golden("box.glb", &dir.join("crate.glb"));

    let (doc, _, _) = gltf::import_slice(&bytes).unwrap();
    let prim = doc.meshes().next().unwrap().primitives().next().unwrap();
    assert_eq!(prim.get(&gltf::Semantic::Positions).unwrap().count(), 24);
    assert_eq!(doc.nodes().next().unwrap().name(), Some("Crate"));
}

#[test]
fn a_tiled_room_keeps_nearest_filtering_names_and_extras() {
    let dir = folder("room");
    let png = dir.join("tiles.png");
    write_tileset(&png);
    let mut r = registry();
    run(&mut r, "tile.import_tileset", json!({ "path": png }));
    run(
        &mut r,
        "tile.fill_rect",
        json!({ "a": [0, 0], "b": [2, 2], "object": "Floor" }),
    );
    run(
        &mut r,
        "scene.set_collision",
        json!({ "objects": ["Floor"], "collision": true }),
    );
    run(
        &mut r,
        "scene.marker_add",
        json!({ "name": "spawn_player", "type": "spawn", "props": { "team": 1 }, "position": [1, 0, -1] }),
    );
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Crate", "position": [5, 0.5, 0] }),
    );
    run(
        &mut r,
        "scene.instance_add",
        json!({ "source": "Crate", "position": [7, 0.5, 0] }),
    );
    run(
        &mut r,
        "scene.parent",
        json!({ "objects": ["spawn_player"], "parent": "Floor" }),
    );
    let reply = run(
        &mut r,
        "export.run",
        json!({ "path": dir.join("room.glb") }),
    );
    assert_eq!(reply["report"]["errors"], json!([]));
    let bytes = golden("room.glb", &dir.join("room.glb"));

    let (doc, _, _) = gltf::import_slice(&bytes).unwrap();
    let sampler = doc.samplers().next().unwrap();
    assert_eq!(
        sampler.mag_filter(),
        Some(gltf::texture::MagFilter::Nearest),
        "magFilter 9728"
    );

    let node = |name: &str| {
        doc.nodes()
            .find(|n| n.name() == Some(name))
            .unwrap_or_else(|| panic!("node {name}"))
    };
    let extras = |n: gltf::Node| -> Value {
        serde_json::from_str(n.extras().as_ref().unwrap().get()).unwrap()
    };
    assert_eq!(extras(node("Floor"))["quarry"]["collision"], true);
    let spawn = extras(node("spawn_player"));
    assert_eq!(spawn["quarry"]["marker"], "spawn");
    assert_eq!(spawn["quarry"]["props"]["team"], 1.0);
    assert!(
        node("Floor")
            .children()
            .any(|c| c.name() == Some("spawn_player")),
        "hierarchy is kept"
    );

    let crate_mesh = node("Crate").mesh().unwrap().index();
    assert_eq!(
        node("Crate instance").mesh().unwrap().index(),
        crate_mesh,
        "instances share the mesh"
    );
    assert_eq!(
        doc.meshes().count(),
        2,
        "floor and crate, nothing duplicated"
    );
}

#[test]
fn separate_format_writes_gltf_bin_and_png() {
    let dir = folder("separate");
    let png = dir.join("tiles.png");
    write_tileset(&png);
    let mut r = registry();
    run(&mut r, "tile.import_tileset", json!({ "path": png }));
    run(&mut r, "tile.place", json!({}));
    let out = dir.join("out").join("debug");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let reply = run(
        &mut r,
        "export.run",
        json!({ "path": out, "format": "gltf_separate" }),
    );
    assert_eq!(reply["files"].as_array().unwrap().len(), 3, "{reply}");
    assert_eq!(reply["report"]["errors"], json!([]));
    assert!(dir.join("out/debug.gltf").is_file() && dir.join("out/debug.bin").is_file());
}

#[test]
fn exporting_nothing_says_so() {
    let mut r = registry();
    let err = r
        .execute("export.run", json!({ "path": "/tmp/x.glb" }), Source::Human)
        .unwrap_err();
    assert_eq!(err.code, quarry_core::ErrorCode::NothingSelected);
}
