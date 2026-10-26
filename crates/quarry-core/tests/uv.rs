//! M3 UVs: rotate, seams and unwrap, projection, pixel snapping.

use glam::Vec2;
use quarry_core::model::Mesh;
use quarry_core::{CommandRegistry, Source};
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

fn mesh(r: &CommandRegistry) -> &Mesh {
    r.document()
        .objects
        .by_name("M")
        .unwrap()
        .mesh
        .as_deref()
        .unwrap()
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

fn polygon(mesh: &Mesh, f: usize) -> Vec<Vec2> {
    mesh.faces[f].corners.iter().map(|c| c.uv).collect()
}

fn area(p: &[Vec2]) -> f32 {
    (0..p.len())
        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
        .sum::<f32>()
        .abs()
        * 0.5
}

/// Convex polygons overlap in their interiors (touching edges do not count).
fn overlap(a: &[Vec2], b: &[Vec2]) -> bool {
    for poly in [a, b] {
        for i in 0..poly.len() {
            let axis = (poly[(i + 1) % poly.len()] - poly[i]).perp();
            let span = |p: &[Vec2]| {
                p.iter()
                    .map(|v| v.dot(axis))
                    .fold((f32::MAX, f32::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)))
            };
            let ((a0, a1), (b0, b1)) = (span(a), span(b));
            let eps = 1e-3 * axis.length();
            if a1 <= b0 + eps || b1 <= a0 + eps {
                return false;
            }
        }
    }
    true
}

#[test]
fn rotating_uvs_ninety_rotates_the_texture() {
    let mut r = editing("plane");
    let before = polygon(mesh(&r), 0);
    run(&mut r, "uv.transform", json!({ "rotate": 90 }));
    let after = polygon(mesh(&r), 0);
    let centre = Vec2::splat(8.0);
    for (b, a) in before.iter().zip(&after) {
        let expected = centre + (*b - centre).rotate(Vec2::from_angle(-90f32.to_radians()));
        assert!(a.distance(expected) < 1e-4, "{a} vs {expected}");
    }
}

#[test]
fn a_box_with_seven_seams_unwraps_into_six_flat_faces() {
    let mut r = editing("box");
    // Cut all 12 edges except a cross: the front's 4 edges and top-to-back.
    let hinges = [(4, 5), (5, 7), (6, 7), (4, 6), (2, 3)];
    let all: Vec<[u32; 2]> = {
        let m = mesh(&r);
        let mut e = std::collections::BTreeSet::new();
        for f in &m.faces {
            for k in 0..4 {
                let (a, b) = (f.corners[k].vertex, f.corners[(k + 1) % 4].vertex);
                e.insert((a.min(b), a.max(b)));
            }
        }
        e.into_iter()
            .filter(|e| !hinges.contains(e))
            .map(|(a, b)| [a, b])
            .collect()
    };
    assert_eq!(all.len(), 7);
    run(&mut r, "select.mode", json!({ "mode": "edge" }));
    run(&mut r, "select.by_ids", json!({ "edges": all }));
    run(&mut r, "uv.mark_seam", json!({}));
    run(&mut r, "select.mode", json!({ "mode": "face" }));
    run(&mut r, "select.none", json!({}));
    let reply = run(&mut r, "uv.unwrap", json!({}));
    assert_eq!(reply["islands"], 1, "one connected piece");

    let m = mesh(&r);
    for f in 0..6 {
        assert!(
            (area(&polygon(m, f)) - 256.0).abs() < 1e-2,
            "each 1 m face is one 16 px tile"
        );
        assert!(
            polygon(m, f)
                .iter()
                .all(|uv| uv.x >= -1e-4 && uv.y >= -1e-4)
        );
        for g in f + 1..6 {
            assert!(
                !overlap(&polygon(m, f), &polygon(m, g)),
                "faces {f} and {g} overlap"
            );
        }
    }
}

#[test]
fn unwrapping_without_seams_still_never_overlaps() {
    let mut r = editing("box");
    run(&mut r, "uv.unwrap", json!({}));
    let m = mesh(&r);
    for f in 0..6 {
        for g in f + 1..6 {
            assert!(
                !overlap(&polygon(m, f), &polygon(m, g)),
                "faces {f} and {g} overlap"
            );
        }
    }
}

#[test]
fn snapping_leaves_whole_pixels_and_projection_matches_the_view() {
    let mut r = editing("plane");
    run(&mut r, "uv.transform", json!({ "translate": [0.3, 0.6] }));
    run(&mut r, "uv.snap_pixels", json!({}));
    assert!(
        polygon(mesh(&r), 0)
            .iter()
            .all(|uv| uv.fract() == Vec2::ZERO)
    );

    run(&mut r, "view.camera", json!({ "preset": "top" }));
    run(&mut r, "uv.project_view", json!({}));
    let p = polygon(mesh(&r), 0);
    assert!(
        (area(&p) - 256.0).abs() < 1e-2,
        "a 1 m plane seen from above covers one tile"
    );
}

/// Signed area in image space; the sign says whether the texture is mirrored.
fn winding(p: &[Vec2]) -> f32 {
    (0..p.len())
        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
        .sum()
}

#[test]
fn unwrapped_textures_are_not_mirrored() {
    let mut r = editing("box");
    let before: Vec<f32> = (0..6).map(|f| winding(&polygon(mesh(&r), f))).collect();
    run(&mut r, "uv.unwrap", json!({}));
    for (f, b) in before.iter().enumerate() {
        let a = winding(&polygon(mesh(&r), f));
        assert_eq!(a.signum(), b.signum(), "face {f} is mirrored after unwrap");
    }
}
