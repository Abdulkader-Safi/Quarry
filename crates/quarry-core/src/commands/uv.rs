//! UV editing (FR-UV-01, FR-UV-02). UVs are in pixels; 1 m of surface is one
//! tile (tile_px pixels) when unwrapping or projecting.

use super::select::edit_mesh;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::model::{Mesh, SelectMode};
use glam::{Vec2, Vec3};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

const PADDING: f32 = 1.0;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<UvTransform>();
    r.register::<UvMarkSeam>();
    r.register::<UvUnwrap>();
    r.register::<UvProjectView>();
    r.register::<UvSnapPixels>();
}

/// The edited mesh and the faces to work on: the selected faces in face mode,
/// otherwise every face.
fn faces_and_mesh<'a>(
    cx: &'a mut Context<'_>,
) -> Result<(Vec<usize>, glam::Mat4, &'a mut Mesh), CommandError> {
    let doc = cx.doc();
    let (o, mesh) = edit_mesh(doc)?;
    let (id, world) = (o.id, doc.world_matrix(o));
    let faces: Vec<usize> =
        if doc.selection.mode == SelectMode::Face && !doc.selection.faces.is_empty() {
            doc.selection.faces.iter().map(|f| *f as usize).collect()
        } else {
            (0..mesh.faces.len()).collect()
        };
    if faces.is_empty() {
        return Err(CommandError::new(
            ErrorCode::NothingSelected,
            "The mesh has no faces.",
        ));
    }
    let mesh = Arc::make_mut(
        doc.objects
            .get_mut(id)
            .expect("edited")
            .mesh
            .as_mut()
            .expect("mesh"),
    );
    Ok((faces, world, mesh))
}

/// Size of the image the edited mesh's first textured face uses, if any.
fn image_size(cx: &Context) -> Option<Vec2> {
    let doc = &cx.state.doc;
    let (_, mesh) = edit_mesh(doc).ok()?;
    let image = mesh.faces.iter().find_map(|f| f.image)?;
    doc.images
        .get(image)
        .map(|i| Vec2::new(i.size[0] as f32, i.size[1] as f32))
}

