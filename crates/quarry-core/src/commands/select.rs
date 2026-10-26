//! Selection (FR-MESH-02). Selecting never enters undo history; undo brings
//! back whatever was selected alongside the mesh it restores.

use super::object;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{Document, Mesh, Object, SelectMode, Selection};
use glam::{Vec2, Vec3};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<SelectModeCmd>();
    r.register::<SelectByIds>();
    r.register::<SelectBox>();
    r.register::<SelectAll>();
    r.register::<SelectNone>();
    r.register::<SelectInvert>();
    r.register::<SelectByImage>();
    r.register::<SelectByNormal>();
    r.register::<QuerySelection>();
}

/// The mesh object being edited, for element modes.
pub(super) fn edit_mesh(doc: &Document) -> Result<(&Object, &Mesh), CommandError> {
    let none = || {
        CommandError::new(
            ErrorCode::NothingSelected,
            "No mesh is being edited. Call select.mode with vertex, edge or face and an object.",
        )
    };
    let o = doc
        .selection
        .object
        .and_then(|id| doc.objects.get(id))
        .ok_or_else(none)?;
    let mesh = o.mesh.as_deref().ok_or_else(none)?;
    Ok((o, mesh))
}

fn edge(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Every edge of a mesh, each once.
pub(super) fn edges(mesh: &Mesh) -> BTreeSet<(u32, u32)> {
    mesh.faces
        .iter()
        .flat_map(|f| {
            let n = f.corners.len();
            (0..n).map(move |k| edge(f.corners[k].vertex, f.corners[(k + 1) % n].vertex))
        })
        .collect()
}

/// Pick what `wanted` names in the current mode, adding to or replacing the selection.
fn apply(sel: &mut Selection, mode: SelectMode, pick: Picked, add: bool, remove: bool) {
    if !add && !remove {
        sel.objects.clear();
        sel.vertices.clear();
        sel.edges.clear();
        sel.faces.clear();
    }
    macro_rules! merge {
        ($set:expr, $items:expr) => {
            for item in $items {
                if remove {
                    $set.remove(&item);
                } else {
                    $set.insert(item);
                }
            }
        };
    }
    match mode {
        SelectMode::Object => merge!(sel.objects, pick.objects),
        SelectMode::Vertex => merge!(sel.vertices, pick.vertices),
        SelectMode::Edge => merge!(sel.edges, pick.edges),
        SelectMode::Face => merge!(sel.faces, pick.faces),
    }
}

#[derive(Default)]
struct Picked {
    objects: Vec<Id<Object>>,
    vertices: Vec<u32>,
    edges: Vec<(u32, u32)>,
    faces: Vec<u32>,
}

fn summary(doc: &Document) -> Value {
    let s = &doc.selection;
    json!({
        "mode": s.mode,
        "object": s.object,
        "objects": s.objects.len(),
        "vertices": s.vertices.len(),
        "edges": s.edges.len(),
        "faces": s.faces.len(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectModeCmd {
    /// object, vertex, edge or face.
    pub mode: SelectMode,
    /// The mesh object to edit in vertex, edge or face mode. Default the one
    /// selected object, or the one already being edited.
    pub object: Option<String>,
}

impl Command for SelectModeCmd {
    const NAME: &'static str = "select.mode";
    const DESCRIPTION: &'static str =
        "Switch between selecting objects and selecting a mesh's vertices, edges or faces.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let target = match (&self.object, self.mode) {
            (_, SelectMode::Object) => None,
            (Some(r), _) => Some(object(doc, r)?),
            (None, _) => doc.selection.object.or_else(|| {
                (doc.selection.objects.len() == 1)
                    .then(|| *doc.selection.objects.first().expect("one"))
            }),
        };
        if self.mode != SelectMode::Object {
            let id = target.ok_or_else(|| {
                CommandError::new(
                    ErrorCode::NothingSelected,
                    "Name the mesh object to edit, or select exactly one object first.",
                )
            })?;
            if doc.objects.get(id).and_then(|o| o.mesh.as_ref()).is_none() {
                return Err(CommandError::invalid(
                    "That object has no mesh to edit. Pick a mesh object.",
                ));
            }
        }
        let sel = &mut cx.doc().selection;
        let same_object = sel.object == target;
        sel.mode = self.mode;
        if self.mode == SelectMode::Object {
            if let Some(id) = sel.object.take() {
                sel.objects = BTreeSet::from([id]);
            }
        } else {
            sel.object = target;
        }
        if !same_object {
            sel.vertices.clear();
            sel.edges.clear();
            sel.faces.clear();
        }
        Ok(summary(&cx.state.doc))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectByIds {
    /// Object IDs or names (object mode).
    #[serde(default)]
    pub objects: Vec<String>,
    /// Vertex indices (vertex mode).
    #[serde(default)]
    pub vertices: Vec<u32>,
    /// Edges as vertex index pairs (edge mode).
    #[serde(default)]
    pub edges: Vec<[u32; 2]>,
    /// Face indices (face mode).
    #[serde(default)]
    pub faces: Vec<u32>,
    /// Add to the selection instead of replacing it.
    #[serde(default)]
    pub add: bool,
    /// Remove these from the selection.
    #[serde(default)]
    pub remove: bool,
}

impl Command for SelectByIds {
    const NAME: &'static str = "select.by_ids";
    const DESCRIPTION: &'static str = "Select objects, or vertices, edges or faces of the edited mesh, by ID or index. Indices come from query.mesh.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let mode = doc.selection.mode;
        let mut pick = Picked::default();
        if mode == SelectMode::Object {
            pick.objects = self
                .objects
                .iter()
                .map(|r| object(doc, r))
                .collect::<Result<_, _>>()?;
        } else {
            let (o, mesh) = edit_mesh(doc)?;
            let (v, f) = (mesh.positions.len() as u32, mesh.faces.len() as u32);
            let known = edges(mesh);
            let bad = |what: &str, n: u32| {
                CommandError::new(
                    ErrorCode::OutOfBounds,
                    format!(
                        "'{}' has {n} {what}. Check the indices with query.mesh.",
                        o.name
                    ),
                )
            };
            if self.vertices.iter().any(|i| *i >= v) {
                return Err(bad("vertices", v));
            }
            if self.faces.iter().any(|i| *i >= f) {
                return Err(bad("faces", f));
            }
            pick.vertices = self.vertices;
            pick.faces = self.faces;
            pick.edges = self.edges.iter().map(|[a, b]| edge(*a, *b)).collect();
            if pick.edges.iter().any(|e| !known.contains(e)) {
                return Err(CommandError::not_found(
                    "One of those edges is not an edge of the mesh.",
                ));
            }
        }
        apply(&mut cx.doc().selection, mode, pick, self.add, self.remove);
        Ok(summary(&cx.state.doc))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectBox {
    /// Rectangle in viewport fractions [left, top, right, bottom], 0..1 from the top-left.
    pub rect: [f32; 4],
    /// Viewport width / height. Default 1.6 (1600 x 1000).
    pub aspect: Option<f32>,
    #[serde(default)]
    pub add: bool,
    #[serde(default)]
    pub remove: bool,
}

impl Command for SelectBox {
    const NAME: &'static str = "select.box";
    const DESCRIPTION: &'static str = "Select everything whose screen position falls inside a rectangle of the current view: object origins in object mode, vertices, edge midpoints or face centres otherwise.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let aspect = self.aspect.unwrap_or(1.6);
        if !(aspect.is_finite() && aspect > 0.0) {
            return Err(CommandError::invalid("aspect must be a positive number."));
        }
        let [l, t, r, b] = self.rect;
        let (lo, hi) = (Vec2::new(l.min(r), t.min(b)), Vec2::new(l.max(r), t.max(b)));
        let view_proj = doc.view.camera.view_proj(aspect);
        let inside = |p: Vec3| {
            let clip = view_proj * p.extend(1.0);
            if clip.w <= 0.0 {
                return false;
            }
            let ndc = clip.truncate() / clip.w;
            let s = Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
            s.cmpge(lo).all() && s.cmple(hi).all()
        };
        let mode = doc.selection.mode;
        let mut pick = Picked::default();
        if mode == SelectMode::Object {
            pick.objects = doc
                .objects
                .iter()
                .filter(|o| o.visible && !o.locked)
                .filter(|o| inside(doc.world_matrix(o).transform_point3(Vec3::ZERO)))
                .map(|o| o.id)
                .collect();
        } else {
            let (o, mesh) = edit_mesh(doc)?;
            let world = doc.world_matrix(o);
            let at = |v: u32| world.transform_point3(mesh.positions[v as usize]);
            match mode {
                SelectMode::Vertex => {
                    pick.vertices = (0..mesh.positions.len() as u32)
                        .filter(|v| inside(at(*v)))
                        .collect();
                }
                SelectMode::Edge => {
                    pick.edges = edges(mesh)
                        .into_iter()
                        .filter(|(a, b)| inside((at(*a) + at(*b)) * 0.5))
                        .collect();
                }
                _ => {
                    pick.faces = (0..mesh.faces.len() as u32)
                        .filter(|f| {
                            let c = &mesh.faces[*f as usize].corners;
                            inside(c.iter().map(|k| at(k.vertex)).sum::<Vec3>() / c.len() as f32)
                        })
                        .collect();
                }
            }
        }
        apply(&mut cx.doc().selection, mode, pick, self.add, self.remove);
        Ok(summary(&cx.state.doc))
    }
}

fn everything(doc: &Document) -> Result<Picked, CommandError> {
    let mut pick = Picked::default();
    match doc.selection.mode {
        SelectMode::Object => {
            pick.objects = doc
                .objects
                .iter()
                .filter(|o| o.visible && !o.locked)
                .map(|o| o.id)
                .collect();
        }
        mode => {
            let (_, mesh) = edit_mesh(doc)?;
            match mode {
                SelectMode::Vertex => pick.vertices = (0..mesh.positions.len() as u32).collect(),
                SelectMode::Edge => pick.edges = edges(mesh).into_iter().collect(),
                _ => pick.faces = (0..mesh.faces.len() as u32).collect(),
            }
        }
    }
    Ok(pick)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectAll {}

impl Command for SelectAll {
    const NAME: &'static str = "select.all";
    const DESCRIPTION: &'static str =
        "Select everything in the current mode (visible, unlocked objects in object mode).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let pick = everything(&cx.state.doc)?;
        let mode = cx.state.doc.selection.mode;
        apply(&mut cx.doc().selection, mode, pick, false, false);
        Ok(summary(&cx.state.doc))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectNone {}

impl Command for SelectNone {
    const NAME: &'static str = "select.none";
    const DESCRIPTION: &'static str = "Deselect everything in the current mode.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let mode = cx.state.doc.selection.mode;
        apply(
            &mut cx.doc().selection,
            mode,
            Picked::default(),
            false,
            false,
        );
        Ok(summary(&cx.state.doc))
    }
}

fn flip<T: Ord>(set: &mut BTreeSet<T>, all: Vec<T>) {
    *set = all.into_iter().filter(|x| !set.contains(x)).collect();
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectInvert {}

impl Command for SelectInvert {
    const NAME: &'static str = "select.invert";
    const DESCRIPTION: &'static str = "Select what is not selected, and deselect what is.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let all = everything(&cx.state.doc)?;
        let sel = &mut cx.doc().selection;
        match sel.mode {
            SelectMode::Object => flip(&mut sel.objects, all.objects),
            SelectMode::Vertex => flip(&mut sel.vertices, all.vertices),
            SelectMode::Edge => flip(&mut sel.edges, all.edges),
            SelectMode::Face => flip(&mut sel.faces, all.faces),
        }
        Ok(summary(&cx.state.doc))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectByImage {
    /// Image ID or name.
    pub image: String,
    #[serde(default)]
    pub add: bool,
}

impl Command for SelectByImage {
    const NAME: &'static str = "select.by_image";
    const DESCRIPTION: &'static str =
        "Select the edited mesh's faces that use an image (switches to face mode).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let image = doc.images.find(&self.image).map(|i| i.id).ok_or_else(|| {
            CommandError::not_found(format!(
                "No image called '{}'. Call query.images.",
                self.image
            ))
        })?;
        let (_, mesh) = edit_mesh(doc)?;
        let pick = Picked {
            faces: (0..mesh.faces.len() as u32)
                .filter(|f| mesh.faces[*f as usize].image == Some(image))
                .collect(),
            ..Picked::default()
        };
        let sel = &mut cx.doc().selection;
        sel.mode = SelectMode::Face;
        apply(sel, SelectMode::Face, pick, self.add, false);
        Ok(summary(&cx.state.doc))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectByNormal {
    /// World direction [x, y, z], such as [0, 1, 0] for faces pointing up.
    pub direction: [f32; 3],
    /// Largest angle from that direction, in degrees. Default 10.
    pub angle: Option<f32>,
    #[serde(default)]
    pub add: bool,
}

impl Command for SelectByNormal {
    const NAME: &'static str = "select.by_normal";
    const DESCRIPTION: &'static str = "Select the edited mesh's faces that point within an angle of a direction (switches to face mode).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let dir = Vec3::from(self.direction).normalize_or_zero();
        if dir == Vec3::ZERO || !dir.is_finite() {
            return Err(CommandError::invalid(
                "direction must be a non-zero vector.",
            ));
        }
        let limit = self
            .angle
            .unwrap_or(10.0)
            .clamp(0.0, 180.0)
            .to_radians()
            .cos();
        let (o, mesh) = edit_mesh(doc)?;
        let world = doc.world_matrix(o);
        let pick = Picked {
            faces: (0..mesh.faces.len() as u32)
                .filter(|f| {
                    let p: Vec<Vec3> = mesh.faces[*f as usize]
                        .corners
                        .iter()
                        .map(|c| world.transform_point3(mesh.positions[c.vertex as usize]))
                        .collect();
                    let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                    n.dot(dir) >= limit - 1e-6
                })
                .collect(),
            ..Picked::default()
        };
        let sel = &mut cx.doc().selection;
        sel.mode = SelectMode::Face;
        apply(sel, SelectMode::Face, pick, self.add, false);
        Ok(summary(&cx.state.doc))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuerySelection {}

impl Command for QuerySelection {
    const NAME: &'static str = "query.selection";
    const DESCRIPTION: &'static str = "The selection: mode, edited object, counts, the selected IDs or indices, and their world bounds.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let s = &doc.selection;
        let mut reply = summary(doc);
        reply["object_ids"] = json!(s.objects);
        reply["vertex_ids"] = json!(s.vertices);
        reply["edge_ids"] = json!(s.edges);
        reply["face_ids"] = json!(s.faces);
        let bounds = match s.mode {
            SelectMode::Object => {
                doc.bounds(s.objects.iter().filter_map(|id| doc.objects.get(*id)))
            }
            _ => edit_mesh(doc).ok().and_then(|(o, mesh)| {
                let world = doc.world_matrix(o);
                let points: Vec<Vec3> = s
                    .touched_vertices(mesh)
                    .iter()
                    .map(|v| world.transform_point3(mesh.positions[*v as usize]))
                    .collect();
                let first = *points.first()?;
                Some(
                    points
                        .iter()
                        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))),
                )
            }),
        };
        reply["bounds"] = json!(bounds.map(|(lo, hi)| [lo, hi]));
        Ok(reply)
    }
}
