//! Mesh edits on the selection (FR-MESH-03 to 06, 08). Element edits act on
//! the mesh being edited (`select.mode`); transforms in object mode move objects.

use super::select::edit_mesh;
use super::tile::compact;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{Axis, Corner, Document, Face, Influences, Mesh, Object, SelectMode};
use glam::{EulerRot, Mat4, Quat, Vec2, Vec3, Vec4};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const SAME: f32 = 1e-4;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<MeshTransform>();
    r.register::<MeshExtrude>();
    r.register::<MeshMerge>();
    r.register::<MeshSplit>();
    r.register::<MeshDelete>();
    r.register::<MeshMakeFace>();
    r.register::<MeshFlipNormals>();
    r.register::<MeshFlipDiagonal>();
    r.register::<MeshMirrorMode>();
    r.register::<MeshMirrorApply>();
}

/// The edited object's id, world matrix and a mutable copy of its mesh.
fn editing(doc: &mut Document) -> Result<(Id<Object>, Mat4, &mut Mesh), CommandError> {
    let (o, _) = edit_mesh(doc)?;
    let (id, world) = (o.id, doc.world_matrix(o));
    let object = doc.objects.get_mut(id).expect("edited object exists");
    let mesh = Arc::make_mut(object.mesh.as_mut().expect("edit_mesh checked"));
    Ok((id, world, mesh))
}

fn nothing(what: &str) -> CommandError {
    CommandError::new(
        ErrorCode::NothingSelected,
        format!("No {what} selected. Call select first."),
    )
}

fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

fn mirrored(p: Vec3, axis: usize) -> Vec3 {
    let mut q = p;
    q[axis] = -q[axis];
    q
}

fn face_normal(mesh: &Mesh, face: &Face) -> Vec3 {
    let p: Vec<Vec3> = face
        .corners
        .iter()
        .map(|c| mesh.positions[c.vertex as usize])
        .collect();
    let mut n = Vec3::ZERO;
    for i in 0..p.len() {
        n += p[i].cross(p[(i + 1) % p.len()]);
    }
    n.normalize_or_zero()
}

fn add_vertex(mesh: &mut Mesh, p: Vec3, like: u32) -> u32 {
    mesh.positions.push(p);
    if !mesh.weights.is_empty() {
        let w = mesh.weights.get(like as usize).copied().unwrap_or_default();
        mesh.weights.push(w);
    }
    (mesh.positions.len() - 1) as u32
}

/// Drop faces left with under 3 distinct vertices, and repeated corners.
fn tidy(mesh: &mut Mesh) {
    for face in &mut mesh.faces {
        face.corners.dedup_by_key(|c| c.vertex);
        while face.corners.len() > 1
            && face.corners.first().map(|c| c.vertex) == face.corners.last().map(|c| c.vertex)
        {
            face.corners.pop();
        }
    }
    mesh.faces.retain(|f| {
        let distinct: BTreeSet<u32> = f.corners.iter().map(|c| c.vertex).collect();
        distinct.len() >= 3 && f.corners.len() <= 4
    });
    compact(mesh);
}

// ---- transform -------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Pivot {
    /// The middle of the selection.
    #[default]
    Median,
    /// The 3D crosshair.
    Cursor,
    /// Each object about its own origin (object mode).
    Origins,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshTransform {
    /// Move by [x, y, z] metres, in world space.
    pub translate: Option<[f32; 3]>,
    /// Rotate by Euler degrees [x, y, z].
    pub rotate: Option<[f32; 3]>,
    /// Scale by [x, y, z].
    pub scale: Option<[f32; 3]>,
    /// Centre for rotate and scale. Default median.
    #[serde(default)]
    pub pivot: Pivot,
}

