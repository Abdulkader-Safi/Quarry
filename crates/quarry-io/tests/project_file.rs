//! M1 `.quarry` file: round trip, plain zip layout, damaged files, atomic save.

use glam::{Quat, Vec2, Vec3, Vec4};
use quarry_core::id::Id;
use quarry_core::model::*;
use quarry_core::{CommandRegistry, ErrorCode, Source};
use quarry_io::project::{QuarryFiles, decode, encode, read_project, write_project};
use serde_json::json;
use std::io::{Read, Write};
use std::sync::Arc;

fn image(name: &str, role: ImageRole, w: u32, h: u32, seed: u8) -> Image {
    let pixels: Vec<u8> = (0..w * h * 4)
        .map(|i| (i as u8).wrapping_mul(seed))
        .collect();
    Image {
        id: Id::new(0, 0),
        name: name.into(),
        role,
        size: [w, h],
        tile_px: 16,
        padding: 0,
        zip_path: String::new(),
        source_path: Some(format!("/art/{name}.png")),
        source_hash: seed as u64,
        filter: Filter::Nearest,
        watch: true,
        pixels: Arc::from(pixels),
    }
}

/// FR-PROJ-01's scene: 2 tilesets, 3 objects, a rig and 2 clips.
fn scene() -> Document {
    let mut doc = Document::new("Room");
    let floor = doc
        .images
        .insert(image("floor", ImageRole::Tileset, 64, 32, 3));
    doc.images
        .insert(image("walls", ImageRole::Tileset, 32, 32, 7));
    doc.images
        .insert(image("robot", ImageRole::Texture, 32, 32, 11));
    let rig = doc.skeletons.insert(Skeleton {
        id: Id::new(0, 0),
        name: "rig".into(),
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                head: Vec3::ZERO,
                tail: Vec3::Y,
                roll: 0.0,
            },
            Bone {
                name: "arm.L".into(),
                parent: Some(0),
                head: Vec3::Y,
                tail: Vec3::new(1.0, 1.0, 0.0),
                roll: 0.1,
            },
        ],
        ik_chains: vec![IkChain {
            tip_bone: 1,
            target: Vec3::X,
            pole: Vec3::Z,
        }],
    });

    let mut crate_ = Object::new("crate", ObjectKind::Mesh);
    crate_.skin = Some(Skin {
        skeleton: rig,
        bind: Bind::Weighted,
    });
    crate_.mesh = Some(Arc::new(Mesh {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
        weights: vec![
            Influences {
                bones: [0, 1, 0, 0],
                weights: [0.6, 0.4, 0.0, 0.0]
            };
            4
        ],
        faces: vec![Face {
            corners: (0..4)
                .map(|v| Corner {
                    vertex: v,
                    uv: Vec2::new(16.0 * (v % 2) as f32, 16.0 * (v / 2) as f32),
                    color: Vec4::new(0.25, 0.5, 1.0, 1.0),
                })
                .collect(),
            image: Some(floor),
            smooth_group: 2,
            tile: Some(5),
            hidden: false,
        }],
        mirror: Some(Axis::X),
    }));
    let crate_ = doc.objects.insert(crate_);
    let mut spawn = Object::new("spawn_player", ObjectKind::Marker);
    spawn.parent = Some(crate_);
    spawn.marker_type = Some(MarkerType::Spawn);
    spawn
        .properties
        .insert("team".into(), PropValue::Number(1.0));
    doc.objects.insert(spawn);
    doc.objects.insert(Object::new("props", ObjectKind::Layer));

    for (name, interp) in [("idle", Interp::Linear), ("walk", Interp::EaseInOut)] {
        doc.clips.insert(Clip {
            id: Id::new(0, 0),
            name: name.into(),
            skeleton: rig,
            length: 1.0,
            looping: true,
            tracks: vec![Track {
                target: TrackTarget::Bone(1),
                channel: Channel::Rotation,
                keys: vec![
                    Key {
                        time: 0.0,
                        value: KeyValue::Quat(Quat::IDENTITY),
                        interp,
                        handles: None,
                    },
                    Key {
                        time: 0.5,
                        value: KeyValue::Quat(Quat::from_rotation_z(0.4)),
                        interp,
                        handles: None,
                    },
                ],
            }],
        });
    }
    doc
}

