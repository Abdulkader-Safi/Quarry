//! M6 rigging through the registry.

use glam::Vec3;
use quarry_core::model::Document;
use quarry_core::{CommandRegistry, ErrorCode, Source};
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

/// World positions of an object's vertices under the current pose (CPU skinning).
fn skinned(doc: &Document, object: &str) -> Vec<Vec3> {
    let o = doc.objects.by_name(object).unwrap();
    let mesh = o.mesh.as_deref().unwrap();
    let world = doc.world_matrix(o);
    let skin = o.skin.as_ref().unwrap();
    let m = doc.skeletons.get(skin.skeleton).unwrap().skinning();
    mesh.positions
        .iter()
        .zip(&mesh.weights)
        .map(|(p, w)| {
            let p = world.transform_point3(*p);
            (0..4).fold(Vec3::ZERO, |acc, k| {
                acc + m[w.bones[k] as usize].transform_point3(p) * w.weights[k]
            })
        })
        .collect()
}

/// A 2 m arm along +X: upper 0..1, lower 1..2, and a box mesh over it.
fn arm() -> CommandRegistry {
    let mut r = CommandRegistry::new();
    run(&mut r, "rig.skeleton_new", json!({ "name": "rig" }));
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "upper.L", "head": [0, 0, 0], "tail": [1, 0, 0] }),
    );
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "lower.L", "parent": "upper.L", "tail": [2, 0, 0] }),
    );
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Arm", "size": [2, 0.2, 0.2], "position": [1, 0, 0] }),
    );
    r
}

#[test]
fn chained_bones_start_at_their_parents_tail() {
    let mut r = CommandRegistry::new();
    run(&mut r, "rig.skeleton_new", json!({}));
    run(&mut r, "view.cursor", json!({ "position": [0, 1, 0] }));
    run(&mut r, "rig.bone_add", json!({ "name": "a" }));
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "b", "parent": "a" }),
    );
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "c", "parent": "b" }),
    );
    let s = r.document().skeletons.iter().next().unwrap();
    assert_eq!(
        s.bones[0].head,
        Vec3::new(0.0, 1.0, 0.0),
        "first bone at the crosshair"
    );
    for i in 1..3 {
        assert_eq!(s.bones[i].head, s.bones[i - 1].tail);
    }
    let err = r
        .execute(
            "rig.bone_edit",
            json!({ "bone": "a", "parent": "c" }),
            Source::Agent,
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict, "no loops");
    let err = r
        .execute("rig.bone_add", json!({ "name": "a" }), Source::Agent)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict, "names are unique");
}

#[test]
fn bending_the_forearm_leaves_the_shoulder_still() {
    let mut r = arm();
    run(&mut r, "rig.bind_auto", json!({ "object": "Arm" }));
    let rest = skinned(r.document(), "Arm");
    run(
        &mut r,
        "rig.pose",
        json!({ "bone": "lower.L", "rotation": [0, 0, 45] }),
    );
    let posed = skinned(r.document(), "Arm");
    for (a, b) in rest.iter().zip(&posed) {
        if a.x < 0.01 {
            assert!(a.distance(*b) < 1e-5, "shoulder vertex moved: {a} -> {b}");
        }
        if a.x > 1.99 {
            assert!(a.distance(*b) > 0.1, "the hand end moves");
        }
    }
    let m = r
        .document()
        .objects
        .by_name("Arm")
        .unwrap()
        .mesh
        .clone()
        .unwrap();
    assert!(
        m.weights
            .iter()
            .all(|w| (w.weights.iter().sum::<f32>() - 1.0).abs() < 1e-6)
    );
}

