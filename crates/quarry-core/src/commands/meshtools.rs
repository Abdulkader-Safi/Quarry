//! Extra mesh tools (FR-MESH-07): inset, subdivide, bevel one edge, knife
//! and edge-loop select.
//! Faces stay triangles or quads, so every result is valid for export.

use super::mesh::{add_vertex, editing, nothing};
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context};
use crate::model::{Corner, Face, Mesh, SelectMode};
use glam::{Vec2, Vec3, Vec4};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<MeshInset>();
    r.register::<MeshSubdivide>();
    r.register::<SelectLoop>();
    r.register::<MeshBevel>();
    r.register::<MeshKnife>();
}

/// Faces must have 3 or 4 corners: split a 5- or 6-corner polygon into a
/// quad plus a triangle or two quads, fanned from its first corner.
fn valid(corners: Vec<Corner>) -> Vec<Vec<Corner>> {
    match corners.len() {
        5 => vec![
            corners[..4].to_vec(),
            vec![corners[0], corners[3], corners[4]],
        ],
        6 => vec![
            corners[..4].to_vec(),
            vec![corners[0], corners[3], corners[4], corners[5]],
        ],
        _ => vec![corners],
    }
}

/// Replace face `f` with `parts`: the first keeps its index, the rest append.
fn replace(mesh: &mut Mesh, f: usize, parts: Vec<Vec<Corner>>) {
    let face = mesh.faces[f].clone();
    let mut parts = parts.into_iter();
    if let Some(first) = parts.next() {
        mesh.faces[f].corners = first;
    }
    for part in parts {
        mesh.faces.push(like(&face, part));
    }
}

fn selected_faces(cx: &mut Context) -> Result<BTreeSet<u32>, CommandError> {
    let sel = &cx.state.doc.selection;
    if sel.mode != SelectMode::Face {
        return Err(CommandError::invalid(
            "This works on faces. Call select.mode with face, then select some.",
        ));
    }
    if sel.faces.is_empty() {
        return Err(nothing("faces"));
    }
    Ok(sel.faces.clone())
}

/// The selected edges of the edited mesh, when in edge mode.
fn selected_edges(cx: &Context) -> Vec<[u32; 2]> {
    let sel = &cx.state.doc.selection;
    if sel.mode != SelectMode::Edge {
        return Vec::new();
    }
    sel.edges.iter().map(|&(a, b)| [a, b]).collect()
}

/// A corner between `a` and `b` at `t`, on vertex `vertex`.
fn mix(a: &Corner, b: &Corner, t: f32, vertex: u32) -> Corner {
    Corner {
        vertex,
        uv: a.uv.lerp(b.uv, t),
        color: a.color.lerp(b.color, t),
    }
}

fn centre(corners: &[Corner], mesh: &Mesh) -> (Vec3, Vec2, Vec4) {
    let n = corners.len() as f32;
    let p = corners
        .iter()
        .map(|c| mesh.positions[c.vertex as usize])
        .sum::<Vec3>()
        / n;
    let uv = corners.iter().map(|c| c.uv).sum::<Vec2>() / n;
    let color = corners.iter().map(|c| c.color).sum::<Vec4>() / n;
    (p, uv, color)
}

fn like(face: &Face, corners: Vec<Corner>) -> Face {
    Face {
        corners,
        ..face.clone()
    }
}

// ---- inset -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshInset {
    /// How far each corner moves toward its face's centre, 0.05 to 0.95 of
    /// the way. Default 0.25.
    pub amount: Option<f32>,
}

impl Command for MeshInset {
    const NAME: &'static str = "mesh.inset";
    const DESCRIPTION: &'static str = "Inset each selected face: a smaller copy in the middle, joined to the old edges by a ring of quads. The inner faces stay selected.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let amount = self.amount.unwrap_or(0.25);
        if !(0.05..=0.95).contains(&amount) {
            return Err(CommandError::invalid("amount must be 0.05 to 0.95."));
        }
        let faces = selected_faces(cx)?;
        let (_, _, mesh) = editing(cx.doc())?;
        let mut ring = 0;
        for &f in &faces {
            let face = mesh.faces[f as usize].clone();
            let (mid, uv, color) = centre(&face.corners, mesh);
            let inner: Vec<Corner> = face
                .corners
                .iter()
                .map(|c| {
                    let p = mesh.positions[c.vertex as usize];
                    let v = add_vertex(mesh, p.lerp(mid, amount), c.vertex);
                    Corner {
                        vertex: v,
                        uv: c.uv.lerp(uv, amount),
                        color: c.color.lerp(color, amount),
                    }
                })
                .collect();
            let n = face.corners.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let quad = vec![face.corners[i], face.corners[j], inner[j], inner[i]];
                mesh.faces.push(like(&face, quad));
                ring += 1;
            }
            mesh.faces[f as usize].corners = inner;
        }
        Ok(json!({ "faces": faces.len(), "ring_faces": ring }))
    }
}

