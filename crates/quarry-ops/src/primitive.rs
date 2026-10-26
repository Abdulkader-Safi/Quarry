//! Primitive shapes as plain positions and quads (FR-MESH-01).

use glam::Vec3;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuboid_faces_point_outward() {
        let (p, quads) = cuboid(Vec3::new(2.0, 1.0, 4.0));
        for q in quads {
            let [a, b, c, d] = q.map(|i| p[i as usize]);
            let normal = (b - a).cross(c - a).normalize();
            let centre = (a + b + c + d) / 4.0;
            assert!(normal.dot(centre) > 0.0, "quad {q:?} faces inward");
            assert!(
                (c - b).cross(d - b).dot(normal) > 0.0,
                "quad {q:?} is not planar and convex"
            );
        }
    }
}
