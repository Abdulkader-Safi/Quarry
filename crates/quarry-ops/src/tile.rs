//! Tile placement geometry (FR-TILE-02 to 05, 09): the placement plane, cell
//! corners, how a brush stamp is laid out, and each tile's UV corners.
//!
//! A cell is 1 m (one tile). Cell (0, 0) has its first corner at the plane
//! origin (the crosshair); `u` runs along the plane's first axis, `v` "up".

use glam::{Quat, Vec2, Vec3};

/// The plane's axes: `u` and `v` span it, `n` is the face normal (u × v).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub n: Vec3,
}

/// Plane locked to an axis (0 = X, 1 = Y floor, 2 = Z), tilted about `u`.
pub fn frame(origin: Vec3, axis: usize, tilt_degrees: f32) -> Frame {
    let (u, v) = match axis {
        0 => (Vec3::NEG_Z, Vec3::Y),
        1 => (Vec3::X, Vec3::NEG_Z),
        _ => (Vec3::X, Vec3::Y),
    };
    let tilt = Quat::from_axis_angle(u, tilt_degrees.to_radians());
    let v = tilt * v;
    Frame {
        origin,
        u,
        v,
        n: u.cross(v).normalize(),
    }
}

impl Frame {
    /// The four corners of cell (i, j), counter-clockwise seen from `n`.
    pub fn cell(&self, i: i32, j: i32) -> [Vec3; 4] {
        let at = |a: f32, b: f32| self.origin + self.u * (i as f32 + a) + self.v * (j as f32 + b);
        [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)]
    }
}

/// Brush orientation: quarter turns counter-clockwise, then mirror (left-right)
/// and flip (top-bottom) applied first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Orient {
    pub rotate: u8,
    pub mirror: bool,
    pub flip: bool,
}

/// Pixel rect (x0, y0, x1, y1) of tile (col, row) in a tileset, y down.
pub fn tile_rect(col: u32, row: u32, tile_px: u32, padding: u32) -> [f32; 4] {
    let x = (padding + col * (tile_px + padding)) as f32;
    let y = (padding + row * (tile_px + padding)) as f32;
    [x, y, x + tile_px as f32, y + tile_px as f32]
}

/// Columns and rows of whole tiles in an image.
pub fn grid(width: u32, height: u32, tile_px: u32, padding: u32) -> (u32, u32) {
    let step = tile_px + padding;
    let count = |side: u32| side.saturating_sub(padding) / step.max(1);
    (count(width), count(height))
}

/// UVs (pixels) for the four cell corners, in [`Frame::cell`] order.
pub fn tile_uvs(rect: [f32; 4], orient: Orient) -> [Vec2; 4] {
    let [x0, y0, x1, y1] = rect;
    // Unrotated: cell bottom-left shows the tile's bottom-left (image y is down).
    let mut corners = [
        Vec2::new(x0, y1),
        Vec2::new(x1, y1),
        Vec2::new(x1, y0),
        Vec2::new(x0, y0),
    ];
    if orient.mirror {
        corners = [corners[1], corners[0], corners[3], corners[2]];
    }
    if orient.flip {
        corners = [corners[3], corners[2], corners[1], corners[0]];
    }
    // Turning the picture CCW on the cell: each corner takes the UV of the
    // corner after it.
    corners.rotate_left((orient.rotate % 4) as usize);
    corners
}

/// The orientation that produced `uvs` from `rect`, if any (for the eyedropper).
pub fn orient_of(rect: [f32; 4], uvs: [Vec2; 4]) -> Option<Orient> {
    (0..16u8)
        .map(|k| Orient {
            rotate: k % 4,
            mirror: k & 4 != 0,
            flip: k & 8 != 0,
        })
        .find(|o| {
            tile_uvs(rect, *o)
                .iter()
                .zip(&uvs)
                .all(|(a, b)| a.distance(*b) < 1e-3)
        })
}

/// Where each tile of a `w` x `h` brush lands, as (cell offset, tile offset).
/// Tile offsets count rows down the image; cell offsets count `v` up the plane.
pub fn stamp(w: u32, h: u32, orient: Orient) -> Vec<((i32, i32), (u32, u32))> {
    let mut out = Vec::new();
    for ty in 0..h {
        for tx in 0..w {
            let (mut x, mut y) = (tx as i32, (h - 1 - ty) as i32);
            if orient.mirror {
                x = w as i32 - 1 - x;
            }
            if orient.flip {
                y = h as i32 - 1 - y;
            }
            let (mut cw, mut ch) = (w as i32, h as i32);
            for _ in 0..orient.rotate % 4 {
                // CCW quarter turn inside a cw x ch box.
                (x, y) = (ch - 1 - y, x);
                (cw, ch) = (ch, cw);
            }
            out.push(((x, y), (tx, ty)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planes_face_their_axis() {
        for (axis, normal) in [(0, Vec3::X), (1, Vec3::Y), (2, Vec3::Z)] {
            let f = frame(Vec3::ZERO, axis, 0.0);
            assert!(f.n.distance(normal) < 1e-6, "axis {axis}: {}", f.n);
            let c = f.cell(2, -1);
            let face_normal = (c[1] - c[0]).cross(c[2] - c[0]).normalize();
            assert!(
                face_normal.distance(normal) < 1e-6,
                "corners are CCW from the normal"
            );
        }
    }

    #[test]
    fn tilted_cells_share_edges() {
        let f = frame(Vec3::ZERO, 1, 45.0);
        let (a, b) = (f.cell(0, 0), f.cell(0, 1));
        assert!(a[3].distance(b[0]) < 1e-6 && a[2].distance(b[1]) < 1e-6);
        assert!((f.n.y - 45f32.to_radians().cos()).abs() < 1e-5);
    }

    #[test]
    fn grid_counts_whole_tiles() {
        assert_eq!(grid(256, 256, 16, 0), (16, 16));
        assert_eq!(grid(52, 35, 16, 1), (3, 2));
        assert_eq!(tile_rect(1, 1, 16, 1), [18.0, 18.0, 34.0, 34.0]);
    }

    #[test]
    fn rotating_a_quarter_turn_rotates_uvs_and_round_trips() {
        let rect = tile_rect(3, 2, 16, 0);
        let plain = tile_uvs(rect, Orient::default());
        let turned = tile_uvs(
            rect,
            Orient {
                rotate: 1,
                ..Orient::default()
            },
        );
        assert_eq!(
            turned[0], plain[1],
            "cell bottom-left now shows the tile's bottom-right"
        );
        for k in 0..16u8 {
            let o = Orient {
                rotate: k % 4,
                mirror: k & 4 != 0,
                flip: k & 8 != 0,
            };
            let found = orient_of(rect, tile_uvs(rect, o)).unwrap();
            assert_eq!(tile_uvs(rect, found), tile_uvs(rect, o));
        }
    }

    #[test]
    fn stamp_keeps_a_2x2_brush_together() {
        let cells: Vec<_> = stamp(2, 2, Orient::default()).into_iter().collect();
        // Top-left tile of the brush sits top-left on the plane.
        assert!(cells.contains(&((0, 1), (0, 0))));
        assert!(cells.contains(&((1, 0), (1, 1))));
        let turned = stamp(
            3,
            1,
            Orient {
                rotate: 1,
                ..Orient::default()
            },
        );
        let mut offsets: Vec<_> = turned.iter().map(|(c, _)| *c).collect();
        offsets.sort();
        assert_eq!(
            offsets,
            [(0, 0), (0, 1), (0, 2)],
            "a 3x1 brush turned is 1x3"
        );
    }
}
