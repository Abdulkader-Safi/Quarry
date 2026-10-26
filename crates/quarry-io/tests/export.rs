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

fn rigged_arm() -> CommandRegistry {
    let mut r = registry();
    run(&mut r, "rig.skeleton_new", json!({ "name": "rig" }));
    run(&mut r, "rig.bone_add", json!({ "name": "upper.L", "head": [0, 1, 0], "tail": [1, 1, 0] }));
    run(&mut r, "rig.bone_add", json!({ "name": "lower.L", "parent": "upper.L", "tail": [2, 1, 0] }));
    run(&mut r, "mesh.add_primitive", json!({ "kind": "box", "name": "Arm", "size": [2, 0.2, 0.2], "position": [1, 1, 0] }));
    run(&mut r, "rig.bind_auto", json!({ "object": "Arm" }));
    r
}

#[test]
fn a_skinned_arm_exports_skin_joints_weights_and_bind_matrices() {
    let dir = folder("arm");
    let mut r = rigged_arm();
    let reply = run(&mut r, "export.run", json!({ "path": dir.join("arm.glb") }));
    assert_eq!(reply["report"]["errors"], json!([]));
    let bytes = golden("arm.glb", &dir.join("arm.glb"));

    let (doc, buffers, _) = gltf::import_slice(&bytes).unwrap();
    assert_eq!(doc.skins().count(), 1);
    let skin = doc.skins().next().unwrap();
    let names: Vec<_> = skin.joints().map(|j| j.name().unwrap().to_string()).collect();
    assert_eq!(names, ["upper.L", "lower.L"]);
    let ibms: Vec<[[f32; 4]; 4]> = skin.reader(|b| Some(&buffers[b.index()])).read_inverse_bind_matrices().unwrap().collect();
    // Each joint's rest world matrix times its inverse bind matrix is identity.
    let world = |n: gltf::Node| glam::Mat4::from_cols_array_2d(&n.transform().matrix());
    let upper = world(skin.joints().next().unwrap());
    let lower = upper * world(skin.joints().nth(1).unwrap());
    for (m, ibm) in [upper, lower].iter().zip(&ibms) {
        let i = *m * glam::Mat4::from_cols_array_2d(ibm);
        assert!(i.abs_diff_eq(glam::Mat4::IDENTITY, 1e-5), "{i}");
    }
    let arm = doc.nodes().find(|n| n.name() == Some("Arm")).unwrap();
    assert!(arm.skin().is_some());
    let prim = arm.mesh().unwrap().primitives().next().unwrap();
    let reader = prim.reader(|b| Some(&buffers[b.index()]));
    let weights: Vec<[f32; 4]> = reader.read_weights(0).unwrap().into_f32().collect();
    assert!(weights.iter().all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-5));
    assert!(reader.read_joints(0).is_some());
}

#[test]
fn posing_before_export_does_not_move_the_bind_pose() {
    let dir = folder("pose");
    let mut r = rigged_arm();
    run(&mut r, "export.run", json!({ "path": dir.join("rest.glb") }));
    run(&mut r, "rig.pose", json!({ "bone": "lower.L", "rotation": [0, 0, 70] }));
    run(&mut r, "export.run", json!({ "path": dir.join("posed.glb") }));
    assert!(std::fs::read(dir.join("rest.glb")).unwrap() == std::fs::read(dir.join("posed.glb")).unwrap());
}
