//! An immutable, GPU-ready copy of what the document shows: triangles grouped
//! by image, in world space. Built on the CPU after each commit.

use glam::{Mat4, Vec3};
use quarry_core::id::Id;
use quarry_core::model::{Document, Filter, Image, Mesh, Object};
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DocVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// 0..1, from the face's pixel UVs and the image size.
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// Triangles that share one image (or none).
pub struct Batch {
    pub image: Option<Id<Image>>,
    pub vertices: Vec<DocVertex>,
}

pub struct SnapshotImage {
    pub id: Id<Image>,
    pub size: [u32; 2],
    pub pixels: Arc<[u8]>,
    pub filter: Filter,
}

pub struct Snapshot {
    pub batches: Vec<Batch>,
    pub images: Vec<SnapshotImage>,
    /// World-space bounds of everything drawn.
    pub bounds: Option<(Vec3, Vec3)>,
    pub grid_step: f32,
}

impl Snapshot {
    pub fn triangle_count(&self) -> usize {
        self.batches.iter().map(|b| b.vertices.len() / 3).sum()
    }
}

/// World matrix of `object`, following parents (cycles cut at 64 levels).
pub fn world_matrix(doc: &Document, object: &Object) -> Mat4 {
    let mut matrix = Mat4::IDENTITY;
    let mut current = Some(object);
    for _ in 0..64 {
        let Some(o) = current else { break };
        let t = &o.transform;
        matrix = Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation) * matrix;
        current = o.parent.and_then(|p| doc.objects.get(p));
    }
    matrix
}

pub fn snapshot(doc: &Document) -> Snapshot {
    let mut batches: Vec<Batch> = Vec::new();
    let mut bounds: Option<(Vec3, Vec3)> = None;

    for object in doc.objects.iter().filter(|o| o.visible) {
        // Instances draw their source's mesh with their own transform.
        let mesh = match object.instance_of {
            Some(source) => doc.objects.get(source).and_then(|s| s.mesh.as_deref()),
            None => object.mesh.as_deref(),
        };
        let Some(mesh) = mesh else { continue };
        let world = world_matrix(doc, object);
        add_mesh(doc, mesh, world, &mut batches, &mut bounds);
    }

    let images = doc
        .images
        .iter()
        .filter(|i| i.pixels.len() == (i.size[0] * i.size[1] * 4) as usize)
        .map(|i| SnapshotImage {
            id: i.id,
            size: i.size,
            pixels: i.pixels.clone(),
            filter: i.filter,
        })
        .collect();
    Snapshot {
        batches,
        images,
        bounds,
        grid_step: doc.settings.grid_step,
    }
}

fn add_mesh(
    doc: &Document,
    mesh: &Mesh,
    world: Mat4,
    batches: &mut Vec<Batch>,
    bounds: &mut Option<(Vec3, Vec3)>,
) {
    let normal_matrix = world.inverse().transpose();
    for face in mesh.faces.iter().filter(|f| !f.hidden) {
        let corners: Vec<_> = face
            .corners
            .iter()
            .filter(|c| (c.vertex as usize) < mesh.positions.len())
            .collect();
        if corners.len() < 3 {
            continue;
        }
        let points: Vec<Vec3> = corners
            .iter()
            .map(|c| world.transform_point3(mesh.positions[c.vertex as usize]))
            .collect();
        // Newell's method: a stable normal for triangles and slightly bent quads.
        let mut normal = Vec3::ZERO;
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            normal += Vec3::new(
                (a.y - b.y) * (a.z + b.z),
                (a.z - b.z) * (a.x + b.x),
                (a.x - b.x) * (a.y + b.y),
            );
        }
        let normal = normal.normalize_or(
            normal_matrix
                .transform_vector3(Vec3::Y)
                .normalize_or(Vec3::Y),
        );

        let size = face
            .image
            .and_then(|id| doc.images.get(id))
            .map_or([1.0, 1.0], |i| {
                [i.size[0].max(1) as f32, i.size[1].max(1) as f32]
            });
        let vertex = |i: usize| DocVertex {
            position: points[i].into(),
            normal: normal.into(),
            uv: [corners[i].uv.x / size[0], corners[i].uv.y / size[1]],
            color: corners[i].color.into(),
        };

        let batch = match batches.iter_mut().position(|b| b.image == face.image) {
            Some(i) => &mut batches[i],
            None => {
                batches.push(Batch {
                    image: face.image,
                    vertices: Vec::new(),
                });
                batches.last_mut().expect("just pushed")
            }
        };
        // Fan: 3 corners make one triangle, 4 make two.
        for i in 1..corners.len() - 1 {
            batch.vertices.extend([vertex(0), vertex(i), vertex(i + 1)]);
        }
        for p in &points {
            *bounds = Some(match *bounds {
                None => (*p, *p),
                Some((lo, hi)) => (lo.min(*p), hi.max(*p)),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quarry_core::{CommandRegistry, Source};
    use serde_json::json;

    #[test]
    fn a_box_is_twelve_triangles_in_world_space() {
        let mut r = CommandRegistry::new();
        r.execute(
            "mesh.add_primitive",
            json!({ "kind": "box", "position": [10, 0, 0] }),
            Source::Human,
        )
        .unwrap();
        let s = snapshot(r.document());
        assert_eq!(s.triangle_count(), 12);
        let (lo, hi) = s.bounds.unwrap();
        assert_eq!(
            (lo, hi),
            (Vec3::new(9.5, -0.5, -0.5), Vec3::new(10.5, 0.5, 0.5))
        );
        // Every face's normal points away from the box centre.
        for tri in s.batches[0].vertices.chunks(3) {
            let centre = tri.iter().map(|v| Vec3::from(v.position)).sum::<Vec3>() / 3.0;
            assert!(Vec3::from(tri[0].normal).dot(centre - Vec3::new(10.0, 0.0, 0.0)) > 0.0);
        }
    }

    #[test]
    fn hidden_objects_draw_nothing() {
        let mut r = CommandRegistry::new();
        r.execute(
            "mesh.add_primitive",
            json!({ "kind": "box" }),
            Source::Human,
        )
        .unwrap();
        r.execute(
            "scene.visibility",
            json!({ "objects": ["Box"], "visible": false }),
            Source::Human,
        )
        .unwrap();
        let s = snapshot(r.document());
        assert_eq!(s.triangle_count(), 0);
        assert!(s.bounds.is_none());
    }
}
