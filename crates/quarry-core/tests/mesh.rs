//! M3 mesh edits through the registry.

use glam::Vec3;
use quarry_core::model::{Document, Mesh};
use quarry_core::{CommandRegistry, ErrorCode, Source};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

fn mesh<'a>(doc: &'a Document, name: &str) -> &'a Mesh {
    doc.objects.by_name(name).unwrap().mesh.as_deref().unwrap()
}

fn editing(kind: &str) -> CommandRegistry {
    let mut r = CommandRegistry::new();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": kind, "name": "M" }),
    );
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "face", "object": "M" }),
    );
    r
}

#[test]
fn typed_move_raises_the_selection_centre_exactly() {
    let mut r = editing("box");
    run(
        &mut r,
        "select.by_normal",
        json!({ "direction": [0, 1, 0] }),
    );
    let before = run(&mut r, "query.selection", json!({}))["bounds"].clone();
    run(
        &mut r,
        "mesh.transform",
        json!({ "translate": [0, 0.5, 0] }),
    );
    let after = run(&mut r, "query.selection", json!({}))["bounds"].clone();
    assert_eq!(
        after[0][1].as_f64().unwrap() - before[0][1].as_f64().unwrap(),
        0.5
    );

    run(&mut r, "select.mode", json!({ "mode": "object" }));
    run(&mut r, "mesh.transform", json!({ "rotate": [0, 45, 0] }));
    let o = r.document().objects.by_name("M").unwrap();
    let (axis, angle) = o.transform.rotation.to_axis_angle();
    assert!((angle.to_degrees() - 45.0).abs() < 1e-3 && axis.distance(Vec3::Y) < 1e-4);
}

#[test]
fn extruding_the_top_face_adds_four_sides_and_lifts_it() {
    let mut r = editing("box");
    run(
        &mut r,
        "select.by_normal",
        json!({ "direction": [0, 1, 0] }),
    );
    run(&mut r, "mesh.extrude", json!({ "distance": 1.0 }));
    let m = mesh(r.document(), "M");
    assert_eq!(
        m.faces.len(),
        10,
        "6 faces: the top moves up and 4 sides join it"
    );
    assert_eq!(m.positions.len(), 12, "4 new corners, no doubles");
    let top = r.document().selection.faces.iter().next().copied().unwrap() as usize;
    for c in &m.faces[top].corners {
        assert_eq!(
            m.positions[c.vertex as usize].y, 1.5,
            "the new top sits 1 higher"
        );
    }
    // Every face still points away from the middle of the now taller box.
    let centre = Vec3::new(0.0, 0.5, 0.0);
    for f in &m.faces {
        let p: Vec<Vec3> = f
            .corners
            .iter()
            .map(|c| m.positions[c.vertex as usize])
            .collect();
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let mid = p.iter().sum::<Vec3>() / p.len() as f32;
        assert!(n.dot(mid - centre) > 0.0, "no inverted normals");
    }
    run(&mut r, "history.undo", json!({}));
    assert_eq!(
        mesh(r.document(), "M").faces.len(),
        6,
        "undo restores the exact topology"
    );
}

#[test]
fn extruding_edges_shares_new_vertices() {
    let mut r = editing("plane");
    run(&mut r, "select.mode", json!({ "mode": "edge" }));
    run(&mut r, "select.all", json!({}));
    run(
        &mut r,
        "mesh.extrude",
        json!({ "distance": 1.0, "direction": [0, 1, 0] }),
    );
    let m = mesh(r.document(), "M");
    assert_eq!(m.faces.len(), 5);
    assert_eq!(m.positions.len(), 8, "each corner extrudes once");
}

#[test]
fn merge_by_distance_welds_coincident_vertices() {
    let mut r = editing("box");
    run(
        &mut r,
        "select.by_normal",
        json!({ "direction": [0, 1, 0] }),
    );
    run(&mut r, "mesh.split", json!({}));
    assert_eq!(mesh(r.document(), "M").positions.len(), 12);
    run(&mut r, "select.none", json!({}));
    let reply = run(
        &mut r,
        "mesh.merge",
        json!({ "at": "distance", "distance": 0.001 }),
    );
    assert_eq!(reply["removed"], 4);
    assert_eq!(mesh(r.document(), "M").positions.len(), 8);
}