impl Command for MeshTransform {
    const NAME: &'static str = "mesh.transform";
    const DESCRIPTION: &'static str = "Move, rotate and scale the selection: objects in object mode, or the edited mesh's selected vertices, edges or faces. Live mirror moves the twins too.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let t = Vec3::from(self.translate.unwrap_or([0.0; 3]));
        let r = Vec3::from(self.rotate.unwrap_or([0.0; 3]));
        let s = Vec3::from(self.scale.unwrap_or([1.0; 3]));
        if !(t.is_finite() && r.is_finite() && s.is_finite()) {
            return Err(CommandError::invalid(
                "translate, rotate and scale must be finite numbers.",
            ));
        }
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            r.x.to_radians(),
            r.y.to_radians(),
            r.z.to_radians(),
        );
        let about = |pivot: Vec3| {
            Mat4::from_translation(pivot + t)
                * Mat4::from_quat(rotation)
                * Mat4::from_scale(s)
                * Mat4::from_translation(-pivot)
        };
        let doc = cx.doc();
        let cursor = doc.view.cursor;

        if doc.selection.mode == SelectMode::Object {
            let ids: Vec<Id<Object>> = doc.selection.objects.iter().copied().collect();
            if ids.is_empty() {
                return Err(nothing("objects"));
            }
            let median = doc
                .bounds(ids.iter().filter_map(|id| doc.objects.get(*id)))
                .map_or(Vec3::ZERO, |(lo, hi)| (lo + hi) * 0.5);
            for id in &ids {
                let object = doc.objects.get(*id).expect("selected");
                let world = doc.world_matrix(object);
                let parent = object
                    .parent
                    .and_then(|p| doc.objects.get(p))
                    .map_or(Mat4::IDENTITY, |p| doc.world_matrix(p));
                let pivot = match self.pivot {
                    Pivot::Median => median,
                    Pivot::Cursor => cursor,
                    Pivot::Origins => world.transform_point3(Vec3::ZERO),
                };
                let local = parent.inverse() * about(pivot) * world;
                let (scale, rot, translation) = local.to_scale_rotation_translation();
                let tf = &mut doc.objects.get_mut(*id).expect("selected").transform;
                (tf.scale, tf.rotation, tf.translation) = (scale, rot.normalize(), translation);
            }
            return Ok(json!({ "objects": ids.len() }));
        }

        let selection = doc.selection.clone();
        let (_, world, mesh) = editing(doc)?;
        let moving = selection.touched_vertices(mesh);
        if moving.is_empty() {
            return Err(nothing("vertices"));
        }
        let points: Vec<Vec3> = moving
            .iter()
            .map(|v| world.transform_point3(mesh.positions[*v as usize]))
            .collect();
        let median = points.iter().sum::<Vec3>() / points.len() as f32;
        let pivot = match self.pivot {
            Pivot::Cursor => cursor,
            _ => median,
        };
        let local = world.inverse() * about(pivot) * world;

        // Live mirror: find each moving vertex's twin before anything moves.
        let mirror = mesh.mirror.map(axis_index);
        let twins: Vec<(u32, u32)> = match mirror {
            Some(axis) => moving
                .iter()
                .filter_map(|&v| {
                    let target = mirrored(mesh.positions[v as usize], axis);
                    let twin = mesh
                        .positions
                        .iter()
                        .position(|p| p.distance(target) < SAME)?
                        as u32;
                    (twin != v && !moving.contains(&twin)).then_some((v, twin))
                })
                .collect(),
            None => Vec::new(),
        };
        for &v in &moving {
            let p = &mut mesh.positions[v as usize];
            let on_axis = mirror.is_some_and(|a| p[a].abs() < SAME);
            *p = local.transform_point3(*p);
            if let (true, Some(a)) = (on_axis, mirror) {
                p[a] = 0.0; // vertices on the mirror plane stay welded to it
            }
        }
        for (v, twin) in &twins {
            mesh.positions[*twin as usize] = mirrored(
                mesh.positions[*v as usize],
                mirror.expect("twins imply mirror"),
            );
        }
        Ok(json!({ "vertices": moving.len(), "mirrored": twins.len() }))
    }
}

// ---- extrude ---------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshExtrude {
    /// How far, in metres.
    pub distance: f32,
    /// World direction [x, y, z]. Default: along the faces' normal (faces) or
    /// the neighbouring faces' normal (edges).
    pub direction: Option<[f32; 3]>,
}