fn tile_px(cx: &Context) -> f32 {
    cx.state.doc.settings.tile_px as f32
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlipUv {
    /// Left-right.
    U,
    /// Top-bottom.
    V,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UvTransform {
    /// Move by [u, v] pixels.
    pub translate: Option<[f32; 2]>,
    /// Rotate counter-clockwise by degrees about the faces' UV centre.
    pub rotate: Option<f32>,
    /// Scale by [u, v] about the UV centre.
    pub scale: Option<[f32; 2]>,
    pub flip: Option<FlipUv>,
}

impl Command for UvTransform {
    const NAME: &'static str = "uv.transform";
    const DESCRIPTION: &'static str = "Move, rotate, scale or flip the UVs of the selected faces (all faces when none are selected), in pixels.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let t = Vec2::from(self.translate.unwrap_or([0.0; 2]));
        let s = Vec2::from(self.scale.unwrap_or([1.0; 2]));
        let angle = self.rotate.unwrap_or(0.0).to_radians();
        if !(t.is_finite() && s.is_finite() && angle.is_finite()) {
            return Err(CommandError::invalid(
                "translate, rotate and scale must be finite numbers.",
            ));
        }
        let (faces, _, mesh) = faces_and_mesh(cx)?;
        let uvs: Vec<Vec2> = faces
            .iter()
            .flat_map(|f| mesh.faces[*f].corners.iter().map(|c| c.uv))
            .collect();
        let centre = uvs.iter().sum::<Vec2>() / uvs.len() as f32;
        // Image v points down, so a counter-clockwise turn on screen is clockwise in UV.
        let rot = Vec2::from_angle(-angle);
        for f in &faces {
            for c in &mut mesh.faces[*f].corners {
                let mut d = c.uv - centre;
                match self.flip {
                    Some(FlipUv::U) => d.x = -d.x,
                    Some(FlipUv::V) => d.y = -d.y,
                    None => {}
                }
                c.uv = centre + rot.rotate(d * s) + t;
            }
        }
        Ok(json!({ "faces": faces.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UvMarkSeam {
    /// Clear seams instead of marking them.
    #[serde(default)]
    pub clear: bool,
}

impl Command for UvMarkSeam {
    const NAME: &'static str = "uv.mark_seam";
    const DESCRIPTION: &'static str =
        "Mark (or clear) the selected edges as UV seams. Unwrap cuts the mesh open along seams.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let edges = doc.selection.edges.clone();
        if doc.selection.mode != SelectMode::Edge || edges.is_empty() {
            return Err(CommandError::new(
                ErrorCode::NothingSelected,
                "No edges selected. Call select.mode with edge, then select edges.",
            ));
        }
        let (o, _) = edit_mesh(doc)?;
        let id = o.id;
        let mesh = Arc::make_mut(
            doc.objects
                .get_mut(id)
                .expect("edited")
                .mesh
                .as_mut()
                .expect("mesh"),
        );
        for e in &edges {
            if self.clear {
                mesh.seams.remove(e);
            } else {
                mesh.seams.insert(*e);
            }
        }
        Ok(json!({ "seams": mesh.seams.len() }))
    }
}

fn edge(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Lay one face flat with corners `a`, `b` pinned at `pa`, `pb` (2D), the
/// face's interior to the right of a→b when `a→b` runs against its winding.
fn place(mesh: &Mesh, face: usize, a: u32, b: u32, pa: Vec2, pb: Vec2) -> BTreeMap<u32, Vec2> {
    let corners = &mesh.faces[face].corners;
    let p = |v: u32| mesh.positions[v as usize];
    let normal = {
        let q: Vec<Vec3> = corners.iter().map(|c| p(c.vertex)).collect();
        let mut n = Vec3::ZERO;
        for i in 0..q.len() {
            n += q[i].cross(q[(i + 1) % q.len()]);
        }
        n.normalize_or(Vec3::Y)
    };
    let x3 = (p(b) - p(a)).normalize_or(Vec3::X);
    let y3 = normal.cross(x3);
    let x2 = (pb - pa).normalize_or(Vec2::X);
    let y2 = x2.perp();
    let scale = (pb - pa).length() / (p(b) - p(a)).length().max(1e-9);
    corners
        .iter()
        .map(|c| {
            let d = p(c.vertex) - p(a);
            (c.vertex, pa + (x2 * d.dot(x3) + y2 * d.dot(y3)) * scale)
        })
        .collect()
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UvUnwrap {}

impl Command for UvUnwrap {
    const NAME: &'static str = "uv.unwrap";
    const DESCRIPTION: &'static str = "Unfold the selected faces (all faces when none are selected) flat, cutting along seams, at 1 m per tile, and lay the pieces out without overlap, shrunk to fit the texture if needed.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let px = tile_px(cx);
        let fit = image_size(cx);
        let (faces, _, mesh) = faces_and_mesh(cx)?;
        let chosen: BTreeSet<usize> = faces.iter().copied().collect();
        let mut owners: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
        for f in &faces {
            let c = &mesh.faces[*f].corners;
            for k in 0..c.len() {
                owners
                    .entry(edge(c[k].vertex, c[(k + 1) % c.len()].vertex))
                    .or_default()
                    .push(*f);
            }
        }

        // Islands: faces joined across shared edges that are not seams. Each
        // face is unfolded into its neighbour's plane, hinged on that edge.
        let mut placed: BTreeMap<usize, BTreeMap<u32, Vec2>> = BTreeMap::new();
        let mut islands: Vec<Vec<usize>> = Vec::new();
        for &start in &faces {
            if placed.contains_key(&start) {
                continue;
            }
            let first = &mesh.faces[start].corners;
            let (a, b) = (first[0].vertex, first[1].vertex);
            let len = mesh.positions[b as usize].distance(mesh.positions[a as usize]);
            placed.insert(
                start,
                place(mesh, start, a, b, Vec2::ZERO, Vec2::new(len, 0.0)),
            );
            let mut island = vec![start];
            let mut queue = VecDeque::from([start]);
            while let Some(f) = queue.pop_front() {
                let c = mesh.faces[f].corners.clone();
                for k in 0..c.len() {
                    let (a, b) = (c[k].vertex, c[(k + 1) % c.len()].vertex);
                    let e = edge(a, b);
                    if mesh.seams.contains(&e) {
                        continue;
                    }
                    for &g in owners.get(&e).into_iter().flatten() {
                        if g == f
                            || placed.contains_key(&g)
                            || !chosen.contains(&g)
                            || owners[&e].len() != 2
                        {
                            continue;
                        }
                        let (pa, pb) = (placed[&f][&a], placed[&f][&b]);
                        // The neighbour runs this edge b→a; pin it the same way round.
                        placed.insert(g, place(mesh, g, b, a, pb, pa));
                        island.push(g);
                        queue.push_back(g);
                    }
                }
            }
            islands.push(island);
        }

        // Up the surface is up the image, whose rows run down.
        let to_px = |p: Vec2| Vec2::new(p.x, -p.y) * px;
        // Shelf-pack islands in pixels with 1 px between them.
        let boxes: Vec<(Vec2, Vec2)> = islands
            .iter()
            .map(|island| {
                island
                    .iter()
                    .flat_map(|f| placed[f].values())
                    .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| {
                        (lo.min(to_px(*p)), hi.max(to_px(*p)))
                    })
            })
            .collect();
        let area: f32 = boxes
            .iter()
            .map(|(lo, hi)| (hi.x - lo.x + PADDING) * (hi.y - lo.y + PADDING))
            .sum();
        let widest = boxes.iter().map(|(lo, hi)| hi.x - lo.x).fold(0.0, f32::max);
        let row_limit = (area.sqrt() * 1.3).max(widest);
        let mut order: Vec<usize> = (0..islands.len()).collect();
        order.sort_by(|a, b| {
            (boxes[*b].1.y - boxes[*b].0.y).total_cmp(&(boxes[*a].1.y - boxes[*a].0.y))
        });
        let (mut x, mut y, mut row_h) = (0.0f32, 0.0f32, 0.0f32);
        let mut offset = vec![Vec2::ZERO; islands.len()];
        for i in order {
            let (lo, hi) = boxes[i];
            let size = hi - lo;
            if x > 0.0 && x + size.x > row_limit {
                (x, y, row_h) = (0.0, y + row_h + PADDING, 0.0);
            }
            offset[i] = Vec2::new(x, y) - lo;
            x += size.x + PADDING;
            row_h = row_h.max(size.y);
        }
        // Shrink the whole layout to fit the faces' image if it is too big.
        let extent = (0..islands.len()).fold(Vec2::ZERO, |m, i| m.max(boxes[i].1 + offset[i]));
        let shrink = fit.map_or(1.0, |size| {
            (size / extent.max(Vec2::ONE)).min_element().min(1.0)
        });
        for (i, island) in islands.iter().enumerate() {
            for f in island {
                for c in &mut mesh.faces[*f].corners {
                    c.uv = (to_px(placed[f][&c.vertex]) + offset[i]) * shrink;
                }
            }
        }
        Ok(json!({ "faces": faces.len(), "islands": islands.len(), "scale": shrink }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UvProjectView {}

impl Command for UvProjectView {
    const NAME: &'static str = "uv.project_view";
    const DESCRIPTION: &'static str = "Set the selected faces' UVs (all faces when none are selected) to how they look from the current camera, at 1 m per tile.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let px = tile_px(cx);
        let view = cx.state.doc.view.camera.view();
        let (faces, world, mesh) = faces_and_mesh(cx)?;
        let to_view = view * world;
        let project = |p: Vec3| {
            let v = to_view.transform_point3(p);
            Vec2::new(v.x, -v.y) * px
        };
        let points: Vec<Vec2> = faces
            .iter()
            .flat_map(|f| {
                mesh.faces[*f]
                    .corners
                    .iter()
                    .map(|c| project(mesh.positions[c.vertex as usize]))
            })
            .collect();
        let lo = points.iter().fold(Vec2::MAX, |a, b| a.min(*b));
        for f in &faces {
            let corners: Vec<(usize, u32)> = mesh.faces[*f]
                .corners
                .iter()
                .map(|c| c.vertex)
                .enumerate()
                .collect();
            for (k, v) in corners {
                mesh.faces[*f].corners[k].uv = project(mesh.positions[v as usize]) - lo;
            }
        }
        Ok(json!({ "faces": faces.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UvSnapPixels {}

impl Command for UvSnapPixels {
    const NAME: &'static str = "uv.snap_pixels";
    const DESCRIPTION: &'static str =
        "Round the selected faces' UVs (all faces when none are selected) to whole pixels.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let (faces, _, mesh) = faces_and_mesh(cx)?;
        for f in &faces {
            for c in &mut mesh.faces[*f].corners {
                c.uv = c.uv.round();
            }
        }
        Ok(json!({ "faces": faces.len() }))
    }
}
