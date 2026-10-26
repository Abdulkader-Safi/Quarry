//! M1 core: model round trip, one door, failure changes nothing, shared undo.

use glam::{Quat, Vec2, Vec3, Vec4};
use quarry_core::history::MAX_ENTRIES;
use quarry_core::id::Arena;
use quarry_core::model::*;
use quarry_core::{Command, CommandRegistry, CommandResult, Context, ErrorCode, Source};
use serde_json::{Value, json};
use std::sync::Arc;

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> CommandResult {
    r.execute(name, args, Source::Human)
}

/// A document touching every §5 type, weights and keys included.
fn full_document() -> Document {
    let mut doc = Document::new("Robot");
    let tileset = doc.images.insert(Image {
        id: quarry_core::id::Id::new(0, 0),
        name: "tiles".into(),
        role: ImageRole::Tileset,
        size: [256, 256],
        tile_px: 16,
        padding: 1,
        zip_path: "images/tiles.png".into(),
        source_path: Some("/art/tiles.psd".into()),
        source_hash: 0xDEAD_BEEF,
        filter: Filter::Nearest,
        watch: true,
        pixels: Arc::from(vec![]),
    });
    let skeleton = doc.skeletons.insert(Skeleton {
        id: quarry_core::id::Id::new(0, 0),
        name: "rig".into(),
        bones: vec![
            Bone {
                name: "hip".into(),
                parent: None,
                head: Vec3::ZERO,
                tail: Vec3::Y,
                roll: 0.0,
            },
            Bone {
                name: "leg.L".into(),
                parent: Some(0),
                head: Vec3::Y,
                tail: Vec3::new(0.2, 0.0, 0.0),
                roll: 0.5,
            },
        ],
        ik_chains: vec![IkChain {
            tip_bone: 1,
            target: Vec3::new(0.2, 0.0, 0.1),
            pole: Vec3::Z,
        }],
    });
    let mut body = Object::new("body", ObjectKind::Mesh);
    body.transform.rotation = Quat::from_rotation_y(0.7);
    body.skin = Some(Skin {
        skeleton,
        bind: Bind::Weighted,
    });
    body.properties
        .insert("team".into(), PropValue::Number(1.0));
    body.mesh = Some(Arc::new(Mesh {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        weights: vec![
            Influences {
                bones: [0, 1, 0, 0],
                weights: [0.75, 0.25, 0.0, 0.0],
            };
            3
        ],
        faces: vec![Face {
            corners: (0..3)
                .map(|v| Corner {
                    vertex: v,
                    uv: Vec2::new(v as f32 * 16.0, 0.0),
                    color: Vec4::new(1.0, 0.5, 0.25, 1.0),
                })
                .collect(),
            image: Some(tileset),
            smooth_group: 1,
            tile: Some(3),
            hidden: false,
        }],
        mirror: Some(Axis::X),
    }));
    let body = doc.objects.insert(body);
    let mut spawn = Object::new("spawn_player", ObjectKind::Marker);
    spawn.parent = Some(body);
    spawn.marker_type = Some(MarkerType::Spawn);
    spawn.collision = true;
    doc.objects.insert(spawn);
    doc.clips.insert(Clip {
        id: quarry_core::id::Id::new(0, 0),
        name: "walk".into(),
        skeleton,
        length: 1.0,
        looping: true,
        tracks: vec![
            Track {
                target: TrackTarget::Bone(1),
                channel: Channel::Rotation,
                keys: vec![Key {
                    time: 0.5,
                    value: KeyValue::Quat(Quat::from_rotation_x(0.3)),
                    interp: Interp::Bezier,
                    handles: Some([Vec2::new(0.1, 0.0), Vec2::new(0.9, 1.0)]),
                }],
            },
            Track {
                target: TrackTarget::Ik(0),
                channel: Channel::IkTarget,
                keys: vec![Key {
                    time: 0.0,
                    value: KeyValue::Vec3(Vec3::new(0.0, 0.1, 0.0)),
                    interp: Interp::Step,
                    handles: None,
                }],
            },
        ],
    });
    doc.palettes.push(Palette {
        name: "pico".into(),
        colors: vec![[0, 0, 0, 255], [255, 241, 232, 255]],
        weights: Some(vec![1.0, 0.5]),
    });
    doc.export_presets.push(ExportPreset {
        name: "game".into(),
        path: "out/robot.glb".into(),
        format: ExportFormat::Glb,
        selection: ExportSelection::Objects(vec!["body".into()]),
        scale: 1.0,
    });
    doc
}

