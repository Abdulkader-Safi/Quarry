//! Command structs. Each is one MCP tool; descriptions are what the agent reads.

use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::{Id, Keyed};
use crate::model::{Corner, Document, Face, Filter, Mesh, Object, ObjectKind, ProjectSettings};
use glam::{Vec2, Vec3, Vec4};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn register_all(r: &mut CommandRegistry) {
    r.register::<ProjectNew>();
    r.register::<ProjectOpen>();
    r.register::<ProjectRecover>();
    r.register::<ProjectSave>();
    r.register::<ProjectSaveAs>();
    r.register::<ProjectInfo>();
    r.register::<ProjectSetSettings>();
    r.register::<HistoryUndo>();
    r.register::<HistoryRedo>();
    r.register::<ControlCancel>();
    r.register::<QueryScene>();
    r.register::<QueryHistory>();
    r.register::<SceneRename>();
    r.register::<SceneVisibility>();
    r.register::<SceneLock>();
    r.register::<SceneDelete>();
    r.register::<MeshAddPrimitive>();
}

/// Resolve an object by ID ("o3g1") or name.
fn object(doc: &Document, reference: &str) -> Result<Id<Object>, CommandError> {
    doc.objects.find(reference).map(Keyed::id).ok_or_else(|| {
        CommandError::not_found(format!(
            "No object called '{reference}'. Call query.scene to list objects."
        ))
    })
}

fn objects(doc: &Document, references: &[String]) -> Result<Vec<Id<Object>>, CommandError> {
    if references.is_empty() {
        return Err(CommandError::invalid("Name at least one object."));
    }
    references.iter().map(|r| object(doc, r)).collect()
}

// ---- project ---------------------------------------------------------------

/// Settings to change; anything left out keeps its value.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    /// Tile size in pixels; 1 tile is 1 metre. Default 16.
    pub tile_px: Option<u32>,
    /// Texture filtering. Default nearest (pixel art).
    pub filter: Option<Filter>,
    /// Frames per second for animation sampling. Default 30.
    pub fps: Option<u32>,
    /// Snap painted colours to the palette. Default true.
    pub palette_lock: Option<bool>,
    /// Grid spacing in metres. Default 1.0.
    pub grid_step: Option<f32>,
}