impl Command for MeshExtrude {
    const NAME: &'static str = "mesh.extrude";
    const DESCRIPTION: &'static str = "Extrude the selected faces (as one region) or edges by a distance. New side faces reuse the source face's tile and image.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if !self.distance.is_finite() {
            return Err(CommandError::invalid("distance must be a finite number."));
        }
        let doc = cx.doc();
        let mode = doc.selection.mode;
        let selection = doc.selection.clone();
        let (id, world, mesh) = editing(doc)?;
        let to_local = world.inverse();
        let direction = self.direction.map(|d| {
            to_local
                .transform_vector3(Vec3::from(d))
                .normalize_or_zero()
        });

        let (new_faces, new_edges) = match mode {
            SelectMode::Face => {
                if selection.faces.is_empty() {
                    return Err(nothing("faces"));
                }
                (
                    extrude_faces(mesh, &selection.faces, self.distance, direction),
                    BTreeSet::new(),
                )
            }
            SelectMode::Edge => {
                if selection.edges.is_empty() {
                    return Err(nothing("edges"));
                }
                extrude_edges(mesh, &selection.edges, self.distance, direction)
            }
            _ => {
                return Err(CommandError::invalid(
                    "Extrude works on faces or edges. Call select.mode with face or edge.",
                ));
            }
        };
        let sel = &mut cx.doc().selection;
        sel.object = Some(id);
        match mode {
            SelectMode::Face => sel.faces = new_faces.clone(),
            _ => sel.edges = new_edges,
        }
        Ok(json!({ "faces": new_faces.len() }))
    }
}

/// Region extrude: the selected faces move out on new vertices, and each
/// boundary edge gets a side quad. Returns the moved faces (still selected).
fn extrude_faces(
    mesh: &mut Mesh,
    faces: &BTreeSet<u32>,
    distance: f32,
    direction: Option<Vec3>,
) -> BTreeSet<u32> {
    // Boundary edges: used once among the selected faces, kept in face winding.
    let mut count: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for f in faces {
        let c = &mesh.faces[*f as usize].corners;
        for k in 0..c.len() {
            let (a, b) = (c[k].vertex, c[(k + 1) % c.len()].vertex);
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    // Per-vertex direction: the average normal of the selected faces using it.
    let mut normals: BTreeMap<u32, Vec3> = BTreeMap::new();
    for f in faces {
        let face = &mesh.faces[*f as usize];
        let n = face_normal(mesh, face);
        for c in &face.corners {
            *normals.entry(c.vertex).or_default() += n;
        }
    }
    let mut copy: BTreeMap<u32, u32> = BTreeMap::new();
    for (&v, n) in &normals {
        let dir = direction.unwrap_or_else(|| n.normalize_or_zero());
        let p = mesh.positions[v as usize] + dir * distance;
        copy.insert(v, add_vertex(mesh, p, v));
    }
    let mut sides = Vec::new();
    for f in faces {
        let face = mesh.faces[*f as usize].clone();
        let n = face.corners.len();
        let (lo, hi) = uv_box(&face);
        for k in 0..n {
            let (a, b) = (face.corners[k].vertex, face.corners[(k + 1) % n].vertex);
            if count[&(a.min(b), a.max(b))] == 1 {
                sides.push(side_quad(a, b, copy[&b], copy[&a], lo, hi, &face));
            }
        }
        let moved = &mut mesh.faces[*f as usize];
        for c in &mut moved.corners {
            c.vertex = copy[&c.vertex];
        }
    }
    mesh.faces.extend(sides);
    let selected = faces.clone();
    // Vertices only the moved faces used are now orphans.
    let before = mesh.faces.len();
    compact(mesh);
    debug_assert_eq!(before, mesh.faces.len());
    selected
}

/// Edge extrude: each selected edge grows a quad; shared vertices extrude once.
fn extrude_edges(
    mesh: &mut Mesh,
    selected: &BTreeSet<(u32, u32)>,
    distance: f32,
    direction: Option<Vec3>,
) -> (BTreeSet<u32>, BTreeSet<(u32, u32)>) {
    let mut copy: BTreeMap<u32, u32> = BTreeMap::new();
    let mut quads = Vec::new();
    for &(a, b) in selected {
        // The face that owns this edge gives winding, normal and tile.
        let owner = mesh.faces.iter().find(|f| {
            let n = f.corners.len();
            (0..n).any(|k| {
                (f.corners[k].vertex, f.corners[(k + 1) % n].vertex) == (a, b)
                    || (f.corners[k].vertex, f.corners[(k + 1) % n].vertex) == (b, a)
            })
        });
        let (from, to, template, normal) = match owner {
            Some(f) => {
                let n = f.corners.len();
                let forward =
                    (0..n).any(|k| (f.corners[k].vertex, f.corners[(k + 1) % n].vertex) == (a, b));
                let (x, y) = if forward { (b, a) } else { (a, b) };
                (x, y, f.clone(), face_normal(mesh, f))
            }
            None => continue,
        };
        // Out of the face, along its plane, unless a direction is given.
        let edge_dir =
            (mesh.positions[to as usize] - mesh.positions[from as usize]).normalize_or_zero();
        let dir = direction.unwrap_or_else(|| edge_dir.cross(normal).normalize_or_zero());
        for v in [from, to] {
            if let std::collections::btree_map::Entry::Vacant(slot) = copy.entry(v) {
                let p = mesh.positions[v as usize] + dir * distance;
                slot.insert(add_vertex(mesh, p, v));
            }
        }
        let (lo, hi) = uv_box(&template);
        quads.push(side_quad(
            from,
            to,
            copy[&to],
            copy[&from],
            lo,
            hi,
            &template,
        ));
    }
    let start = mesh.faces.len() as u32;
    let new_edges = selected
        .iter()
        .filter_map(|(a, b)| {
            let (x, y) = (*copy.get(a)?, *copy.get(b)?);
            Some((x.min(y), x.max(y)))
        })
        .collect();
    let added = quads.len() as u32;
    mesh.faces.extend(quads);
    ((start..start + added).collect(), new_edges)
}

fn uv_box(face: &Face) -> (Vec2, Vec2) {
    face.corners.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), c| (lo.min(c.uv), hi.max(c.uv)),
    )
}