#[test]
fn whole_model_round_trips_through_json() {
    let doc = full_document();
    let text = serde_json::to_string_pretty(&doc).unwrap();
    let back: Document = serde_json::from_str(&text).unwrap();
    assert_eq!(back, doc);
    assert!(
        text.contains("\"o1g1\"") || text.contains("\"o0g1\""),
        "IDs are short strings"
    );
}

#[test]
fn ids_and_names_both_resolve() {
    let mut arena: Arena<Object> = Arena::default();
    let a = arena.insert(Object::new("crate", ObjectKind::Mesh));
    assert_eq!(arena.find("crate").unwrap().id, a);
    assert_eq!(arena.find(&a.to_string()).unwrap().name, "crate");
    arena.remove(a);
    let b = arena.insert(Object::new("crate", ObjectKind::Mesh));
    assert_eq!(a.slot, b.slot);
    assert!(
        arena.get(a).is_none(),
        "a stale ID misses after the slot is reused"
    );
    assert_eq!(arena.unique_name("crate"), "crate.001");
}

#[test]
fn new_project_has_pixel_art_defaults() {
    let mut r = CommandRegistry::new();
    run(&mut r, "project.new", json!({ "name": "Room" })).unwrap();
    let s = &r.document().settings;
    assert_eq!(
        (s.tile_px, s.filter, s.fps, s.grid_step),
        (16, Filter::Nearest, 30, 1.0)
    );
    assert!(s.palette_lock);
}

#[test]
fn failed_command_changes_nothing() {
    let mut r = CommandRegistry::new();
    run(&mut r, "mesh.add_primitive", json!({ "kind": "box" })).unwrap();
    let before = serde_json::to_string(r.document()).unwrap();
    let entries = r.history().entries().len();

    for (name, args, code) in [
        (
            "scene.rename",
            json!({ "object": "nope", "name": "x" }),
            ErrorCode::NotFound,
        ),
        (
            "scene.rename",
            json!({ "object": "Box" }),
            ErrorCode::InvalidArgs,
        ),
        (
            "project.set_settings",
            json!({ "fps": 0 }),
            ErrorCode::OutOfBounds,
        ),
        (
            "mesh.add_primitive",
            json!({ "kind": "box", "name": "Box" }),
            ErrorCode::Conflict,
        ),
        ("project.save", json!({}), ErrorCode::InvalidArgs),
        ("history.redo", json!({}), ErrorCode::Conflict),
        ("no.such", json!({}), ErrorCode::NotFound),
    ] {
        let err = run(&mut r, name, args).unwrap_err();
        assert_eq!(err.code, code, "{name}: {}", err.message);
        assert!(err.message.ends_with('.'), "{name}: message is a sentence");
    }
    assert_eq!(serde_json::to_string(r.document()).unwrap(), before);
    assert_eq!(r.history().entries().len(), entries);
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct Explode {}

impl Command for Explode {
    const NAME: &'static str = "test.explode";
    const DESCRIPTION: &'static str = "Edits the document, then panics.";
    fn run(self, cx: &mut Context) -> CommandResult {
        cx.doc().name = "half-done".into();
        panic!("boom");
    }
}

#[test]
fn panicking_command_commits_nothing() {
    let mut r = CommandRegistry::new();
    r.register::<Explode>();
    let err = r
        .execute("test.explode", json!({}), Source::Agent)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(r.document().name, "Untitled");
    assert!(!r.history().can_undo());
}

#[test]
fn agent_edit_is_one_labelled_undo_step() {
    let mut r = CommandRegistry::new();
    r.execute(
        "mesh.add_primitive",
        json!({ "kind": "box" }),
        Source::Agent,
    )
    .unwrap();
    let last = r.history().entries().last().unwrap();
    assert_eq!(last.display_label(), "Agent: add primitive");
    r.execute("history.undo", json!({}), Source::Human).unwrap();
    assert!(r.document().objects.is_empty());
    r.execute("history.redo", json!({}), Source::Human).unwrap();
    assert_eq!(r.document().objects.len(), 1);
}

#[test]
fn queries_do_not_enter_history() {
    let mut r = CommandRegistry::new();
    run(&mut r, "query.scene", json!({})).unwrap();
    run(&mut r, "project.info", json!({})).unwrap();
    assert!(!r.history().can_undo());
    assert!(!r.is_dirty());
}

/// Tiny deterministic generator, so the test needs no extra dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_command(rng: &mut Rng, r: &CommandRegistry) -> (&'static str, Value) {
    let names: Vec<String> = r
        .document()
        .objects
        .iter()
        .map(|o| o.name.clone())
        .collect();
    let pick = |rng: &mut Rng| {
        names
            .get(rng.below(names.len().max(1)))
            .cloned()
            .unwrap_or_else(|| "missing".into())
    };
    match rng.below(8) {
        0 | 1 => (
            "mesh.add_primitive",
            json!({ "kind": "box", "size": [1.0 + rng.below(3) as f32, 1.0, 1.0] }),
        ),
        2 => (
            "scene.rename",
            json!({ "object": pick(rng), "name": format!("n{}", rng.below(50)) }),
        ),
        3 => (
            "scene.visibility",
            json!({ "objects": [pick(rng)], "visible": rng.below(2) == 0 }),
        ),
        4 => (
            "scene.lock",
            json!({ "objects": [pick(rng)], "locked": rng.below(2) == 0 }),
        ),
        5 => ("scene.delete", json!({ "objects": [pick(rng)] })),
        6 => ("project.set_settings", json!({ "fps": 12 + rng.below(50) })),
        _ => (
            "project.new",
            json!({ "name": format!("p{}", rng.below(9)) }),
        ),
    }
}