#[test]
fn five_bones_on_one_vertex_keep_four_summing_to_one() {
    let mut r = CommandRegistry::new();
    run(&mut r, "rig.skeleton_new", json!({}));
    for i in 0..5 {
        run(
            &mut r,
            "rig.bone_add",
            json!({ "name": format!("b{i}"), "head": [i, 0, 0], "tail": [i, 1, 0] }),
        );
    }
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "plane", "name": "P" }),
    );
    run(
        &mut r,
        "rig.bind_rigid",
        json!({ "object": "P", "bone": "b0" }),
    );
    for i in 1..5 {
        run(
            &mut r,
            "rig.weight_paint",
            json!({ "object": "P", "bone": format!("b{i}"), "mode": "add", "value": 0.2, "vertices": [0] }),
        );
    }
    let w = r
        .document()
        .objects
        .by_name("P")
        .unwrap()
        .mesh
        .as_ref()
        .unwrap()
        .weights[0];
    assert_eq!(w.weights.iter().filter(|v| **v > 0.0).count(), 4);
    assert!((w.weights.iter().sum::<f32>() - 1.0).abs() < 1e-6, "{w:?}");

    // Subtract keeps the sum at 1 and lowers the bone.
    let before = w.weights[w.bones.iter().position(|b| *b == 4).unwrap()];
    run(
        &mut r,
        "rig.weight_paint",
        json!({ "object": "P", "bone": "b4", "mode": "subtract", "value": 0.1, "vertices": [0] }),
    );
    let w = r
        .document()
        .objects
        .by_name("P")
        .unwrap()
        .mesh
        .as_ref()
        .unwrap()
        .weights[0];
    let after = w
        .bones
        .iter()
        .position(|b| *b == 4)
        .map_or(0.0, |k| w.weights[k]);
    assert!(after < before && (w.weights.iter().sum::<f32>() - 1.0).abs() < 1e-6);
}

#[test]
fn a_rigid_hand_moves_without_bending() {
    let mut r = arm();
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Hand", "size": [0.3, 0.3, 0.3], "position": [2.2, 0, 0] }),
    );
    run(
        &mut r,
        "rig.bind_rigid",
        json!({ "object": "Hand", "bone": "lower.L" }),
    );
    let rest = skinned(r.document(), "Hand");
    run(
        &mut r,
        "rig.pose",
        json!({ "bone": "lower.L", "rotation": [0, 0, 60] }),
    );
    let posed = skinned(r.document(), "Hand");
    for i in 0..rest.len() {
        for j in 0..rest.len() {
            assert!(
                (rest[i].distance(rest[j]) - posed[i].distance(posed[j])).abs() < 1e-4,
                "shape kept"
            );
        }
    }
    assert!(rest[0].distance(posed[0]) > 0.1);
}

fn legs() -> CommandRegistry {
    let mut r = CommandRegistry::new();
    run(&mut r, "rig.skeleton_new", json!({}));
    for side in ["L", "R"] {
        let x = if side == "L" { 0.2 } else { -0.2 };
        run(
            &mut r,
            "rig.bone_add",
            json!({ "name": format!("thigh.{side}"), "head": [x, 2, 0], "tail": [x, 1, 0] }),
        );
        run(
            &mut r,
            "rig.bone_add",
            json!({ "name": format!("shin.{side}"), "parent": format!("thigh.{side}"), "tail": [x, 0, 0] }),
        );
        run(
            &mut r,
            "rig.ik_add",
            json!({ "tip_bone": format!("shin.{side}"), "pole": [x, 1, 5] }),
        );
    }
    r
}

fn tip(doc: &Document, bone: &str) -> Vec3 {
    let s = doc.skeletons.iter().next().unwrap();
    let i = s.index(bone).unwrap() as usize;
    let b = &s.bones[i];
    s.pose_world()[i].transform_point3(Vec3::Y * b.head.distance(b.tail))
}

#[test]
fn ik_reaches_the_target_and_the_knee_follows_the_pole() {
    let mut r = legs();
    let reply = run(
        &mut r,
        "rig.ik_set",
        json!({ "tip_bone": "shin.L", "target": [0.2, 0.6, 0.6] }),
    );
    assert!(reply["gap"].as_f64().unwrap() < 1e-3, "{reply}");
    let doc = r.document();
    assert!(tip(doc, "shin.L").distance(Vec3::new(0.2, 0.6, 0.6)) < 1e-3);
    let s = doc.skeletons.iter().next().unwrap();
    let knee = s.pose_world()[s.index("shin.L").unwrap() as usize].transform_point3(Vec3::ZERO);
    assert!(knee.z > 0.0, "knee points at the pole");
    assert!(
        tip(doc, "shin.R").distance(Vec3::new(-0.2, 0.0, 0.0)) < 1e-4,
        "the other chain is untouched"
    );

    let reply = run(
        &mut r,
        "rig.ik_set",
        json!({ "tip_bone": "shin.R", "target": [-0.2, 2, 9] }),
    );
    assert!(reply["warning"].is_string(), "out of reach is reported");
    let t = tip(r.document(), "shin.R");
    assert!(
        t.is_finite() && (t.distance(Vec3::new(-0.2, 2.0, 0.0)) - 2.0).abs() < 1e-2,
        "full stretch, not stretched"
    );
}

