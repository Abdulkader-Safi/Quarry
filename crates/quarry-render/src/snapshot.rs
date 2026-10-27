//! An immutable, GPU-ready copy of what the document shows: triangles grouped
//! by image, in world space. Built on the CPU after each commit.

use glam::{Mat4, Vec3};
use quarry_core::id::Id;
use quarry_core::model::{Document, Filter, Image, Mesh, SelectMode};
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
    /// Line pairs drawn over the meshes: posed bones, and the 3D crosshair in
    /// the live view.
    pub overlay: Vec<DocVertex>,
}

impl Snapshot {
    pub fn triangle_count(&self) -> usize {
        self.batches.iter().map(|b| b.vertices.len() / 3).sum()
    }
}

pub fn snapshot(doc: &Document) -> Snapshot {
    snapshot_with(doc, false)
}

/// Selected objects (object mode) or selected faces (face mode) tinted cobalt
/// for the live viewport. Thumbnails and screenshots use [`snapshot`].
pub fn snapshot_with(doc: &Document, highlight: bool) -> Snapshot {
    let mut batches: Vec<Batch> = Vec::new();
    let mut bounds: Option<(Vec3, Vec3)> = None;

    for object in doc.objects.iter().filter(|o| o.visible) {
        // Instances draw their source's mesh with their own transform.
        let Some(mesh) = doc.drawn_mesh(object) else {
            continue;
        };
        let world = doc.world_matrix(object);
        // Skinned meshes show their skeleton's current pose (CPU skinning).
        let skinning = object
            .skin
            .as_ref()
            .filter(|_| mesh.weights.len() == mesh.positions.len())
            .and_then(|k| doc.skeletons.get(k.skeleton))
            .map(|s| s.skinning());
        let points = posed_points(mesh, world, skinning.as_deref());
        let sel = &doc.selection;
        let tint = if !highlight {
            Tint::None
        } else if sel.mode == SelectMode::Object && sel.objects.contains(&object.id) {
            Tint::All
        } else if sel.mode != SelectMode::Object && sel.object == Some(object.id) {
            Tint::Faces(&sel.faces)
        } else {
            Tint::None
        };
        add_mesh(doc, mesh, &points, world, tint, &mut batches, &mut bounds);
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
    let mut overlay = Vec::new();
    let mut line = |a: Vec3, b: Vec3, color: [f32; 4]| {
        for p in [a, b] {
            overlay.push(DocVertex {
                position: p.into(),
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color,
            });
        }
    };
    if doc.view.display.bones {
        for skeleton in doc.skeletons.iter() {
            for (bone, world) in skeleton.bones.iter().zip(skeleton.pose_world()) {
                let length = bone.head.distance(bone.tail);
                let (head, tail) = (
                    world.transform_point3(Vec3::ZERO),
                    world.transform_point3(Vec3::Y * length),
                );
                line(head, tail, [0.83, 0.85, 0.88, 1.0]);
            }
        }
    }
    if doc.view.display.onion {
        onion_skin(doc, &mut line);
    }
    if highlight {
        // The crosshair: short X, Y and Z ticks in the axis colours.
        let c = doc.view.cursor;
        let r = 0.3;
        line(c - Vec3::X * r, c + Vec3::X * r, [1.0, 0.07, 0.11, 1.0]);
        line(c - Vec3::Y * r, c + Vec3::Y * r, [0.2, 0.72, 0.07, 1.0]);
        line(c - Vec3::Z * r, c + Vec3::Z * r, [0.07, 0.26, 1.0, 1.0]);
    }
    Snapshot {
        batches,
        images,
        bounds,
        grid_step: doc.settings.grid_step,
        overlay,
    }
}

/// World positions of a mesh's vertices, skinned when `skinning` is given.
fn posed_points(mesh: &Mesh, world: glam::Mat4, skinning: Option<&[glam::Mat4]>) -> Vec<Vec3> {
    mesh.positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let p = world.transform_point3(*p);
            match skinning {
                Some(m) => {
                    let w = mesh.weights[i];
                    (0..4).fold(Vec3::ZERO, |acc, k| {
                        acc + m
                            .get(w.bones[k] as usize)
                            .map_or(p, |m| m.transform_point3(p))
                            * w.weights[k]
                    })
                }
                None => p,
            }
        })
        .collect()
}