#[test]
fn random_edits_then_undo_all_returns_to_start() {
    for seed in 1..=8u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut r = CommandRegistry::new();
        for _ in 0..5 {
            run(&mut r, "mesh.add_primitive", json!({ "kind": "box" })).unwrap();
        }
        let start = r.document().clone();
        let start_entries = r.history().entries().len();

        for _ in 0..150 {
            let (name, args) = random_command(&mut rng, &r);
            let source = if rng.below(2) == 0 {
                Source::Human
            } else {
                Source::Agent
            };
            let before = r.document().clone();
            if r.execute(name, args, source).is_err() {
                assert_eq!(
                    r.document(),
                    &before,
                    "{name} failed but changed the document"
                );
            }
        }
        while r.history().entries().len() > start_entries {
            run(&mut r, "history.undo", json!({})).unwrap();
        }
        assert_eq!(r.document(), &start, "seed {seed}");
    }
}

#[test]
fn history_keeps_the_newest_200() {
    let mut r = CommandRegistry::new();
    for i in 0..(MAX_ENTRIES + 25) {
        run(
            &mut r,
            "project.set_settings",
            json!({ "fps": 1 + i % 200 }),
        )
        .unwrap();
    }
    assert_eq!(r.history().entries().len(), MAX_ENTRIES);
}

#[test]
fn every_command_lists_a_schema_and_description() {
    let r = CommandRegistry::new();
    let names: Vec<_> = r.commands().map(|c| c.name).collect();
    assert!(names.contains(&"project.new") && names.contains(&"history.undo"));
    for c in r.commands() {
        let (area, verb) = c.name.split_once('.').expect("area.verb");
        assert!(!area.is_empty() && !verb.is_empty() && !verb.contains('.'));
        assert!(c.schema.is_object(), "{} has a schema", c.name);
        assert!(
            c.description.ends_with('.'),
            "{} description is a sentence",
            c.name
        );
    }
}

#[test]
fn view_changes_stay_out_of_history_and_undo_keeps_the_view() {
    let mut r = CommandRegistry::new();
    run(&mut r, "mesh.add_primitive", json!({ "kind": "box" })).unwrap();
    run(&mut r, "view.camera", json!({ "preset": "top" })).unwrap();
    run(&mut r, "view.display", json!({ "grid": false })).unwrap();
    assert_eq!(r.history().entries().len(), 1, "only the box is an edit");
    let view = r.document().view;
    run(&mut r, "history.undo", json!({})).unwrap();
    assert!(r.document().objects.is_empty());
    assert_eq!(
        r.document().view,
        view,
        "undo leaves the camera and toggles alone"
    );
}

#[test]
fn primitives_land_on_the_snapped_crosshair() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "position": [3, 0, 0], "name": "A" }),
    )
    .unwrap();
    run(
        &mut r,
        "view.cursor",
        json!({ "position": [9, 9, 9], "snap_to": "vertex", "object": "A" }),
    )
    .unwrap();
    assert_eq!(r.document().view.cursor, Vec3::new(3.5, 0.5, 0.5));
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "B" }),
    )
    .unwrap();
    let b = r.document().objects.by_name("B").unwrap();
    assert_eq!(b.transform.translation, Vec3::new(3.5, 0.5, 0.5));

    run(
        &mut r,
        "view.cursor",
        json!({ "position": [1.4, 0.6, -2.5], "snap_to": "grid" }),
    )
    .unwrap();
    assert_eq!(r.document().view.cursor, Vec3::new(1.0, 1.0, -3.0));
    let err = run(&mut r, "view.cursor", json!({ "snap_to": "object" })).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgs);
}

