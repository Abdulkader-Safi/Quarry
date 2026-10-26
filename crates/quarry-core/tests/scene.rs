//! M3 scene: parenting, order, parts, origins, instances, markers.

use glam::Vec3;
use quarry_core::{CommandRegistry, ErrorCode, Source};
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

fn world(r: &CommandRegistry, name: &str) -> Vec3 {
    let doc = r.document();
    doc.world_matrix(doc.objects.by_name(name).unwrap())
        .transform_point3(Vec3::ZERO)
}

#[test]
fn children_follow_their_parent_and_loops_are_refused() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "A" }),
    );
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "B", "position": [3, 0, 0] }),
    );
    run(
        &mut r,
        "scene.parent",
        json!({ "objects": ["B"], "parent": "A" }),
    );
    assert_eq!(
        world(&r, "B"),
        Vec3::new(3.0, 0.0, 0.0),
        "parenting keeps B in place"
    );
    run(&mut r, "select.by_ids", json!({ "objects": ["A"] }));
    run(&mut r, "mesh.transform", json!({ "translate": [0, 2, 0] }));
    assert_eq!(world(&r, "B"), Vec3::new(3.0, 2.0, 0.0), "moving A moves B");
    let err = r
        .execute(
            "scene.parent",
            json!({ "objects": ["A"], "parent": "B" }),
            Source::Agent,
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);

    run(&mut r, "scene.parent", json!({ "objects": ["B"] }));
    run(
        &mut r,
        "scene.reorder",
        json!({ "object": "B", "index": 0 }),
    );
    let tree = run(&mut r, "query.scene", json!({}));
    assert_eq!(tree["objects"][0]["name"], "B");
}

#[test]
fn six_faces_become_an_object_with_its_origin_at_the_crosshair() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Crate", "position": [5, 0, 0] }),
    );
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "face", "object": "Crate" }),
    );
    run(&mut r, "select.all", json!({}));
    run(&mut r, "view.cursor", json!({ "position": [1, 2, 3] }));
    run(
        &mut r,
        "scene.object_from_selection",
        json!({ "name": "Lid" }),
    );
    let doc = r.document();
    let lid = doc.objects.by_name("Lid").unwrap();
    assert_eq!(lid.transform.translation, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(lid.mesh.as_ref().unwrap().faces.len(), 6);
    assert!(
        doc.objects
            .by_name("Crate")
            .unwrap()
            .mesh
            .as_ref()
            .unwrap()
            .faces
            .is_empty()
    );
    // The geometry did not move in the world.
    let (lo, hi) = doc.bounds([lid]).unwrap();
    assert!(((lo + hi) * 0.5).distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);
}

#[test]
fn instances_follow_the_source() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Crate" }),
    );
    for x in 1..=5 {
        run(
            &mut r,
            "scene.instance_add",
            json!({ "source": "Crate", "position": [x * 2, 0, 0] }),
        );
    }
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "face", "object": "Crate" }),
    );
    run(
        &mut r,
        "select.by_normal",
        json!({ "direction": [0, 1, 0] }),
    );
    run(&mut r, "mesh.extrude", json!({ "distance": 1 }));
    let doc = r.document();
    let instances: Vec<_> = doc
        .objects
        .iter()
        .filter(|o| o.instance_of.is_some())
        .collect();
    assert_eq!(instances.len(), 5);
    for i in instances {
        assert_eq!(
            doc.drawn_mesh(i).unwrap().faces.len(),
            10,
            "{} shows the edit",
            i.name
        );
    }
    let err = r
        .execute(
            "scene.delete",
            json!({ "objects": ["Crate"] }),
            Source::Agent,
        )
        .unwrap_err();
    assert_eq!(
        err.code,
        ErrorCode::Conflict,
        "a source with instances is not deleted silently"
    );
}

#[test]
fn markers_and_collision_carry_their_data() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "scene.marker_add",
        json!({ "name": "spawn_player", "type": "spawn", "props": { "team": 1 } }),
    );
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "plane", "name": "Floor" }),
    );
    run(
        &mut r,
        "scene.set_collision",
        json!({ "objects": ["Floor"], "collision": true }),
    );
    run(
        &mut r,
        "scene.set_props",
        json!({ "object": "spawn_player", "set": { "label": "start" } }),
    );
    let doc = r.document();
    let m = doc.objects.by_name("spawn_player").unwrap();
    assert_eq!(m.properties.len(), 2);
    assert!(doc.objects.by_name("Floor").unwrap().collision);
    let text = serde_json::to_string(doc).unwrap();
    let back: quarry_core::Document = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back.objects.by_name("spawn_player").unwrap().properties,
        m.properties,
        "survives save and reopen"
    );
}

#[test]
fn origin_to_cursor_keeps_the_geometry_still() {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "A", "position": [2, 0, 0] }),
    );
    let before = r.document().bounds(r.document().objects.iter()).unwrap();
    run(&mut r, "view.cursor", json!({ "position": [0, 0, 0] }));
    run(&mut r, "scene.origin_to_cursor", json!({ "object": "A" }));
    assert_eq!(world(&r, "A"), Vec3::ZERO);
    let after = r.document().bounds(r.document().objects.iter()).unwrap();
    assert!(before.0.distance(after.0) < 1e-5 && before.1.distance(after.1) < 1e-5);
}
