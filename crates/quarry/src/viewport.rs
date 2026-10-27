//! The 3D view: renders the document with quarry-render (every mesh as it is,
//! posed and textured), orbits, pans and zooms the camera, and turns clicks
//! into rays for picking objects and faces and for placing tiles.

use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};
use gpui_kit::*;
use quarry_core::Document;
use quarry_core::camera::OrbitCamera;
use quarry_core::id::Id;
use quarry_core::model::Object;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// What the last frame was drawn from; redraw only when it changes.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    revision: u64,
    camera: [f32; 8],
    size: (u32, u32),
    display: quarry_core::model::DisplayToggles,
    anim: (f32, bool),
    /// Selection is not part of the revision but changes the tint.
    selection: u64,
    cursor: [f32; 3],
}

pub struct Viewport {
    pub camera: OrbitCamera,
    /// Logical bounds of the view on screen, written during paint.
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    image: Option<Arc<RenderImage>>,
    key: Option<Key>,
    stale: Vec<Arc<RenderImage>>,
    pub drag: Option<Drag>,
}

#[derive(Clone, Copy)]
pub struct Drag {
    pub last: Point<Pixels>,
    pub pan: bool,
}

impl Viewport {
    pub fn new(camera: OrbitCamera) -> Self {
        Self {
            camera,
            bounds: Rc::new(Cell::new(Bounds::default())),
            image: None,
            key: None,
            stale: Vec::new(),
            drag: None,
        }
    }

    /// Redraw if anything the picture depends on changed; returns the image.
    pub fn frame(
        &mut self,
        doc: &Document,
        revision: u64,
        window: &mut Window,
    ) -> Option<Arc<RenderImage>> {
        for old in self.stale.drain(..) {
            let _ = window.drop_image(old);
        }
        let b = self.bounds.get();
        let scale = window.scale_factor();
        let (w, h) = (
            (f32::from(b.size.width) * scale).round() as u32,
            (f32::from(b.size.height) * scale).round() as u32,
        );
        if w < 8 || h < 8 {
            return self.image.clone();
        }
        let c = self.camera;
        let key = Key {
            revision,
            camera: [
                c.target.x,
                c.target.y,
                c.target.z,
                c.distance,
                c.yaw,
                c.pitch,
                c.fov,
                c.ortho as u8 as f32,
            ],
            size: (w, h),
            display: doc.view.display,
            anim: (doc.view.anim.time, doc.view.anim.clip.is_some()),
            selection: {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                format!("{:?}", doc.selection).hash(&mut h);
                h.finish()
            },
            cursor: doc.view.cursor.to_array(),
        };
        if self.key == Some(key) {
            return self.image.clone();
        }
        let rgba = quarry_render::viewport_rgba(doc, &self.camera, w, h)?;
        let mut bgra = image::RgbaImage::from_raw(w, h, rgba)?;
        for p in bgra.pixels_mut() {
            p.0.swap(0, 2);
        }
        let render = Arc::new(RenderImage::new(vec![image::Frame::new(bgra)]));
        if let Some(old) = self.image.replace(render) {
            self.stale.push(old);
        }
        self.key = Some(key);
        self.image.clone()
    }

    pub fn aspect(&self) -> f32 {
        let s = self.bounds.get().size;
        (f32::from(s.width) / f32::from(s.height).max(1.0)).max(0.01)
    }

    /// Mouse position to a world ray (origin, direction).
    pub fn ray(&self, at: Point<Pixels>) -> (Vec3, Vec3) {
        let b = self.bounds.get();
        let x = (f32::from(at.x - b.origin.x) / f32::from(b.size.width)) * 2.0 - 1.0;
        let y = 1.0 - (f32::from(at.y - b.origin.y) / f32::from(b.size.height)) * 2.0;
        let inv = self.camera.view_proj(self.aspect()).inverse();
        let unproject = |z: f32| {
            let p = inv * glam::Vec4::new(x, y, z, 1.0);
            p.xyz() / p.w
        };
        let (near, far) = (unproject(0.0), unproject(1.0));
        (near, (far - near).normalize())
    }