#[test]
fn frame_centres_the_named_object() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "position": [40, -3, 25] }),
    )
    .unwrap();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Near" }),
    )
    .unwrap();
    run(&mut r, "view.frame", json!({ "objects": ["Box"] })).unwrap();
    assert_eq!(r.document().view.camera.target, Vec3::new(40.0, -3.0, 25.0));
    run(&mut r, "project.new", json!({})).unwrap();
    let err = run(&mut r, "view.frame", json!({})).unwrap_err();
    assert_eq!(err.code, ErrorCode::NothingSelected);
}

#[test]
fn every_primitive_is_one_undo_step_with_sane_uvs() {
    let mut r = CommandRegistry::new();
    for kind in ["box", "wedge", "cylinder", "cone", "sphere", "plane"] {
        let before = r.history().entries().len();
        run(
            &mut r,
            "mesh.add_primitive",
            json!({ "kind": kind, "segments": 8 }),
        )
        .unwrap();
        assert_eq!(r.history().entries().len(), before + 1, "{kind}");
    }
    for o in r.document().objects.iter() {
        let mesh = o.mesh.as_deref().unwrap();
        for f in &mesh.faces {
            assert!(f.corners.len() == 3 || f.corners.len() == 4, "{}", o.name);
            assert!(
                f.corners
                    .iter()
                    .all(|c| c.uv.is_finite() && c.uv.x >= 0.0 && c.uv.x <= 16.0)
            );
        }
    }
    let cyl = r
        .document()
        .objects
        .by_name("Cylinder")
        .unwrap()
        .mesh
        .as_deref()
        .unwrap();
    assert_eq!(cyl.faces.iter().filter(|f| f.corners.len() == 4).count(), 8);
    let err = run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "sphere", "segments": 2 }),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::OutOfBounds);
}

#[test]
fn box_select_four_then_add_a_fifth() {
    let mut r = CommandRegistry::new();
    run(&mut r, "mesh.add_primitive", json!({ "kind": "box" })).unwrap();
    run(&mut r, "view.camera", json!({ "preset": "front" })).unwrap();
    run(&mut r, "view.frame", json!({})).unwrap();
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "vertex", "object": "Box" }),
    )
    .unwrap();
    // Left half of a front view: the four corners at x = -0.5.
    let s = run(
        &mut r,
        "select.box",
        json!({ "rect": [0.0, 0.0, 0.5, 1.0], "aspect": 1.0 }),
    )
    .unwrap();
    assert_eq!(s["vertices"], 4);
    let right = (0..8u32)
        .find(|v| !r.document().selection.vertices.contains(v))
        .unwrap();
    let s = run(
        &mut r,
        "select.by_ids",
        json!({ "vertices": [right], "add": true }),
    )
    .unwrap();
    assert_eq!(s["vertices"], 5);
    assert!(r.history().entries().len() == 1, "selecting is not an edit");

    let before = r.document().selection.clone();
    run(&mut r, "select.invert", json!({})).unwrap();
    assert_eq!(r.document().selection.vertices.len(), 3);
    run(&mut r, "select.invert", json!({})).unwrap();
    assert_eq!(
        r.document().selection,
        before,
        "invert twice is the original set"
    );
}

#[test]
fn select_faces_by_normal_and_bad_indices_are_refused() {
    let mut r = CommandRegistry::new();
    run(&mut r, "mesh.add_primitive", json!({ "kind": "box" })).unwrap();
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "face", "object": "Box" }),
    )
    .unwrap();
    let s = run(
        &mut r,
        "select.by_normal",
        json!({ "direction": [0, 1, 0] }),
    )
    .unwrap();
    assert_eq!(s["faces"], 1);
    let q = run(&mut r, "query.selection", json!({})).unwrap();
    assert_eq!(q["bounds"][0][1], 0.5, "the top face is at y = 0.5");
    let err = run(&mut r, "select.by_ids", json!({ "faces": [99] })).unwrap_err();
    assert_eq!(err.code, ErrorCode::OutOfBounds);
    assert_eq!(
        r.document().selection.faces.len(),
        1,
        "a refused select changes nothing"
    );

    run(&mut r, "scene.delete", json!({ "objects": ["Box"] })).unwrap();
    assert_eq!(
        r.document().selection.mode,
        quarry_core::model::SelectMode::Object,
        "editing a deleted mesh ends"
    );
}