/// FR-ANIM-07: the shown clip's poses at the keys either side of the
/// playhead, as dim edge outlines of every mesh skinned to its skeleton
/// (warm before, cool after).
fn onion_skin(doc: &Document, line: &mut impl FnMut(Vec3, Vec3, [f32; 4])) {
    let Some(clip) = doc.view.anim.clip.and_then(|c| doc.clips.get(c)) else {
        return;
    };
    let Some(skeleton) = doc.skeletons.get(clip.skeleton) else {
        return;
    };
    let now = doc.view.anim.time;
    let mut times: Vec<f32> = clip
        .tracks
        .iter()
        .flat_map(|t| t.keys.iter().map(|k| k.time))
        .collect();
    times.sort_by(f32::total_cmp);
    let before = times.iter().rev().find(|t| **t < now - 1e-4);
    let after = times.iter().find(|t| **t > now + 1e-4);
    for (time, color) in [
        (before, [0.55, 0.32, 0.18, 1.0]),
        (after, [0.2, 0.38, 0.6, 1.0]),
    ] {
        let Some(&time) = time else { continue };
        let skinning = clip.pose_at(skeleton, time).skinning();
        for object in doc.objects.iter().filter(|o| o.visible) {
            let skinned = object
                .skin
                .as_ref()
                .is_some_and(|k| k.skeleton == clip.skeleton);
            let Some(mesh) = doc
                .drawn_mesh(object)
                .filter(|m| skinned && m.weights.len() == m.positions.len())
            else {
                continue;
            };
            let points = posed_points(mesh, doc.world_matrix(object), Some(&skinning));
            let mut edges = std::collections::BTreeSet::new();
            for f in mesh.faces.iter().filter(|f| !f.hidden) {
                let c = &f.corners;
                for i in 0..c.len() {
                    let (a, b) = (c[i].vertex, c[(i + 1) % c.len()].vertex);
                    edges.insert((a.min(b), a.max(b)));
                }
            }
            for (a, b) in edges {
                line(points[a as usize], points[b as usize], color);
            }
        }
    }
}

enum Tint<'a> {
    None,
    All,
    Faces(&'a std::collections::BTreeSet<u32>),
}

/// Accent/default (#3873d3) in linear RGB, mixed into highlighted faces.
const ACCENT: [f32; 3] = [0.039, 0.171, 0.651];