// ---- subdivide -------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshSubdivide {}

impl Command for MeshSubdivide {
    const NAME: &'static str = "mesh.subdivide";
    const DESCRIPTION: &'static str = "Split each selected face in four: quads through their centre and edge midpoints, triangles through their edge midpoints. Shape stays the same; neighbours that were not selected keep their edges.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let faces = selected_faces(cx)?;
        let (_, _, mesh) = editing(cx.doc())?;
        let mut midpoints: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut made = BTreeSet::new();
        for &f in &faces {
            let face = mesh.faces[f as usize].clone();
            let c = &face.corners;
            let n = c.len();
            let mids: Vec<Corner> = (0..n)
                .map(|i| {
                    let (a, b) = (c[i], c[(i + 1) % n]);
                    let key = (a.vertex.min(b.vertex), a.vertex.max(b.vertex));
                    let v = *midpoints.entry(key).or_insert_with(|| {
                        let p = (mesh.positions[a.vertex as usize]
                            + mesh.positions[b.vertex as usize])
                            / 2.0;
                        add_vertex(mesh, p, a.vertex)
                    });
                    mix(&a, &b, 0.5, v)
                })
                .collect();
            let parts: Vec<Vec<Corner>> = if n == 4 {
                let (p, uv, color) = centre(c, mesh);
                let mid = Corner {
                    vertex: add_vertex(mesh, p, c[0].vertex),
                    uv,
                    color,
                };
                (0..4)
                    .map(|i| vec![c[i], mids[i], mid, mids[(i + 3) % 4]])
                    .collect()
            } else {
                vec![
                    vec![c[0], mids[0], mids[2]],
                    vec![mids[0], c[1], mids[1]],
                    vec![mids[2], mids[1], c[2]],
                    vec![mids[0], mids[1], mids[2]],
                ]
            };
            let mut parts = parts.into_iter();
            mesh.faces[f as usize].corners = parts.next().expect("four parts");
            made.insert(f);
            for part in parts {
                mesh.faces.push(like(&face, part));
                made.insert((mesh.faces.len() - 1) as u32);
            }
        }
        cx.doc().selection.faces = made.clone();
        Ok(json!({ "faces": made.len() }))
    }
}

// ---- edge loop -------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectLoop {
    /// An edge of the loop as two vertex indices. Default the selected edge.
    pub edge: Option<[u32; 2]>,
    /// Add to the current edge selection instead of replacing it.
    pub add: Option<bool>,
}

impl Command for SelectLoop {
    const NAME: &'static str = "select.loop";
    const DESCRIPTION: &'static str = "Select the edge loop through an edge of the edited mesh: it runs straight across quads and stops at triangles, poles (vertices without 4 edges) and open borders.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let edge = match (self.edge, selected_edges(cx).as_slice()) {
            (Some(e), _) => e,
            (None, [e]) => *e,
            _ => {
                return Err(CommandError::invalid(
                    "Name an edge, or select exactly one.",
                ));
            }
        };
        let (_, _, mesh) = editing(cx.doc())?;
        let key = |a: u32, b: u32| (a.min(b), a.max(b));
        // Every edge, with the faces on it.
        let mut edge_faces: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
        for (fi, f) in mesh.faces.iter().enumerate() {
            let n = f.corners.len();
            for i in 0..n {
                let e = key(f.corners[i].vertex, f.corners[(i + 1) % n].vertex);
                edge_faces.entry(e).or_default().push(fi);
            }
        }
        let start = key(edge[0], edge[1]);
        if !edge_faces.contains_key(&start) {
            return Err(CommandError::not_found(
                "That edge is not in the mesh. Call query.mesh to list edges.",
            ));
        }
        let at = |v: u32| -> Vec<(u32, u32)> {
            edge_faces
                .keys()
                .filter(|e| e.0 == v || e.1 == v)
                .copied()
                .collect()
        };
        let mut found = BTreeSet::from([start]);
        for (from, through) in [(start.0, start.1), (start.1, start.0)] {
            let (mut prev, mut v) = (from, through);
            loop {
                let edges = at(v);
                let came = key(prev, v);
                let beside = &edge_faces[&came];
                if edges.len() != 4 || beside.iter().any(|f| mesh.faces[*f].corners.len() != 4) {
                    break;
                }
                // The way on is the edge at v that shares no face with the one we came by.
                let next = edges
                    .into_iter()
                    .find(|e| *e != came && edge_faces[e].iter().all(|f| !beside.contains(f)));
                let Some(next) = next else { break };
                if !found.insert(next) {
                    break; // closed loop
                }
                let w = if next.0 == v { next.1 } else { next.0 };
                (prev, v) = (v, w);
            }
        }
        let sel = &mut cx.doc().selection;
        sel.mode = SelectMode::Edge;
        if self.add != Some(true) {
            sel.edges.clear();
        }
        sel.edges.extend(found.iter().copied());
        Ok(json!({ "edges": found.len() }))
    }
}

