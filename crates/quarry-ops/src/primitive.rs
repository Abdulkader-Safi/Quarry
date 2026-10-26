//! Primitive shapes (FR-MESH-01): positions plus faces of 3 or 4 corners, each
//! corner with a UV in 0..1 (u right, v down, as image rows). Faces wind
//! counter-clockwise seen from outside. Centred on the origin, Y up.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

pub struct Shape {
    pub positions: Vec<Vec3>,
    /// Corner vertex indices and their UVs.
    pub faces: Vec<(Vec<u32>, Vec<Vec2>)>,
}

/// The full tile on a quad, matching corner order bottom-left, bottom-right,
/// top-right, top-left.
const QUAD_UV: [Vec2; 4] = [
    Vec2::new(0.0, 1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(0.0, 0.0),
];

/// An axis-aligned box centred on the origin: 8 corners and 6 outward-facing
/// quads, counter-clockwise seen from outside.
pub fn cuboid(size: Vec3) -> (Vec<Vec3>, Vec<[u32; 4]>) {
    let h = size * 0.5;
    let positions = (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { -h.x } else { h.x },
                if i & 2 == 0 { -h.y } else { h.y },
                if i & 4 == 0 { -h.z } else { h.z },
            )
        })
        .collect();
    let quads = vec![
        [1, 3, 7, 5], // +X
        [0, 4, 6, 2], // -X
        [2, 6, 7, 3], // +Y
        [0, 1, 5, 4], // -Y
        [4, 5, 7, 6], // +Z
        [0, 2, 3, 1], // -Z
    ];
    (positions, quads)
}

pub fn cube(size: Vec3) -> Shape {
    let (positions, quads) = cuboid(size);
    Shape {
        positions,
        faces: quads
            .into_iter()
            .map(|q| (q.to_vec(), QUAD_UV.to_vec()))
            .collect(),
    }
}

/// A flat square on XZ facing +Y.
pub fn plane(size: Vec3) -> Shape {
    let (x, z) = (size.x * 0.5, size.z * 0.5);
    Shape {
        positions: vec![
            Vec3::new(-x, 0.0, z),
            Vec3::new(x, 0.0, z),
            Vec3::new(x, 0.0, -z),
            Vec3::new(-x, 0.0, -z),
        ],
        faces: vec![(vec![0, 1, 2, 3], QUAD_UV.to_vec())],
    }
}

/// A ramp: full height at -Z, sloping down to +Z.
pub fn wedge(size: Vec3) -> Shape {
    let h = size * 0.5;
    let positions = vec![
        Vec3::new(-h.x, -h.y, -h.z), // 0 back bottom left
        Vec3::new(h.x, -h.y, -h.z),  // 1 back bottom right
        Vec3::new(h.x, -h.y, h.z),   // 2 front bottom right
        Vec3::new(-h.x, -h.y, h.z),  // 3 front bottom left
        Vec3::new(-h.x, h.y, -h.z),  // 4 back top left
        Vec3::new(h.x, h.y, -h.z),   // 5 back top right
    ];
    let tri_uv = vec![
        Vec2::new(0.0, 1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 0.0),
    ];
    Shape {
        positions,
        faces: vec![
            (
                vec![0, 1, 2, 3],
                vec![QUAD_UV[3], QUAD_UV[2], QUAD_UV[1], QUAD_UV[0]],
            ), // bottom
            (vec![1, 0, 4, 5], QUAD_UV.to_vec()), // back
            (vec![3, 2, 5, 4], QUAD_UV.to_vec()), // slope
            (vec![0, 3, 4], tri_uv.clone()),      // left
            (vec![2, 1, 5], tri_uv),              // right
        ],
    }
}

/// A cylinder (or cone when `top_radius` is 0) along Y with `segments` sides
/// and flat caps. Side UVs run once around the tile.
fn tube(size: Vec3, segments: u32, top_scale: f32) -> Shape {
    let n = segments.max(3);
    let (rx, rz, hy) = (size.x * 0.5, size.z * 0.5, size.y * 0.5);
    let ring = |y: f32, scale: f32| {
        (0..n).map(move |i| {
            let a = i as f32 / n as f32 * TAU;
            Vec3::new(a.cos() * rx * scale, y, -a.sin() * rz * scale)
        })
    };
    let mut positions: Vec<Vec3> = ring(-hy, 1.0).collect();
    let cone = top_scale == 0.0;
    if cone {
        positions.push(Vec3::new(0.0, hy, 0.0));
    } else {
        positions.extend(ring(hy, top_scale));
    }
    let apex = n;
    let mut faces = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let (u0, u1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        if cone {
            faces.push((
                vec![i, j, apex],
                vec![
                    Vec2::new(u0, 1.0),
                    Vec2::new(u1, 1.0),
                    Vec2::new((u0 + u1) * 0.5, 0.0),
                ],
            ));
        } else {
            faces.push((
                vec![i, j, n + j, n + i],
                vec![
                    Vec2::new(u0, 1.0),
                    Vec2::new(u1, 1.0),
                    Vec2::new(u1, 0.0),
                    Vec2::new(u0, 0.0),
                ],
            ));
        }
    }
    // Caps: planar UVs from the ring's XZ.
    let cap_uv = |k: u32| {
        let a = k as f32 / n as f32 * TAU;
        Vec2::new(0.5 + a.cos() * 0.5, 0.5 + a.sin() * 0.5)
    };
    faces.push(((0..n).rev().collect(), (0..n).rev().map(cap_uv).collect()));
    if !cone {
        faces.push(((n..2 * n).collect(), (0..n).map(cap_uv).collect()));
    }
    triangulate_big_faces(Shape { positions, faces })
}