#[test]
fn mirroring_bones_and_weights_swaps_sides() {
    let mut r = arm();
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "hand.L", "parent": "lower.L", "tail": [2.5, 0, 0] }),
    );
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "spine", "head": [0, 0, 0], "tail": [0, 1, 0] }),
    );
    run(&mut r, "rig.mirror_bones", json!({}));
    let s = r.document().skeletons.iter().next().unwrap().clone();
    for (l, rgt) in [
        ("upper.L", "upper.R"),
        ("lower.L", "lower.R"),
        ("hand.L", "hand.R"),
    ] {
        let (a, b) = (
            &s.bones[s.index(l).unwrap() as usize],
            &s.bones[s.index(rgt).unwrap() as usize],
        );
        assert_eq!(b.head, Vec3::new(-a.head.x, a.head.y, a.head.z));
        if let (Some(pa), Some(pb)) = (a.parent, b.parent) {
            assert_eq!(
                mirror_side(&s.bones[pa as usize].name),
                s.bones[pb as usize].name
            );
        }
    }
    // A symmetric body: weights painted on +X land on the -X twins with .R bones.
    run(
        &mut r,
        "mesh.add_primitive",
        json!({ "kind": "box", "name": "Body", "size": [4, 0.2, 0.2] }),
    );
    run(
        &mut r,
        "rig.bind_rigid",
        json!({ "object": "Body", "bone": "spine" }),
    );
    let right_side: Vec<u32> = {
        let m = r
            .document()
            .objects
            .by_name("Body")
            .unwrap()
            .mesh
            .clone()
            .unwrap();
        (0..m.positions.len() as u32)
            .filter(|v| m.positions[*v as usize].x > 0.0)
            .collect()
    };
    run(
        &mut r,
        "rig.weight_paint",
        json!({ "object": "Body", "bone": "lower.L", "mode": "set", "value": 1, "vertices": right_side }),
    );
    let reply = run(&mut r, "rig.mirror_weights", json!({ "object": "Body" }));
    assert_eq!(reply["vertices"], 4);
    let doc = r.document();
    let s = doc.skeletons.iter().next().unwrap();
    let m = doc.objects.by_name("Body").unwrap().mesh.clone().unwrap();
    for v in 0..m.positions.len() {
        if m.positions[v].x < 0.0 {
            assert_eq!(m.weights[v].bones[0], s.index("lower.R").unwrap());
            assert_eq!(m.weights[v].weights[0], 1.0);
        }
    }
}

fn mirror_side(name: &str) -> String {
    name.replace(".L", ".R")
}

#[test]
fn deleting_a_bone_keeps_the_rig_valid_and_undo_unbinds_in_one_pass() {
    let mut r = arm();
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "hand.L", "parent": "lower.L", "tail": [2.5, 0, 0] }),
    );
    let entries = r.history().entries().len();
    run(&mut r, "rig.bind_auto", json!({ "object": "Arm" }));
    run(&mut r, "rig.bone_delete", json!({ "bone": "lower.L" }));
    let doc = r.document();
    let s = doc.skeletons.iter().next().unwrap();
    assert_eq!(s.bones.len(), 2);
    assert_eq!(
        s.bones[s.index("hand.L").unwrap() as usize].parent,
        s.index("upper.L"),
        "child moves to the grandparent"
    );
    let m = doc.objects.by_name("Arm").unwrap().mesh.clone().unwrap();
    assert!(
        m.weights
            .iter()
            .all(|w| w.bones.iter().all(|b| (*b as usize) < s.bones.len()))
    );
    assert!(
        m.weights
            .iter()
            .all(|w| (w.weights.iter().sum::<f32>() - 1.0).abs() < 1e-6)
    );

    while r.history().entries().len() > entries {
        run(&mut r, "history.undo", json!({}));
    }
    let arm = r.document().objects.by_name("Arm").unwrap();
    assert!(
        arm.skin.is_none() && arm.mesh.as_ref().unwrap().weights.is_empty(),
        "back to an unweighted mesh"
    );
}
