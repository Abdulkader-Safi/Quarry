//! M5 import: our own goldens come back as editable objects, skeletons and clips.

use quarry_core::model::ObjectKind;
use quarry_core::{CommandRegistry, ErrorCode, Source};
use quarry_io::project::QuarryFiles;
use serde_json::{Value, json};
use std::path::PathBuf;

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens")
        .join(name)
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

#[test]
fn the_room_comes_back_as_separate_named_objects() {
    let mut r = registry();
    run(&mut r, "import.gltf", json!({ "path": golden("room.glb") }));
    assert_eq!(r.history().entries().len(), 1, "import is one undo step");
    let doc = r.document();
    let floor = doc.objects.by_name("Floor").expect("Floor");
    assert!(floor.collision);
    assert!(floor.mesh.as_ref().unwrap().faces.len() >= 9, "3x3 tiles");
    let spawn = doc.objects.by_name("spawn_player").expect("marker");
    assert_eq!(spawn.kind, ObjectKind::Marker);
    assert_eq!(spawn.parent, Some(floor.id), "hierarchy kept");
    assert_eq!(
        spawn.properties.get("team"),
        Some(&quarry_core::model::PropValue::Number(1.0))
    );
    assert!(
        doc.objects.by_name("Crate").is_some() && doc.objects.by_name("Crate instance").is_some()
    );
    let image = doc.images.iter().next().expect("texture imported");
    assert_eq!(image.filter, quarry_core::model::Filter::Nearest);
    assert_eq!(
        image.pixels.len(),
        (image.size[0] * image.size[1] * 4) as usize
    );

    run(&mut r, "history.undo", json!({}));
    assert!(r.document().objects.is_empty() && r.document().images.is_empty());
}

#[test]
fn the_skinned_bar_brings_its_skeleton_and_clip() {
    let mut r = registry();
    run(
        &mut r,
        "import.gltf",
        json!({ "path": golden("skin-bar.glb") }),
    );
    let doc = r.document();
    let skeleton = doc.skeletons.iter().next().expect("skeleton");
    assert_eq!(skeleton.bones.len(), 3);
    assert_eq!(skeleton.bones[1].parent, Some(0));
    assert!(
        (skeleton.bones[2].head.y - 2.0).abs() < 1e-4,
        "rest pose from the bind matrices"
    );
    let clip = doc.clips.iter().next().expect("clip");
    assert_eq!(clip.name, "bend");
    assert_eq!(clip.tracks.len(), 2);
    assert!((clip.length - 2.0).abs() < 1e-5);
    let bar = doc.objects.by_name("bar").expect("bar");
    assert_eq!(bar.skin.as_ref().unwrap().skeleton, skeleton.id);
    let mesh = bar.mesh.as_ref().unwrap();
    assert_eq!(mesh.weights.len(), mesh.positions.len());
    assert!(
        mesh.weights
            .iter()
            .all(|w| (w.weights.iter().sum::<f32>() - 1.0).abs() < 1e-5)
    );
    assert!(
        doc.objects.by_name("bone.0").is_none(),
        "joints are bones, not objects"
    );
}

#[test]
fn a_broken_file_is_a_plain_error() {
    let dir = std::env::temp_dir().join(format!("quarry-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("broken.glb");
    std::fs::write(&bad, b"glTF\x02\0\0\0garbage").unwrap();
    let mut r = registry();
    let err = r
        .execute("import.gltf", json!({ "path": bad }), Source::Human)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("broken.glb"));
    assert!(r.document().objects.is_empty());
}

#[test]
fn an_exported_rig_imports_back_with_the_same_rest_skeleton() {
    let mut r = registry();
    run(&mut r, "import.gltf", json!({ "path": golden("arm.glb") }));
    let doc = r.document();
    let s = doc.skeletons.iter().next().expect("skeleton");
    let names: Vec<&str> = s.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["upper.L", "lower.L"]);
    assert!(s.bones[0].head.distance(glam::Vec3::new(0.0, 1.0, 0.0)) < 1e-4);
    assert!(s.bones[1].head.distance(glam::Vec3::new(1.0, 1.0, 0.0)) < 1e-4);
    let arm = doc.objects.by_name("Arm").expect("Arm");
    assert_eq!(arm.skin.as_ref().unwrap().skeleton, s.id);
    assert!(
        doc.objects.by_name("rig").is_some(),
        "the skeleton root node is kept as a layer"
    );
}

#[test]
fn the_sample_robot_opens_with_its_rig_and_clips() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/samples/sample-robot.quarry");
    let mut r = registry();
    run(&mut r, "project.open", json!({ "path": path }));
    let doc = r.document();
    assert_eq!(doc.skeletons.iter().next().unwrap().bones.len(), 13);
    let clips: Vec<&str> = doc.clips.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(clips, ["idle", "walk"]);
    assert_eq!(doc.objects.len(), 11);
}