    /// A world point to window coordinates, or None behind the camera.
    pub fn project(&self, p: Vec3) -> Option<Point<Pixels>> {
        let b = self.bounds.get();
        let c = self.camera.view_proj(self.aspect()) * p.extend(1.0);
        if c.w <= 1e-5 {
            return None;
        }
        let n = c.xyz() / c.w;
        Some(point(
            b.origin.x + px((n.x * 0.5 + 0.5) * f32::from(b.size.width)),
            b.origin.y + px((0.5 - n.y * 0.5) * f32::from(b.size.height)),
        ))
    }

    /// Orbit (or pan with `pan`) by a mouse delta in logical pixels.
    pub fn drag_by(&mut self, delta: Point<Pixels>, pan: bool) {
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        if pan {
            let h = f32::from(self.bounds.get().size.height).max(1.0);
            let metres = self.camera.distance * (self.camera.fov * 0.5).tan() * 2.0 / h;
            self.camera.pan(-dx * metres, dy * metres);
        } else {
            self.camera.orbit(-dx * 0.008, dy * 0.008);
        }
    }

    /// `view.camera` args that put the document camera where this one is.
    pub fn camera_args(&self) -> serde_json::Value {
        serde_json::json!({
            "target": self.camera.target.to_array(),
            "position": self.camera.eye().to_array(),
            "ortho": self.camera.ortho,
        })
    }
}

/// The nearest visible mesh triangle under a ray: object, face index, hit point.
pub fn pick(doc: &Document, origin: Vec3, dir: Vec3) -> Option<(Id<Object>, u32, Vec3)> {
    let mut best: Option<(f32, Id<Object>, u32)> = None;
    for object in doc.objects.iter().filter(|o| o.visible) {
        let Some(mesh) = doc.drawn_mesh(object) else {
            continue;
        };
        let world: Mat4 = doc.world_matrix(object);
        let p: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|v| world.transform_point3(*v))
            .collect();
        for (fi, face) in mesh.faces.iter().enumerate() {
            if face.hidden {
                continue;
            }
            let c = &face.corners;
            for k in 1..c.len().saturating_sub(1) {
                let tri = [
                    p[c[0].vertex as usize],
                    p[c[k].vertex as usize],
                    p[c[k + 1].vertex as usize],
                ];
                if let Some(t) = ray_triangle(origin, dir, tri)
                    && best.is_none_or(|b| t < b.0)
                {
                    best = Some((t, object.id, fi as u32));
                }
            }
        }
    }
    best.map(|(t, id, face)| (id, face, origin + dir * t))
}

/// Möller-Trumbore, both sides.
fn ray_triangle(o: Vec3, d: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-5).then_some(t)
}

/// The tile cell [i, j] under a ray on the placement plane, or None if the ray
/// runs parallel to it or points away.
pub fn tile_cell(doc: &Document, origin: Vec3, dir: Vec3) -> Option<[i32; 2]> {
    let axis = match doc.view.tile.axis {
        quarry_core::model::Axis::X => 0,
        quarry_core::model::Axis::Y => 1,
        quarry_core::model::Axis::Z => 2,
    };
    let f = quarry_ops::tile::frame(doc.view.cursor, axis, doc.view.tile.tilt);
    let denom = dir.dot(f.n);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (f.origin - origin).dot(f.n) / denom;
    if t <= 0.0 {
        return None;
    }
    let hit = origin + dir * t - f.origin;
    let uv = Vec2::new(hit.dot(f.u), hit.dot(f.v));
    Some([uv.x.floor() as i32, uv.y.floor() as i32])
}

/// Records the element's on-screen bounds so the view can size its render.
pub fn measure(bounds: Rc<Cell<Bounds<Pixels>>>) -> impl IntoElement {
    canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
        .absolute()
        .size_full()
}

#[cfg(test)]
mod tests {
    use super::ray_triangle;
    use glam::Vec3;

    #[test]
    fn a_ray_hits_the_near_side_of_a_triangle() {
        let tri = [
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let t = ray_triangle(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z, tri).unwrap();
        assert!((t - 5.0).abs() < 1e-5);
        assert!(ray_triangle(Vec3::new(3.0, 0.0, 5.0), Vec3::NEG_Z, tri).is_none());
        assert!(ray_triangle(Vec3::new(0.0, 0.0, 5.0), Vec3::Z, tri).is_none());
    }
}