pub fn cylinder(size: Vec3, segments: u32) -> Shape {
    tube(size, segments, 1.0)
}

pub fn cone(size: Vec3, segments: u32) -> Shape {
    tube(size, segments, 0.0)
}

/// A UV sphere: `segments` around, `segments / 2` rings, triangles at the poles.
pub fn sphere(size: Vec3, segments: u32) -> Shape {
    let n = segments.max(3);
    let rings = (n / 2).max(2);
    let h = size * 0.5;
    let mut positions = vec![Vec3::new(0.0, -h.y, 0.0)];
    for r in 1..rings {
        let lat = -std::f32::consts::FRAC_PI_2 + r as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU;
            positions.push(Vec3::new(
                a.cos() * lat.cos() * h.x,
                lat.sin() * h.y,
                -a.sin() * lat.cos() * h.z,
            ));
        }
    }
    positions.push(Vec3::new(0.0, h.y, 0.0));
    let top = positions.len() as u32 - 1;
    let at = |r: u32, i: u32| 1 + (r - 1) * n + (i % n);
    let uv = |r: u32, i: u32| Vec2::new(i as f32 / n as f32, 1.0 - r as f32 / rings as f32);
    let mut faces = Vec::new();
    for i in 0..n {
        faces.push((
            vec![0, at(1, i + 1), at(1, i)],
            vec![uv(0, i), uv(1, i + 1), uv(1, i)],
        ));
        for r in 1..rings - 1 {
            faces.push((
                vec![at(r, i), at(r, i + 1), at(r + 1, i + 1), at(r + 1, i)],
                vec![uv(r, i), uv(r, i + 1), uv(r + 1, i + 1), uv(r + 1, i)],
            ));
        }
        faces.push((
            vec![at(rings - 1, i), at(rings - 1, i + 1), top],
            vec![uv(rings - 1, i), uv(rings - 1, i + 1), uv(rings, i)],
        ));
    }
    Shape { positions, faces }
}

/// Faces may only have 3 or 4 corners: fan larger caps into triangles.
fn triangulate_big_faces(shape: Shape) -> Shape {
    let mut faces = Vec::new();
    for (corners, uvs) in shape.faces {
        if corners.len() <= 4 {
            faces.push((corners, uvs));
            continue;
        }
        for k in 1..corners.len() - 1 {
            faces.push((
                vec![corners[0], corners[k], corners[k + 1]],
                vec![uvs[0], uvs[k], uvs[k + 1]],
            ));
        }
    }
    Shape {
        positions: shape.positions,
        faces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faces point away from the centre and are not degenerate.
    fn assert_outward(shape: &Shape, name: &str) {
        let inside = shape.positions.iter().sum::<Vec3>() / shape.positions.len() as f32;
        for (corners, uvs) in &shape.faces {
            assert!(
                corners.len() == 3 || corners.len() == 4,
                "{name}: face with {} corners",
                corners.len()
            );
            assert_eq!(corners.len(), uvs.len());
            let p: Vec<Vec3> = corners
                .iter()
                .map(|&c| shape.positions[c as usize])
                .collect();
            let mut normal = Vec3::ZERO;
            for i in 0..p.len() {
                normal += p[i].cross(p[(i + 1) % p.len()]);
            }
            let centre = p.iter().sum::<Vec3>() / p.len() as f32;
            assert!(normal.length() > 1e-6, "{name}: degenerate face");
            assert!(
                normal.dot(centre - inside) > 0.0,
                "{name}: face {corners:?} points inward"
            );
            assert!(uvs.iter().all(|uv| uv.is_finite()
                && (0.0..=1.0).contains(&uv.x)
                && (0.0..=1.0).contains(&uv.y)));
        }
    }

    #[test]
    fn cuboid_faces_point_outward() {
        assert_outward(&cube(Vec3::new(2.0, 1.0, 4.0)), "box");
    }

    #[test]
    fn every_primitive_faces_out_with_sane_uvs() {
        let s = Vec3::ONE;
        assert_outward(&wedge(s), "wedge");
        assert_outward(&cylinder(s, 8), "cylinder");
        assert_outward(&cone(s, 8), "cone");
        assert_outward(&sphere(s, 12), "sphere");
        let p = plane(s);
        let (c, _) = &p.faces[0];
        let n = (p.positions[c[1] as usize] - p.positions[c[0] as usize])
            .cross(p.positions[c[2] as usize] - p.positions[c[0] as usize]);
        assert!(n.y > 0.0, "plane faces up");
    }

    /// FR-MESH-01: an 8-segment cylinder has 8 side quads and two caps.
    #[test]
    fn eight_segment_cylinder() {
        let c = cylinder(Vec3::ONE, 8);
        let quads = c.faces.iter().filter(|(f, _)| f.len() == 4).count();
        assert_eq!(quads, 8, "8 side quads (caps are fanned into triangles)");
        let cap_tris = c.faces.iter().filter(|(f, _)| f.len() == 3).count();
        assert_eq!(cap_tris, 2 * 6, "two 8-gon caps, 6 triangles each");
        assert_eq!(c.positions.len(), 16);
    }
}