fn add_mesh(
    doc: &Document,
    mesh: &Mesh,
    // World position of each vertex, posed when skinned.
    world_points: &[Vec3],
    world: Mat4,
    tint: Tint,
    batches: &mut Vec<Batch>,
    bounds: &mut Option<(Vec3, Vec3)>,
) {
    let normal_matrix = world.inverse().transpose();
    // Vertex colours hidden by the display toggle draw white.
    let show_colors = doc.view.display.vertex_color;
    let shown = |c: glam::Vec4| if show_colors { c } else { glam::Vec4::ONE };
    let mirrored = world.determinant() < 0.0;
    let mut corner_normals = mesh.corner_normals(world_points);
    if mirrored {
        corner_normals.iter_mut().flatten().for_each(|n| *n = -*n);
    }
    for (face_index, face) in mesh.faces.iter().enumerate().filter(|(_, f)| !f.hidden) {
        let lit = match tint {
            Tint::None => false,
            Tint::All => true,
            Tint::Faces(set) => set.contains(&(face_index as u32)),
        };
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
            .map(|c| world_points[c.vertex as usize])
            .collect();
        let fallback = normal_matrix
            .transform_vector3(Vec3::Y)
            .normalize_or(Vec3::Y);
        let normals: Vec<Vec3> = face
            .corners
            .iter()
            .zip(&corner_normals[face_index])
            .filter(|(c, _)| (c.vertex as usize) < mesh.positions.len())
            .map(|(_, n)| {
                if n.length_squared() > 0.0 {
                    *n
                } else {
                    fallback
                }
            })
            .collect();

        let size = face
            .image
            .and_then(|id| doc.images.get(id))
            .map_or([1.0, 1.0], |i| {
                [i.size[0].max(1) as f32, i.size[1].max(1) as f32]
            });
        let vertex = |i: usize| DocVertex {
            position: points[i].into(),
            normal: normals[i].into(),
            uv: [corners[i].uv.x / size[0], corners[i].uv.y / size[1]],
            color: if lit {
                let c = shown(corners[i].color);
                [
                    c.x * 0.5 + ACCENT[0] * 0.5,
                    c.y * 0.5 + ACCENT[1] * 0.5,
                    c.z * 0.5 + ACCENT[2] * 0.5,
                    c.w,
                ]
            } else {
                shown(corners[i].color).into()
            },
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
        // Fan: 3 corners make one triangle, 4 make two. A mirrored object
        // (negative scale) turns its faces inside out, so wind them back.
        for i in 1..corners.len() - 1 {
            if mirrored {
                batch.vertices.extend([vertex(0), vertex(i + 1), vertex(i)]);
            } else {
                batch.vertices.extend([vertex(0), vertex(i), vertex(i + 1)]);
            }
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

#[cfg(test)]
mod pose_tests {
    use super::*;
    use quarry_core::{CommandRegistry, Source};
    use serde_json::json;

    #[test]
    fn a_posed_skeleton_moves_what_is_drawn() {
        let mut r = CommandRegistry::new();
        let mut run = |n: &str, a: serde_json::Value| r.execute(n, a, Source::Human).unwrap();
        run("rig.skeleton_new", json!({}));
        run(
            "rig.bone_add",
            json!({ "name": "b", "head": [0, 0, 0], "tail": [0, 1, 0] }),
        );
        run(
            "mesh.add_primitive",
            json!({ "kind": "box", "name": "M", "position": [0, 0.5, 0], "size": [0.2, 1, 0.2] }),
        );
        run("rig.bind_rigid", json!({ "object": "M", "bone": "b" }));
        let rest = snapshot(r.document()).bounds.unwrap();
        r.execute(
            "rig.pose",
            json!({ "bone": "b", "rotation": [0, 0, 90] }),
            Source::Human,
        )
        .unwrap();
        let posed = snapshot(r.document()).bounds.unwrap();
        assert!(
            rest.1.y > 0.9 && posed.1.y < 0.2,
            "the box now lies along -X: {posed:?}"
        );
        assert!(posed.0.x < -0.9);
    }

    #[test]
    fn onion_skin_outlines_the_keys_either_side_of_the_playhead() {
        let mut r = CommandRegistry::new();
        let mut run = |n: &str, a: serde_json::Value| {
            r.execute(n, a, Source::Human)
                .unwrap_or_else(|e| panic!("{n}: {}", e.message));
        };
        run("rig.skeleton_new", json!({}));
        run(
            "rig.bone_add",
            json!({ "name": "arm", "head": [0, 0, 0], "tail": [0, 1, 0] }),
        );
        run(
            "mesh.add_primitive",
            json!({ "kind": "box", "name": "Box" }),
        );
        run("rig.bind_rigid", json!({ "object": "Box", "bone": "arm" }));
        run("anim.clip_new", json!({ "name": "swing", "length": 1.0 }));
        for (t, z) in [(0.0, 0), (1.0, 60)] {
            run(
                "anim.key_set",
                json!({ "bone": "arm", "channel": "rotation", "time": t, "value": [0, 0, z] }),
            );
        }
        run("anim.playhead", json!({ "time": 0.5 }));
        run("view.display", json!({ "bones": false }));
        let lines = |r: &CommandRegistry| snapshot(r.document()).overlay.len() / 2;
        assert_eq!(lines(&r), 0);
        r.execute("view.display", json!({ "onion": true }), Source::Human)
            .unwrap();
        assert_eq!(
            lines(&r),
            2 * 12,
            "a box's 12 edges at each neighbouring key"
        );
    }
}