// ---- bevel -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshBevel {
    /// The edge as two vertex indices. Default the selected edge.
    pub edge: Option<[u32; 2]>,
    /// How far the new edges sit from the old one along each face, in metres.
    /// Default 0.1.
    pub width: Option<f32>,
}

impl Command for MeshBevel {
    const NAME: &'static str = "mesh.bevel";
    const DESCRIPTION: &'static str = "Bevel one edge with one segment: the edge becomes a flat strip `width` metres into each of its two faces. Each end may touch at most one other face.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let width = self.width.unwrap_or(0.1);
        if !(width.is_finite() && width > 0.0) {
            return Err(CommandError::invalid("width must be above 0 metres."));
        }
        let [a, b] = match (self.edge, selected_edges(cx).as_slice()) {
            (Some(e), _) => e,
            (None, [e]) => *e,
            _ => {
                return Err(CommandError::invalid(
                    "Name an edge, or select exactly one.",
                ));
            }
        };
        let (_, _, mesh) = editing(cx.doc())?;
        // F1 runs a then b, F2 runs b then a.
        let runs = |f: &Face, x: u32, y: u32| {
            let n = f.corners.len();
            (0..n).any(|i| f.corners[i].vertex == x && f.corners[(i + 1) % n].vertex == y)
        };
        let f1 = mesh.faces.iter().position(|f| runs(f, a, b));
        let f2 = mesh.faces.iter().position(|f| runs(f, b, a));
        let (Some(f1), Some(f2)) = (f1, f2) else {
            return Err(CommandError::invalid(
                "Bevel needs an edge with a face on each side. Call query.mesh to find one.",
            ));
        };
        // At each end: the vertex slides into F1 and into F2 along their other edge.
        let slide = |mesh: &Mesh,
                     f: usize,
                     v: u32,
                     other: u32|
         -> Result<(Corner, u32, f32), CommandError> {
            let c = &mesh.faces[f].corners;
            let n = c.len();
            let i = c.iter().position(|k| k.vertex == v).expect("on face");
            let (prev, next) = (c[(i + n - 1) % n], c[(i + 1) % n]);
            let toward = if prev.vertex == other { next } else { prev };
            let len = mesh.positions[v as usize].distance(mesh.positions[toward.vertex as usize]);
            if width >= len * 0.95 {
                return Err(CommandError::invalid(format!(
                    "width must be under {:.3} m, the length of the edges it slides along.",
                    len * 0.95
                )));
            }
            Ok((c[i], toward.vertex, width / len))
        };
        let mut moved: BTreeMap<(usize, u32), Corner> = BTreeMap::new();
        let mut ends = Vec::new();
        for (v, other) in [(a, b), (b, a)] {
            let caps: Vec<usize> = (0..mesh.faces.len())
                .filter(|f| *f != f1 && *f != f2)
                .filter(|f| mesh.faces[*f].corners.iter().any(|c| c.vertex == v))
                .collect();
            if caps.len() > 1 {
                return Err(CommandError::invalid(
                    "Bevel works on edges whose ends touch at most three faces.",
                ));
            }
            let mut slid = Vec::new();
            for f in [f1, f2] {
                let (corner, toward, t) = slide(mesh, f, v, other)?;
                let p = mesh.positions[v as usize].lerp(mesh.positions[toward as usize], t);
                let nv = add_vertex(mesh, p, v);
                let toward_corner = mesh.faces[f]
                    .corners
                    .iter()
                    .find(|c| c.vertex == toward)
                    .copied()
                    .expect("neighbour on face");
                moved.insert((f, v), mix(&corner, &toward_corner, t, nv));
                slid.push((nv, toward, t));
            }
            ends.push((v, caps.first().copied(), slid));
        }
        // The cap at each end gets both new vertices in place of the old one.
        for (v, cap, slid) in &ends {
            let Some(cap) = *cap else { continue };
            let c = mesh.faces[cap].corners.clone();
            let n = c.len();
            let i = c.iter().position(|k| k.vertex == *v).expect("on cap");
            let (prev, next) = (c[(i + n - 1) % n], c[(i + 1) % n]);
            let at = |&(nv, toward, t): &(u32, u32, f32)| {
                let end = if prev.vertex == toward { prev } else { next };
                mix(&c[i], &end, t, nv)
            };
            let (in1, in2) = (at(&slid[0]), at(&slid[1]));
            let pair = if prev.vertex == slid[0].1 {
                [in1, in2]
            } else {
                [in2, in1]
            };
            let mut corners = c[..i].to_vec();
            corners.extend(pair);
            corners.extend_from_slice(&c[i + 1..]);
            replace(mesh, cap, valid(corners));
        }
        for f in [f1, f2] {
            for c in &mut mesh.faces[f].corners {
                if let Some(m) = moved.get(&(f, c.vertex)) {
                    *c = *m;
                }
            }
        }
        let strip = vec![
            moved[&(f1, b)],
            moved[&(f1, a)],
            moved[&(f2, a)],
            moved[&(f2, b)],
        ];
        let face = mesh.faces[f1].clone();
        mesh.faces.push(like(&face, strip));
        super::tile::compact(mesh);
        let faces = mesh.faces.len();
        cx.doc().selection.edges.clear();
        Ok(json!({ "faces": faces }))
    }
}

