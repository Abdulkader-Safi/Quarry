//! Vertex colours (FR-PAINT-05) and ambient occlusion baked into them. Colours
//! live on face corners; painting a vertex sets every corner on it. They
//! multiply over the texture in the view and export as COLOR_0.

use super::mesh::{editing, nothing};
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context};
use crate::model::SelectMode;
use glam::{Vec3, Vec4};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<PaintVertexColor>();
    r.register::<PaintBakeAo>();
}

/// sRGB 0..255 to linear 0..1.
fn linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintVertexColor {
    /// [r, g, b, a] in sRGB 0 to 255.
    pub rgba: [u8; 4],
    /// World points of a brush stroke: vertices within `radius` of any of
    /// them are painted. Default the selection (selected vertices, the
    /// vertices of selected faces or edges, or the whole edited mesh).
    pub points: Option<Vec<[f32; 3]>>,
    /// Brush radius in metres. Default 0.25.
    pub radius: Option<f32>,
}

impl Command for PaintVertexColor {
    const NAME: &'static str = "paint.vertex_color";
    const DESCRIPTION: &'static str = "Paint vertex colours on the edited mesh: along a stroke of world points, or on the selection. Colours multiply over the texture (turn on view.display vertex_color to see them) and export as COLOR_0.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let radius = self.radius.unwrap_or(0.25);
        if !(radius.is_finite() && radius > 0.0) {
            return Err(CommandError::invalid("radius must be above 0 metres."));
        }
        let sel = cx.state.doc.selection.clone();
        let (_, world, mesh) = editing(cx.doc())?;
        let targets: BTreeSet<u32> = match &self.points {
            Some(points) => {
                let points: Vec<Vec3> = points.iter().map(|p| Vec3::from(*p)).collect();
                (0..mesh.positions.len() as u32)
                    .filter(|v| {
                        let p = world.transform_point3(mesh.positions[*v as usize]);
                        points.iter().any(|q| q.distance(p) <= radius)
                    })
                    .collect()
            }
            None => match sel.mode {
                SelectMode::Vertex if !sel.vertices.is_empty() => sel.vertices.clone(),
                SelectMode::Edge if !sel.edges.is_empty() => {
                    sel.edges.iter().flat_map(|e| [e.0, e.1]).collect()
                }
                SelectMode::Face if !sel.faces.is_empty() => sel
                    .faces
                    .iter()
                    .flat_map(|f| mesh.faces[*f as usize].corners.iter().map(|c| c.vertex))
                    .collect(),
                _ => (0..mesh.positions.len() as u32).collect(),
            },
        };
        if targets.is_empty() {
            return Err(nothing("vertices under the brush"));
        }
        let [r, g, b, a] = self.rgba;
        let color = Vec4::new(linear(r), linear(g), linear(b), a as f32 / 255.0);
        for c in mesh.faces.iter_mut().flat_map(|f| &mut f.corners) {
            if targets.contains(&c.vertex) {
                c.color = color;
            }
        }
        Ok(json!({ "vertices": targets.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintBakeAo {
    /// Rays per vertex, 8 to 256. Default 64.
    pub rays: Option<u32>,
    /// How far a blocker still darkens, in metres. Default 1.
    pub distance: Option<f32>,
    /// 0 to 1: how dark a fully hidden vertex gets. Default 0.8.
    pub strength: Option<f32>,
}

impl Command for PaintBakeAo {
    const NAME: &'static str = "paint.bake_ao";
    const DESCRIPTION: &'static str = "Bake ambient occlusion into the edited mesh's vertex colours: vertices hidden by nearby geometry (any visible mesh) get darker. Multiplies over the current colours; undo to bake again. Long: reports progress, control.cancel stops it.";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        let rays = self.rays.unwrap_or(64).clamp(8, 256) as usize;
        let reach = self.distance.unwrap_or(1.0);
        let strength = self.strength.unwrap_or(0.8).clamp(0.0, 1.0);
        if !(reach.is_finite() && reach > 0.0) {
            return Err(CommandError::invalid("distance must be above 0 metres."));
        }
        // Every visible triangle in world space, the edited mesh included.
        let doc = &cx.state.doc;
        let mut tris: Vec<[Vec3; 3]> = Vec::new();
        for o in doc.objects.iter().filter(|o| o.visible) {
            let Some(mesh) = doc.drawn_mesh(o) else {
                continue;
            };
            let m = doc.world_matrix(o);
            let p: Vec<Vec3> = mesh
                .positions
                .iter()
                .map(|v| m.transform_point3(*v))
                .collect();
            for f in mesh.faces.iter().filter(|f| !f.hidden) {
                let c = &f.corners;
                for k in 1..c.len().saturating_sub(1) {
                    tris.push([
                        p[c[0].vertex as usize],
                        p[c[k].vertex as usize],
                        p[c[k + 1].vertex as usize],
                    ]);
                }
            }
        }
        // Owned copies, so the mesh is not borrowed while `cx.step` runs.
        let (points, normals) = {
            let (_, world, mesh) = editing(cx.doc())?;
            let points: Vec<Vec3> = mesh
                .positions
                .iter()
                .map(|v| world.transform_point3(*v))
                .collect();
            // A vertex's normal: the sum of its faces' normals.
            let mut normals = vec![Vec3::ZERO; points.len()];
            for (f, n) in mesh.faces.iter().zip(mesh.corner_normals(&points)) {
                for (c, n) in f.corners.iter().zip(n) {
                    normals[c.vertex as usize] += n;
                }
            }
            (points, normals)
        };
        // Evenly spread directions (Fibonacci sphere), flipped into each hemisphere.
        let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
        let dirs: Vec<Vec3> = (0..rays)
            .map(|i| {
                let y = 1.0 - (i as f32 + 0.5) / rays as f32 * 2.0;
                let r = (1.0 - y * y).sqrt();
                let a = golden * i as f32;
                Vec3::new(r * a.cos(), y, r * a.sin())
            })
            .collect();
        let mut shade = vec![1.0f32; points.len()];
        // ponytail: every ray tests every triangle, O(vertices x rays x
        // triangles); fine for low-poly props, a BVH when scenes grow.
        for (v, p) in points.iter().enumerate() {
            let n = normals[v].normalize_or(Vec3::Y);
            let origin = *p + n * 1e-3;
            let hidden = dirs
                .iter()
                .map(|d| if d.dot(n) < 0.0 { -*d } else { *d })
                .filter(|d| {
                    tris.iter()
                        .any(|t| hit(origin, *d, *t).is_some_and(|t| t <= reach))
                })
                .count();
            // Cosine-free average: half the hemisphere hidden is half dark.
            shade[v] = 1.0 - strength * hidden as f32 / rays as f32;
            if v % 64 == 0 {
                cx.step(v as f32 / points.len() as f32)?;
            }
        }
        let (_, _, mesh) = editing(cx.doc())?;
        for c in mesh.faces.iter_mut().flat_map(|f| &mut f.corners) {
            let s = shade[c.vertex as usize];
            c.color = Vec4::new(c.color.x * s, c.color.y * s, c.color.z * s, c.color.w);
        }
        let darkest = shade.iter().copied().fold(1.0, f32::min);
        Ok(json!({ "vertices": points.len(), "darkest": darkest }))
    }
}

/// Möller-Trumbore, both sides; the distance along `d` to the hit.
fn hit(o: Vec3, d: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
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
    (t > 1e-4).then_some(t)
}