#[test]
fn make_face_from_four_vertices_faces_the_camera() {
    let mut r = editing("plane");
    run(&mut r, "select.all", json!({}));
    run(&mut r, "mesh.delete", json!({ "what": "only_faces" }));
    assert_eq!(mesh(r.document(), "M").faces.len(), 0);
    run(&mut r, "view.camera", json!({ "preset": "top" }));
    run(&mut r, "select.mode", json!({ "mode": "vertex" }));
    run(&mut r, "select.all", json!({}));
    run(&mut r, "mesh.make_face", json!({}));
    let m = mesh(r.document(), "M");
    assert_eq!(m.faces.len(), 1);
    let p: Vec<Vec3> = m.faces[0]
        .corners
        .iter()
        .map(|c| m.positions[c.vertex as usize])
        .collect();
    assert!(
        (p[1] - p[0]).cross(p[2] - p[0]).y > 0.0,
        "faces the top camera"
    );
    let err = r
        .execute("mesh.make_face", json!({}), Source::Human)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
}

#[test]
fn flip_diagonal_keeps_the_outline_and_flip_normals_turns_faces() {
    let mut r = editing("plane");
    let before: Vec<u32> = mesh(r.document(), "M").faces[0]
        .corners
        .iter()
        .map(|c| c.vertex)
        .collect();
    run(&mut r, "mesh.flip_diagonal", json!({}));
    let after: Vec<u32> = mesh(r.document(), "M").faces[0]
        .corners
        .iter()
        .map(|c| c.vertex)
        .collect();
    assert_ne!(before[0], after[0], "the fan starts at another corner");
    assert_eq!(
        before.iter().collect::<BTreeSet<_>>(),
        after.iter().collect::<BTreeSet<_>>()
    );
    run(&mut r, "mesh.flip_normals", json!({}));
    let m = mesh(r.document(), "M");
    let p: Vec<Vec3> = m.faces[0]
        .corners
        .iter()
        .map(|c| m.positions[c.vertex as usize])
        .collect();
    assert!((p[1] - p[0]).cross(p[2] - p[0]).y < 0.0, "now faces down");
}

#[test]
fn delete_vertices_takes_their_faces() {
    let mut r = editing("box");
    run(&mut r, "select.mode", json!({ "mode": "vertex" }));
    run(&mut r, "select.by_ids", json!({ "vertices": [7] }));
    let reply = run(&mut r, "mesh.delete", json!({ "what": "vertices" }));
    assert_eq!(reply["vertices_removed"], 1);
    assert_eq!(
        reply["faces_removed"], 3,
        "a box corner touches three faces"
    );
}

#[test]
fn live_mirror_moves_the_twin_and_apply_welds_the_centre() {
    let mut r = editing("box");
    run(&mut r, "mesh.mirror_mode", json!({ "axis": "x" }));
    run(&mut r, "select.mode", json!({ "mode": "vertex" }));
    let m = mesh(r.document(), "M");
    let plus = (0..8u32)
        .find(|v| m.positions[*v as usize].x > 0.0)
        .unwrap();
    let twin_pos = m.positions[plus as usize] * Vec3::new(-1.0, 1.0, 1.0);
    let twin = m.positions.iter().position(|p| *p == twin_pos).unwrap();
    run(&mut r, "select.by_ids", json!({ "vertices": [plus] }));
    run(
        &mut r,
        "mesh.transform",
        json!({ "translate": [0.25, 1, 0] }),
    );
    let m = mesh(r.document(), "M");
    let moved = m.positions[plus as usize];
    assert_eq!(
        m.positions[twin],
        moved * Vec3::new(-1.0, 1.0, 1.0),
        "the -X twin matches"
    );

    // Fold the -X side onto the axis, then bake the mirror back out.
    run(&mut r, "select.none", json!({}));
    let left: Vec<u32> = (0..8u32)
        .filter(|v| mesh(r.document(), "M").positions[*v as usize].x < 0.0)
        .collect();
    run(&mut r, "mesh.mirror_mode", json!({ "axis": "off" }));
    run(&mut r, "select.by_ids", json!({ "vertices": left }));
    run(&mut r, "view.cursor", json!({ "position": [0, 0, 0] }));
    run(
        &mut r,
        "mesh.transform",
        json!({ "scale": [0, 1, 1], "pivot": "cursor" }),
    );
    run(&mut r, "mesh.mirror_mode", json!({ "axis": "x" }));
    run(&mut r, "mesh.mirror_apply", json!({}));
    let m = mesh(r.document(), "M");
    assert_eq!(
        m.positions.len(),
        12,
        "8 + 4 mirrored; the 4 centre vertices are shared"
    );
    assert!(m.mirror.is_none());
    let centre: Vec<_> = m.positions.iter().filter(|p| p.x.abs() < 1e-5).collect();
    assert_eq!(centre.len(), 4, "no duplicate centre vertices");
}

#[test]
fn edits_need_a_selection_and_say_so() {
    let mut r = editing("box");
    let err = r
        .execute("mesh.extrude", json!({ "distance": 1 }), Source::Agent)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NothingSelected);
    assert_eq!(err.message, "No faces selected. Call select first.");
}