// ---- knife -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EdgePoint {
    /// The edge as two vertex indices.
    pub edge: [u32; 2],
    /// 0 at the first vertex, 1 at the second.
    pub at: f32,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshKnife {
    /// Default the middle of the first of two selected edges.
    pub from: Option<EdgePoint>,
    /// Default the middle of the second of two selected edges.
    pub to: Option<EdgePoint>,
}

impl Command for MeshKnife {
    const NAME: &'static str = "mesh.knife";
    const DESCRIPTION: &'static str = "Cut a face in two along a line between points on two of its edges (at 0 or 1 the cut starts at that vertex). Neighbours sharing a cut edge are split so every face stays a triangle or quad.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let (from, to) = match (self.from, self.to, selected_edges(cx).as_slice()) {
            (Some(f), Some(t), _) => (f, t),
            (None, None, [a, b]) => (
                EdgePoint { edge: *a, at: 0.5 },
                EdgePoint { edge: *b, at: 0.5 },
            ),
            _ => {
                return Err(CommandError::invalid(
                    "Give from and to, or select exactly two edges of one face.",
                ));
            }
        };
        let (_, _, mesh) = editing(cx.doc())?;
        let mut cut = Vec::new();
        for point in [&from, &to] {
            if !(0.0..=1.0).contains(&point.at) {
                return Err(CommandError::invalid("at must be 0 to 1."));
            }
            let [u, v] = point.edge;
            let v = match point.at {
                t if t < 1e-3 => u,
                t if t > 1.0 - 1e-3 => v,
                t => split_edge(mesh, u, v, t)?,
            };
            cut.push(v);
        }
        let (p, q) = (cut[0], cut[1]);
        let face = mesh.faces.iter().position(|f| {
            let has = |x| f.corners.iter().any(|c| c.vertex == x);
            has(p) && has(q)
        });
        let Some(f) = face else {
            return Err(CommandError::invalid(
                "The two points must lie on edges of the same face.",
            ));
        };
        let c = mesh.faces[f].corners.clone();
        let n = c.len();
        let (i, j) = (
            c.iter().position(|k| k.vertex == p).expect("on face"),
            c.iter().position(|k| k.vertex == q).expect("on face"),
        );
        if (i + 1) % n == j || (j + 1) % n == i || i == j {
            return Err(CommandError::invalid(
                "Those points are already joined by an edge; pick points across the face.",
            ));
        }
        let walk = |from: usize, to: usize| {
            let mut out = vec![c[from]];
            let mut k = from;
            while k != to {
                k = (k + 1) % n;
                out.push(c[k]);
            }
            out
        };
        let mut parts = valid(walk(i, j));
        parts.extend(valid(walk(j, i)));
        replace(mesh, f, parts);
        // Neighbours that got a new corner on a cut edge may have 5 now.
        for f in 0..mesh.faces.len() {
            if mesh.faces[f].corners.len() > 4 {
                let corners = mesh.faces[f].corners.clone();
                replace(mesh, f, valid(corners));
            }
        }
        Ok(json!({ "faces": mesh.faces.len(), "cut": [p, q] }))
    }
}

/// Put a new vertex on edge u-v at t, in every face that has that edge.
fn split_edge(mesh: &mut Mesh, u: u32, v: u32, t: f32) -> Result<u32, CommandError> {
    let p = mesh.positions[u as usize].lerp(mesh.positions[v as usize], t);
    let mut new = None;
    for f in 0..mesh.faces.len() {
        let c = mesh.faces[f].corners.clone();
        let n = c.len();
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            let along = if (a.vertex, b.vertex) == (u, v) {
                t
            } else if (a.vertex, b.vertex) == (v, u) {
                1.0 - t
            } else {
                continue;
            };
            let nv = *new.get_or_insert_with(|| add_vertex(mesh, p, u));
            mesh.faces[f].corners.insert(i + 1, mix(&a, &b, along, nv));
            break;
        }
    }
    new.ok_or_else(|| {
        CommandError::not_found("That edge is not in the mesh. Call query.mesh to list edges.")
    })
}