/// A quad a, b, b', a' showing the source face's whole tile.
fn side_quad(a: u32, b: u32, b2: u32, a2: u32, lo: Vec2, hi: Vec2, like: &Face) -> Face {
    let uv = [
        Vec2::new(lo.x, hi.y),
        Vec2::new(hi.x, hi.y),
        Vec2::new(hi.x, lo.y),
        Vec2::new(lo.x, lo.y),
    ];
    Face {
        corners: [a, b, b2, a2]
            .iter()
            .zip(uv)
            .map(|(&vertex, uv)| Corner {
                vertex,
                uv,
                color: Vec4::ONE,
            })
            .collect(),
        image: like.image,
        smooth_group: like.smooth_group,
        tile: like.tile,
        hidden: false,
    }
}

// ---- merge, split, delete --------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeAt {
    /// All selected vertices become one, at their middle.
    Center,
    /// All selected vertices become one, at the 3D crosshair.
    Cursor,
    /// Vertices closer than `distance` become one.
    Distance,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshMerge {
    pub at: MergeAt,
    /// Threshold in metres for `distance`. Default 0.001.
    pub distance: Option<f32>,
}

impl Command for MeshMerge {
    const NAME: &'static str = "mesh.merge";
    const DESCRIPTION: &'static str = "Merge the selected vertices: into one at their centre or the crosshair, or by distance (the whole mesh when nothing is selected). Faces that collapse are removed.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let cursor = doc.view.cursor;
        let selection = doc.selection.clone();
        let (_, world, mesh) = editing(doc)?;
        let chosen = selection.touched_vertices(mesh);
        let before = mesh.positions.len();
        let mut target: BTreeMap<u32, u32> = BTreeMap::new();
        match self.at {
            MergeAt::Center | MergeAt::Cursor => {
                if chosen.len() < 2 {
                    return Err(nothing("vertices (at least two)"));
                }
                let keep = *chosen.first().expect("two or more");
                mesh.positions[keep as usize] = match self.at {
                    MergeAt::Cursor => world.inverse().transform_point3(cursor),
                    _ => {
                        chosen
                            .iter()
                            .map(|v| mesh.positions[*v as usize])
                            .sum::<Vec3>()
                            / chosen.len() as f32
                    }
                };
                for v in &chosen {
                    target.insert(*v, keep);
                }
            }
            MergeAt::Distance => {
                let limit = self.distance.unwrap_or(0.001);
                if !(limit.is_finite() && limit >= 0.0) {
                    return Err(CommandError::invalid(
                        "distance must be zero or more metres.",
                    ));
                }
                let pool: Vec<u32> = if chosen.is_empty() {
                    (0..mesh.positions.len() as u32).collect()
                } else {
                    chosen.iter().copied().collect()
                };
                // ponytail: O(n²) pairing; a spatial grid when meshes reach 10k+ vertices.
                for (i, &a) in pool.iter().enumerate() {
                    if target.contains_key(&a) {
                        continue;
                    }
                    for &b in &pool[i + 1..] {
                        if !target.contains_key(&b)
                            && mesh.positions[a as usize].distance(mesh.positions[b as usize])
                                <= limit
                        {
                            target.insert(b, a);
                        }
                    }
                }
            }
        }
        for c in mesh.faces.iter_mut().flat_map(|f| &mut f.corners) {
            if let Some(t) = target.get(&c.vertex) {
                c.vertex = *t;
            }
        }
        tidy(mesh);
        let merged = before - mesh.positions.len();
        let sel = &mut cx.doc().selection;
        sel.vertices.clear();
        sel.edges.clear();
        sel.faces.clear();
        Ok(json!({ "removed": merged }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshSplit {}

impl Command for MeshSplit {
    const NAME: &'static str = "mesh.split";
    const DESCRIPTION: &'static str = "Split the selected faces off their neighbours: shared vertices are duplicated so the faces move on their own.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let faces = doc.selection.faces.clone();
        if faces.is_empty() || doc.selection.mode != SelectMode::Face {
            return Err(nothing("faces"));
        }
        let (_, _, mesh) = editing(doc)?;
        let outside: BTreeSet<u32> = (0..mesh.faces.len() as u32)
            .filter(|f| !faces.contains(f))
            .flat_map(|f| {
                mesh.faces[f as usize]
                    .corners
                    .iter()
                    .map(|c| c.vertex)
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut copy: BTreeMap<u32, u32> = BTreeMap::new();
        for f in &faces {
            for k in 0..mesh.faces[*f as usize].corners.len() {
                let v = mesh.faces[*f as usize].corners[k].vertex;
                if outside.contains(&v) {
                    let n = match copy.get(&v) {
                        Some(n) => *n,
                        None => {
                            let n = add_vertex(mesh, mesh.positions[v as usize], v);
                            copy.insert(v, n);
                            n
                        }
                    };
                    mesh.faces[*f as usize].corners[k].vertex = n;
                }
            }
        }
        Ok(json!({ "new_vertices": copy.len() }))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeleteWhat {
    /// Selected vertices and every face using them.
    Vertices,
    /// Selected faces and any vertices left unused.
    Faces,
    /// Selected faces only, leaving a hole and keeping the vertices.
    OnlyFaces,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshDelete {
    pub what: DeleteWhat,
}

impl Command for MeshDelete {
    const NAME: &'static str = "mesh.delete";
    const DESCRIPTION: &'static str = "Delete selected vertices (with their faces), selected faces, or only the faces (keeping vertices).";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let selection = doc.selection.clone();
        let (_, _, mesh) = editing(doc)?;
        let before = (mesh.positions.len(), mesh.faces.len());
        match self.what {
            DeleteWhat::Vertices => {
                let gone = selection.touched_vertices(mesh);
                if gone.is_empty() {
                    return Err(nothing("vertices"));
                }
                mesh.faces
                    .retain(|f| f.corners.iter().all(|c| !gone.contains(&c.vertex)));
                // Remove the vertices themselves even if no face used them.
                let mut index = 0u32;
                let keep: Vec<bool> = mesh
                    .positions
                    .iter()
                    .map(|_| {
                        index += 1;
                        !gone.contains(&(index - 1))
                    })
                    .collect();
                let mut remap = Vec::with_capacity(keep.len());
                let mut next = 0u32;
                for k in &keep {
                    remap.push(next);
                    next += *k as u32;
                }
                let mut i = 0;
                mesh.positions.retain(|_| {
                    i += 1;
                    keep[i - 1]
                });
                if !mesh.weights.is_empty() {
                    let mut i = 0;
                    mesh.weights.retain(|_| {
                        i += 1;
                        keep[i - 1]
                    });
                }
                for c in mesh.faces.iter_mut().flat_map(|f| &mut f.corners) {
                    c.vertex = remap[c.vertex as usize];
                }
            }
            DeleteWhat::Faces | DeleteWhat::OnlyFaces => {
                if selection.faces.is_empty() {
                    return Err(nothing("faces"));
                }
                let mut i = 0u32;
                mesh.faces.retain(|_| {
                    i += 1;
                    !selection.faces.contains(&(i - 1))
                });
                if matches!(self.what, DeleteWhat::Faces) {
                    compact(mesh);
                }
            }
        }
        let after = (mesh.positions.len(), mesh.faces.len());
        let sel = &mut cx.doc().selection;
        sel.vertices.clear();
        sel.edges.clear();
        sel.faces.clear();
        Ok(json!({ "vertices_removed": before.0 - after.0, "faces_removed": before.1 - after.1 }))
    }
}

// ---- make face, flips ------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshMakeFace {}

impl Command for MeshMakeFace {
    const NAME: &'static str = "mesh.make_face";
    const DESCRIPTION: &'static str =
        "Make one face from 3 or 4 selected vertices, facing the camera.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let eye = doc.view.camera.eye();
        let tile = doc.settings.tile_px as f32;
        let selection = doc.selection.clone();
        let (_, world, mesh) = editing(doc)?;
        let verts: Vec<u32> = selection.touched_vertices(mesh).into_iter().collect();
        if !(3..=4).contains(&verts.len()) {
            return Err(CommandError::invalid(format!(
                "Select 3 or 4 vertices to make a face ({} are selected).",
                verts.len()
            )));
        }
        let p: Vec<Vec3> = verts.iter().map(|v| mesh.positions[*v as usize]).collect();
        let centre = p.iter().sum::<Vec3>() / p.len() as f32;
        let normal = (p[1] - p[0]).cross(p[2] - p[0]);
        let normal = if normal.length() < 1e-9 {
            (p[2] - p[0]).cross(p[3 % p.len()] - p[0])
        } else {
            normal
        }
        .normalize_or_zero();
        if normal == Vec3::ZERO {
            return Err(CommandError::invalid(
                "Those vertices are in a line, so they cannot make a face.",
            ));
        }
        // Order around the centre so the outline does not cross itself.
        let u = (p[0] - centre).normalize_or_zero();
        let w = normal.cross(u);
        let mut order: Vec<usize> = (0..p.len()).collect();
        order.sort_by(|a, b| {
            let angle = |i: usize| (p[i] - centre).dot(w).atan2((p[i] - centre).dot(u));
            angle(*a).total_cmp(&angle(*b))
        });
        // Face the camera.
        let to_eye = world.inverse().transform_point3(eye) - centre;
        if normal.dot(to_eye) < 0.0 {
            order.reverse();
        }
        if mesh.faces.iter().any(|f| {
            let set: BTreeSet<u32> = f.corners.iter().map(|c| c.vertex).collect();
            set == verts.iter().copied().collect()
        }) {
            return Err(CommandError::new(
                ErrorCode::Conflict,
                "A face already uses exactly those vertices.",
            ));
        }
        let uv = [
            Vec2::new(0.0, tile),
            Vec2::new(tile, tile),
            Vec2::new(tile, 0.0),
            Vec2::new(0.0, 0.0),
        ];
        mesh.faces.push(Face {
            corners: order
                .iter()
                .zip(uv)
                .map(|(&i, uv)| Corner {
                    vertex: verts[i],
                    uv,
                    color: Vec4::ONE,
                })
                .collect(),
            image: None,
            smooth_group: 0,
            tile: None,
            hidden: false,
        });
        Ok(json!({ "face": mesh.faces.len() - 1 }))
    }
}

/// Selected faces, or every face when none are selected.
fn target_faces(selection_faces: &BTreeSet<u32>, mesh: &Mesh) -> Vec<usize> {
    if selection_faces.is_empty() {
        (0..mesh.faces.len()).collect()
    } else {
        selection_faces.iter().map(|f| *f as usize).collect()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshFlipNormals {}

impl Command for MeshFlipNormals {
    const NAME: &'static str = "mesh.flip_normals";
    const DESCRIPTION: &'static str =
        "Turn the selected faces (all faces when none are selected) to face the other way.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let faces = doc.selection.faces.clone();
        let (_, _, mesh) = editing(doc)?;
        let targets = target_faces(&faces, mesh);
        for f in &targets {
            // Keep the first corner, reverse the rest: same outline, other side.
            mesh.faces[*f].corners[1..].reverse();
        }
        Ok(json!({ "flipped": targets.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshFlipDiagonal {}

impl Command for MeshFlipDiagonal {
    const NAME: &'static str = "mesh.flip_diagonal";
    const DESCRIPTION: &'static str =
        "Split the selected quads along their other diagonal. The outline stays the same.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let faces = doc.selection.faces.clone();
        let (_, _, mesh) = editing(doc)?;
        let mut flipped = 0;
        for f in target_faces(&faces, mesh) {
            let corners = &mut mesh.faces[f].corners;
            if corners.len() == 4 {
                // Triangles are fanned from corner 0; starting at corner 1 uses the other diagonal.
                corners.rotate_left(1);
                flipped += 1;
            }
        }
        Ok(json!({ "flipped": flipped }))
    }
}

// ---- mirror ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAxis {
    X,
    Y,
    Z,
    Off,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshMirrorMode {
    pub axis: MirrorAxis,
}

impl Command for MeshMirrorMode {
    const NAME: &'static str = "mesh.mirror_mode";
    const DESCRIPTION: &'static str = "Live mirror modelling on the edited mesh: moving a vertex on one side moves its twin on the other, across the object's X, Y or Z axis. Off stops it.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let (_, _, mesh) = editing(cx.doc())?;
        mesh.mirror = match self.axis {
            MirrorAxis::X => Some(Axis::X),
            MirrorAxis::Y => Some(Axis::Y),
            MirrorAxis::Z => Some(Axis::Z),
            MirrorAxis::Off => None,
        };
        Ok(json!({ "mirror": mesh.mirror }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshMirrorApply {}

impl Command for MeshMirrorApply {
    const NAME: &'static str = "mesh.mirror_apply";
    const DESCRIPTION: &'static str = "Bake the live mirror: copy the half of the mesh on the + side across the axis, weld the centre, and turn mirror off.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let (_, _, mesh) = editing(cx.doc())?;
        let axis = mesh.mirror.map(axis_index).ok_or_else(|| {
            CommandError::invalid("This mesh has no mirror axis. Call mesh.mirror_mode first.")
        })?;
        // Keep the + side (and the centre), then add its mirror image.
        let keep: Vec<bool> = mesh.positions.iter().map(|p| p[axis] >= -SAME).collect();
        mesh.faces
            .retain(|f| f.corners.iter().all(|c| keep[c.vertex as usize]));
        compact(mesh);
        let count = mesh.positions.len() as u32;
        let mut twin: Vec<u32> = Vec::with_capacity(count as usize);
        for v in 0..count {
            let p = mesh.positions[v as usize];
            if p[axis].abs() < SAME {
                twin.push(v); // centre vertices are shared, not doubled
            } else {
                let w = mesh
                    .weights
                    .get(v as usize)
                    .copied()
                    .unwrap_or(Influences::default());
                mesh.positions.push(mirrored(p, axis));
                if !mesh.weights.is_empty() {
                    mesh.weights.push(w);
                }
                twin.push((mesh.positions.len() - 1) as u32);
            }
        }
        let copies: Vec<Face> = mesh
            .faces
            .iter()
            .map(|f| {
                let mut g = f.clone();
                for c in &mut g.corners {
                    c.vertex = twin[c.vertex as usize];
                }
                g.corners[1..].reverse(); // mirrored faces wind the other way
                g
            })
            .collect();
        mesh.faces.extend(copies);
        mesh.mirror = None;
        Ok(json!({ "vertices": mesh.positions.len(), "faces": mesh.faces.len() }))
    }
}