impl SettingsPatch {
    fn apply(self, s: &mut ProjectSettings) -> Result<(), CommandError> {
        if let Some(v) = self.tile_px {
            if !(1..=1024).contains(&v) {
                return Err(CommandError::new(
                    ErrorCode::OutOfBounds,
                    "tile_px must be between 1 and 1024.",
                ));
            }
            s.tile_px = v;
        }
        if let Some(v) = self.fps {
            if !(1..=240).contains(&v) {
                return Err(CommandError::new(
                    ErrorCode::OutOfBounds,
                    "fps must be between 1 and 240.",
                ));
            }
            s.fps = v;
        }
        if let Some(v) = self.grid_step {
            if !(v.is_finite() && v > 0.0) {
                return Err(CommandError::new(
                    ErrorCode::OutOfBounds,
                    "grid_step must be a positive number of metres.",
                ));
            }
            s.grid_step = v;
        }
        if let Some(v) = self.filter {
            s.filter = v;
        }
        if let Some(v) = self.palette_lock {
            s.palette_lock = v;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectNew {
    /// Project name. Default "Untitled".
    pub name: Option<String>,
    /// Starting settings; pixel-art defaults otherwise.
    pub settings: Option<SettingsPatch>,
}

impl Command for ProjectNew {
    const NAME: &'static str = "project.new";
    const DESCRIPTION: &'static str = "Start a new empty project with pixel-art defaults (16 px tiles, nearest filtering, 1 tile = 1 m, 30 fps). Undo returns to the previous project.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let mut doc = Document::new(self.name.unwrap_or_else(|| "Untitled".into()));
        self.settings.unwrap_or_default().apply(&mut doc.settings)?;
        let reply = json!({ "name": doc.name, "settings": doc.settings });
        cx.state.doc = doc;
        cx.state.path = None;
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectOpen {
    /// Path to a .quarry file.
    pub path: PathBuf,
}

impl Command for ProjectOpen {
    const NAME: &'static str = "project.open";
    const DESCRIPTION: &'static str = "Open a .quarry project file, replacing the current project.";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        cx.step(0.0)?;
        let doc = cx.storage()?.open(&self.path)?;
        cx.step(1.0)?;
        let reply = json!({ "name": doc.name, "objects": doc.objects.len() });
        cx.state.doc = doc;
        cx.state.path = Some(self.path);
        cx.mark_saved();
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectRecover {
    /// The autosave file offered on launch.
    pub autosave: PathBuf,
}

impl Command for ProjectRecover {
    const NAME: &'static str = "project.recover";
    const DESCRIPTION: &'static str = "Restore work from an autosave after a crash. The project keeps its own file path and is left unsaved, so the next save writes the recovered work there.";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        cx.step(0.0)?;
        let (doc, path) = cx.storage()?.recover(&self.autosave)?;
        cx.step(1.0)?;
        let reply = json!({ "name": doc.name, "path": path, "objects": doc.objects.len() });
        cx.state.doc = doc;
        cx.state.path = path;
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSave {}

impl Command for ProjectSave {
    const NAME: &'static str = "project.save";
    const DESCRIPTION: &'static str =
        "Save the project to its .quarry file. A project never saved needs project.save_as first.";
    const LONG: bool = true;
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let path = cx.state.path.clone().ok_or_else(|| {
            CommandError::invalid(
                "This project has not been saved yet. Call project.save_as with a path.",
            )
        })?;
        save_to(cx, path)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSaveAs {
    /// Where to write the .quarry file. The extension is added if missing.
    pub path: PathBuf,
}

impl Command for ProjectSaveAs {
    const NAME: &'static str = "project.save_as";
    const DESCRIPTION: &'static str =
        "Save the project to a new .quarry file and keep working in that file.";
    const LONG: bool = true;
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let mut path = self.path;
        if path.extension().is_none_or(|e| e != "quarry") {
            path.as_mut_os_string().push(".quarry");
        }
        save_to(cx, path)
    }
}

fn save_to(cx: &mut Context, path: PathBuf) -> CommandResult {
    cx.step(0.0)?;
    cx.storage()?.save(&cx.state.doc, &path)?;
    cx.step(1.0)?;
    let reply = json!({ "path": path });
    cx.state.path = Some(path);
    cx.mark_saved();
    Ok(reply)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectInfo {}

impl Command for ProjectInfo {
    const NAME: &'static str = "project.info";
    const DESCRIPTION: &'static str = "Project name, file path, settings and counts of objects, images, skeletons, clips and triangles.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let triangles: usize = doc
            .objects
            .iter()
            .filter_map(|o| o.mesh.as_deref())
            .map(Mesh::triangle_count)
            .sum();
        Ok(json!({
            "name": doc.name,
            "path": cx.state.path,
            "settings": doc.settings,
            "counts": {
                "objects": doc.objects.len(),
                "images": doc.images.len(),
                "skeletons": doc.skeletons.len(),
                "clips": doc.clips.len(),
                "triangles": triangles,
            },
        }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ProjectSetSettings(pub SettingsPatch);

impl Command for ProjectSetSettings {
    const NAME: &'static str = "project.set_settings";
    const DESCRIPTION: &'static str =
        "Change project settings: tile_px, filter, fps, palette_lock, grid_step.";

    fn run(self, cx: &mut Context) -> CommandResult {
        self.0.apply(&mut cx.doc().settings)?;
        Ok(json!({ "settings": cx.state.doc.settings }))
    }
}

// ---- history and control ---------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryUndo {}

impl Command for HistoryUndo {
    const NAME: &'static str = "history.undo";
    const DESCRIPTION: &'static str = "Undo the last edit, human or agent. History is shared.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let entry = cx
            .history
            .entries()
            .last()
            .ok_or_else(|| CommandError::new(ErrorCode::Conflict, "Nothing to undo."))?;
        let label = entry.display_label();
        cx.request_undo();
        Ok(json!({ "undone": label }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryRedo {}

impl Command for HistoryRedo {
    const NAME: &'static str = "history.redo";
    const DESCRIPTION: &'static str = "Redo the last undone edit.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if !cx.history.can_redo() {
            return Err(CommandError::new(ErrorCode::Conflict, "Nothing to redo."));
        }
        cx.request_redo();
        Ok(json!({ "redone": true }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControlCancel {
    /// The request to stop. Default: whatever is running.
    pub request_id: Option<u64>,
}

impl Command for ControlCancel {
    const NAME: &'static str = "control.cancel";
    const DESCRIPTION: &'static str = "Stop the running long command (open, save, bake, export). The scene goes back to how it was before that command.";
    const UNDOABLE: bool = false;

    /// Commands run one at a time, so by the time this one runs nothing else is
    /// running. The server answers cancel itself while a command is in flight.
    fn run(self, _cx: &mut Context) -> CommandResult {
        Ok(json!({ "cancelled": false, "reason": "Nothing is running." }))
    }
}

// ---- query -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryScene {
    /// How many levels of children to include. Default: all.
    pub depth: Option<u32>,
}

impl Command for QueryScene {
    const NAME: &'static str = "query.scene";
    const DESCRIPTION: &'static str = "The scene tree: each object's id, name, kind and children.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        // ponytail: no paging or 4,000-token cap yet; that is FR-MCP-02 in M8.
        fn node(doc: &Document, o: &Object, depth: Option<u32>) -> serde_json::Value {
            let children: Vec<_> = match depth {
                Some(0) => Vec::new(),
                _ => doc
                    .objects
                    .iter()
                    .filter(|c| c.parent == Some(o.id))
                    .map(|c| node(doc, c, depth.map(|d| d - 1)))
                    .collect(),
            };
            json!({ "id": o.id, "name": o.name, "kind": o.kind, "children": children })
        }
        let doc = &cx.state.doc;
        let roots: Vec<_> = doc
            .objects
            .iter()
            .filter(|o| o.parent.is_none())
            .map(|o| node(doc, o, self.depth))
            .collect();
        Ok(json!({ "objects": roots }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryHistory {
    /// How many recent entries. Default 20.
    pub n: Option<usize>,
}

impl Command for QueryHistory {
    const NAME: &'static str = "query.history";
    const DESCRIPTION: &'static str =
        "The most recent undo history entries, newest last, with their source (human or agent).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let entries = cx.history.entries();
        let n = self.n.unwrap_or(20).min(entries.len());
        let list: Vec<_> = entries[entries.len() - n..]
            .iter()
            .map(|e| json!({ "label": e.display_label(), "source": e.source }))
            .collect();
        Ok(json!({ "entries": list }))
    }
}

// ---- scene -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneRename {
    /// Object ID or name.
    pub object: String,
    /// New unique name.
    pub name: String,
}

impl Command for SceneRename {
    const NAME: &'static str = "scene.rename";
    const DESCRIPTION: &'static str = "Rename an object. Names are unique in the project.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = object(doc, &self.object)?;
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err(CommandError::invalid("A name cannot be empty."));
        }
        if doc.objects.by_name(&name).is_some_and(|o| o.id != id) {
            return Err(CommandError::new(
                ErrorCode::Conflict,
                format!("Another object is already called '{name}'. Pick a different name."),
            ));
        }
        doc.objects.get_mut(id).expect("resolved above").name = name.clone();
        Ok(json!({ "id": id, "name": name }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneVisibility {
    /// Object IDs or names.
    pub objects: Vec<String>,
    pub visible: bool,
}

impl Command for SceneVisibility {
    const NAME: &'static str = "scene.visibility";
    const DESCRIPTION: &'static str =
        "Show or hide objects in the viewport. Hidden objects still export.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        for id in objects(doc, &self.objects)? {
            doc.objects.get_mut(id).expect("resolved").visible = self.visible;
        }
        Ok(json!({ "changed": self.objects.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneLock {
    /// Object IDs or names.
    pub objects: Vec<String>,
    pub locked: bool,
}

impl Command for SceneLock {
    const NAME: &'static str = "scene.lock";
    const DESCRIPTION: &'static str =
        "Lock or unlock objects. Locked objects cannot be selected in the viewport.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        for id in objects(doc, &self.objects)? {
            doc.objects.get_mut(id).expect("resolved").locked = self.locked;
        }
        Ok(json!({ "changed": self.objects.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneDelete {
    /// Object IDs or names. Their children move up to the deleted object's parent.
    pub objects: Vec<String>,
}

impl Command for SceneDelete {
    const NAME: &'static str = "scene.delete";
    const DESCRIPTION: &'static str =
        "Delete objects. Children move up a level. Undo brings them back.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let ids = objects(doc, &self.objects)?;
        for &id in &ids {
            let instances: Vec<String> = doc
                .objects
                .iter()
                .filter(|o| o.instance_of == Some(id) && !ids.contains(&o.id))
                .map(|o| o.name.clone())
                .collect();
            if !instances.is_empty() {
                let name = &doc.objects.get(id).expect("resolved").name;
                return Err(CommandError::new(
                    ErrorCode::Conflict,
                    format!(
                        "'{name}' is the source of instances {}. Delete those first.",
                        instances.join(", ")
                    ),
                ));
            }
        }
        for id in ids {
            let parent = doc.objects.get(id).and_then(|o| o.parent);
            let children: Vec<_> = doc
                .objects
                .iter()
                .filter(|o| o.parent == Some(id))
                .map(|o| o.id)
                .collect();
            for child in children {
                doc.objects.get_mut(child).expect("listed").parent = parent;
            }
            doc.objects.remove(id);
        }
        Ok(json!({ "deleted": self.objects.len() }))
    }
}

// ---- mesh ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Primitive {
    Box,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshAddPrimitive {
    /// Shape to add. Wedge, cylinder, cone, sphere and plane arrive in M3.
    pub kind: Primitive,
    /// Size in metres [x, y, z]. Default [1, 1, 1].
    pub size: Option<[f32; 3]>,
    /// Centre in metres [x, y, z]. Default the origin.
    pub position: Option<[f32; 3]>,
    /// Object name. Default the shape's name, made unique.
    pub name: Option<String>,
}

impl Command for MeshAddPrimitive {
    const NAME: &'static str = "mesh.add_primitive";
    const DESCRIPTION: &'static str =
        "Add a primitive mesh object. Every face gets the full tile as its UVs.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let size = Vec3::from(self.size.unwrap_or([1.0; 3]));
        if !(size.is_finite() && size.min_element() > 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "size must be three positive numbers of metres.",
            ));
        }
        let position = Vec3::from(self.position.unwrap_or([0.0; 3]));
        if !position.is_finite() {
            return Err(CommandError::invalid(
                "position must be three finite numbers.",
            ));
        }
        let doc = cx.doc();
        let tile = doc.settings.tile_px as f32;
        let (positions, quads) = match self.kind {
            Primitive::Box => quarry_ops::primitive::cuboid(size),
        };
        let uvs = [
            Vec2::new(0.0, tile),
            Vec2::new(tile, tile),
            Vec2::new(tile, 0.0),
            Vec2::new(0.0, 0.0),
        ];
        let faces = quads
            .into_iter()
            .map(|q| Face {
                corners: q
                    .iter()
                    .zip(uvs)
                    .map(|(&vertex, uv)| Corner {
                        vertex,
                        uv,
                        color: Vec4::ONE,
                    })
                    .collect(),
                image: None,
                smooth_group: 0,
                tile: None,
                hidden: false,
            })
            .collect();

        let name = match self.name {
            Some(name) if doc.objects.by_name(&name).is_some() => {
                return Err(CommandError::new(
                    ErrorCode::Conflict,
                    format!("An object is already called '{name}'. Pick a different name."),
                ));
            }
            Some(name) => name,
            None => doc.objects.unique_name("Box"),
        };
        let mut object = Object::new(name, ObjectKind::Mesh);
        object.transform.translation = position;
        object.mesh = Some(Arc::new(Mesh {
            positions,
            weights: Vec::new(),
            faces,
            mirror: None,
        }));
        let id = doc.objects.insert(object);
        let name = &doc.objects.get(id).expect("inserted").name;
        Ok(json!({ "id": id, "name": name }))
    }
}