fn temp_dir(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("quarry-io-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn save_then_open_is_identical() {
    let dir = temp_dir("roundtrip");
    let path = dir.join("room.quarry");
    let mut doc = scene();
    write_project(&doc, &path).unwrap();
    let back = read_project(&path).unwrap();
    // Saving fills in where each image sits inside the zip.
    for id in doc.images.ids().collect::<Vec<_>>() {
        doc.images.get_mut(id).unwrap().zip_path = format!("images/{id}.png");
    }
    assert_eq!(back, doc);
    let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert_eq!(leftovers.len(), 1, "no temp file is left behind");
}

#[test]
fn file_is_a_plain_zip_with_json_and_stored_pngs() {
    let bytes = encode(&scene()).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut json = String::new();
    zip.by_name("project.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["format_version"], 1);
    assert!(
        value["objects"][0]["mesh"].is_null(),
        "mesh arrays live in meshes/*.bin"
    );

    let pngs: Vec<String> = zip
        .file_names()
        .map(|n| n.unwrap().into_owned())
        .filter(|n| n.ends_with(".png"))
        .collect();
    assert_eq!(pngs.len(), 3);
    for name in pngs {
        let mut entry = zip.by_name(&name).unwrap();
        assert_eq!(
            entry.compression(),
            zip::CompressionMethod::Stored,
            "{name}"
        );
        let mut png = Vec::new();
        entry.read_to_end(&mut png).unwrap();
        image::load_from_memory(&png).expect("a PNG any editor can open");
    }
}

#[test]
fn damaged_and_hostile_files_are_errors() {
    let good = encode(&scene()).unwrap();
    assert!(decode(b"not a zip").is_err());
    assert!(decode(&good[..good.len() / 2]).is_err());

    // A newer format is refused, never loaded lossily.
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("project.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"{"format_version": 99}"#).unwrap();
    let newer = zip.finish().unwrap().into_inner();
    assert!(decode(&newer).unwrap_err().contains("newer Quarry"));

    // A mesh .bin that lies about its counts.
    let mut doc = Document::new("x");
    doc.objects.insert(Object::new("a", ObjectKind::Mesh));
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    zip.start_file("project.json", o).unwrap();
    zip.write_all(&serde_json::to_vec(&doc).unwrap()).unwrap();
    zip.start_file("meshes/o0g1.bin", o).unwrap();
    zip.write_all(b"QMSH\x01\0\0\0\xff\xff\xff\xff").unwrap();
    let lying = zip.finish().unwrap().into_inner();
    assert!(decode(&lying).unwrap_err().contains("meshes/o0g1.bin"));
}

#[test]
fn registry_saves_opens_and_tracks_dirty() {
    let dir = temp_dir("registry");
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles::default()));
    r.execute(
        "mesh.add_primitive",
        json!({ "kind": "box" }),
        Source::Agent,
    )
    .unwrap();
    assert!(r.is_dirty());

    let path = dir.join("level");
    r.execute("project.save_as", json!({ "path": path }), Source::Human)
        .unwrap();
    assert!(!r.is_dirty());
    assert!(dir.join("level.quarry").exists());

    r.execute(
        "scene.rename",
        json!({ "object": "Box", "name": "Crate" }),
        Source::Human,
    )
    .unwrap();
    assert!(r.is_dirty());
    r.execute("history.undo", json!({}), Source::Human).unwrap();
    assert!(!r.is_dirty(), "undo back to the saved state is clean");

    r.execute("project.new", json!({}), Source::Human).unwrap();
    r.execute(
        "project.open",
        json!({ "path": dir.join("level.quarry") }),
        Source::Human,
    )
    .unwrap();
    assert_eq!(
        r.document()
            .objects
            .by_name("Box")
            .unwrap()
            .mesh
            .as_ref()
            .unwrap()
            .faces
            .len(),
        6
    );
    assert!(!r.is_dirty());

    let err = r
        .execute(
            "project.open",
            json!({ "path": dir.join("missing.quarry") }),
            Source::Human,
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("missing.quarry"));
}
