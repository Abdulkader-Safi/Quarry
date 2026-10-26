//! Scene structure (FR-SCENE-01 to 05): parenting, order, objects from faces,
//! origins, instances, markers, collision and custom properties. The exporter
//! turns markers, collision and properties into node names and `extras` (M5).

use super::select::edit_mesh;
use super::tile::compact;
use super::{object, objects};
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{Document, MarkerType, Mesh, Object, ObjectKind, PropValue, SelectMode};
use glam::{Mat4, Vec3};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<SceneParent>();
    r.register::<SceneReorder>();
    r.register::<SceneObjectFromSelection>();
    r.register::<SceneOriginToCursor>();
    r.register::<SceneInstanceAdd>();
    r.register::<SceneMarkerAdd>();
    r.register::<SceneSetCollision>();
    r.register::<SceneSetProps>();
}

fn parent_world(doc: &Document, parent: Option<Id<Object>>) -> Mat4 {
    parent
        .and_then(|p| doc.objects.get(p))
        .map_or(Mat4::IDENTITY, |p| doc.world_matrix(p))
}

/// Put `id` at `world` (a world matrix) under its current parent.
fn set_world(doc: &mut Document, id: Id<Object>, world: Mat4) {
    let parent = doc.objects.get(id).and_then(|o| o.parent);
    let local = parent_world(doc, parent).inverse() * world;
    let (scale, rotation, translation) = local.to_scale_rotation_translation();
    let t = &mut doc.objects.get_mut(id).expect("exists").transform;
    (t.scale, t.rotation, t.translation) = (scale, rotation.normalize(), translation);
}

fn unique(doc: &Document, name: Option<String>, base: &str) -> Result<String, CommandError> {
    match name {
        Some(n) if n.trim().is_empty() => Err(CommandError::invalid("A name cannot be empty.")),
        Some(n) if doc.objects.by_name(&n).is_some() => Err(CommandError::new(
            ErrorCode::Conflict,
            format!("An object is already called '{n}'. Pick a different name."),
        )),
        Some(n) => Ok(n),
        None => Ok(doc.objects.unique_name(base)),
    }
}

/// Next order value after the last sibling under `parent`.
fn last_order(doc: &Document, parent: Option<Id<Object>>) -> i32 {
    doc.objects
        .iter()
        .filter(|o| o.parent == parent)
        .map(|o| o.order + 1)
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneParent {
    /// Object IDs or names to move in the tree.
    pub objects: Vec<String>,
    /// New parent ID or name; leave out to move them to the top level.
    pub parent: Option<String>,
}

impl Command for SceneParent {
    const NAME: &'static str = "scene.parent";
    const DESCRIPTION: &'static str = "Parent objects under another object (or none). They keep their place in the world and then follow the parent.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let ids = objects(doc, &self.objects)?;
        let parent = self.parent.as_deref().map(|p| object(doc, p)).transpose()?;
        if let Some(p) = parent {
            // Refuse loops: the new parent may not be one of the objects or below them.
            let mut current = Some(p);
            while let Some(c) = current {
                if ids.contains(&c) {
                    return Err(CommandError::new(
                        ErrorCode::Conflict,
                        "That would put an object inside itself. Pick a parent outside the objects you are moving.",
                    ));
                }
                current = doc.objects.get(c).and_then(|o| o.parent);
            }
        }
        for id in &ids {
            let world = doc.world_matrix(doc.objects.get(*id).expect("resolved"));
            let order = last_order(doc, parent);
            let o = doc.objects.get_mut(*id).expect("resolved");
            o.parent = parent;
            o.order = order;
            set_world(doc, *id, world);
        }
        Ok(json!({ "parent": parent, "moved": ids.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneReorder {
    /// Object ID or name to move among its siblings.
    pub object: String,
    /// New position among its siblings, 0 first.
    pub index: usize,
}

impl Command for SceneReorder {
    const NAME: &'static str = "scene.reorder";
    const DESCRIPTION: &'static str =
        "Move an object up or down among its siblings in the scene tree.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = object(doc, &self.object)?;
        let parent = doc.objects.get(id).expect("resolved").parent;
        let mut siblings: Vec<(i32, Id<Object>)> = doc
            .objects
            .iter()
            .filter(|o| o.parent == parent && o.id != id)
            .map(|o| (o.order, o.id))
            .collect();
        siblings.sort();
        let mut order: Vec<Id<Object>> = siblings.into_iter().map(|(_, id)| id).collect();
        order.insert(self.index.min(order.len()), id);
        for (i, sibling) in order.iter().enumerate() {
            doc.objects.get_mut(*sibling).expect("listed").order = i as i32;
        }
        Ok(json!({ "index": self.index.min(order.len() - 1) }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneObjectFromSelection {
    /// Name for the new object. Default "Part", made unique.
    pub name: Option<String>,
}

impl Command for SceneObjectFromSelection {
    const NAME: &'static str = "scene.object_from_selection";
    const DESCRIPTION: &'static str = "Move the selected faces of the edited mesh into a new object whose origin is at the 3D crosshair.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let faces = doc.selection.faces.clone();
        if doc.selection.mode != SelectMode::Face || faces.is_empty() {
            return Err(CommandError::new(
                ErrorCode::NothingSelected,
                "No faces selected. Call select.mode with face, then select faces.",
            ));
        }
        let (source, _) = edit_mesh(doc)?;
        let (source_id, source_world, source_skin) =
            (source.id, doc.world_matrix(source), source.skin.clone());
        let name = unique(doc, self.name, "Part")?;
        let cursor = doc.view.cursor;

        // Copy the faces and the vertices they use into the new mesh.
        let old = doc
            .objects
            .get(source_id)
            .and_then(|o| o.mesh.clone())
            .expect("edit mesh");
        let to_new = Mat4::from_translation(cursor).inverse() * source_world;
        let mut remap: BTreeMap<u32, u32> = BTreeMap::new();
        let mut part = Mesh::default();
        for f in &faces {
            let mut face = old.faces[*f as usize].clone();
            for c in &mut face.corners {
                c.vertex = *remap.entry(c.vertex).or_insert_with(|| {
                    part.positions
                        .push(to_new.transform_point3(old.positions[c.vertex as usize]));
                    if let Some(w) = old.weights.get(c.vertex as usize) {
                        part.weights.push(*w);
                    }
                    (part.positions.len() - 1) as u32
                });
            }
            part.faces.push(face);
        }
        let mut new = Object::new(name, ObjectKind::Mesh);
        new.transform.translation = cursor;
        new.order = last_order(doc, None);
        new.skin = source_skin;
        new.mesh = Some(Arc::new(part));
        let new_id = doc.objects.insert(new);

        // Remove them from the source.
        let mesh = Arc::make_mut(
            doc.objects
                .get_mut(source_id)
                .expect("source")
                .mesh
                .as_mut()
                .expect("edit mesh"),
        );
        let mut i = 0u32;
        mesh.faces.retain(|_| {
            i += 1;
            !faces.contains(&(i - 1))
        });
        compact(mesh);
        let sel = &mut doc.selection;
        sel.mode = SelectMode::Object;
        sel.object = None;
        sel.faces.clear();
        sel.objects = BTreeSet::from([new_id]);
        Ok(json!({ "id": new_id, "faces": faces.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneOriginToCursor {
    /// Object ID or name. Default the selected objects.
    pub object: Option<String>,
}

impl Command for SceneOriginToCursor {
    const NAME: &'static str = "scene.origin_to_cursor";
    const DESCRIPTION: &'static str =
        "Move an object's origin to the 3D crosshair without moving its geometry.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let ids: Vec<Id<Object>> = match &self.object {
            Some(r) => vec![object(doc, r)?],
            None if !doc.selection.objects.is_empty() => {
                doc.selection.objects.iter().copied().collect()
            }
            None => {
                return Err(CommandError::new(
                    ErrorCode::NothingSelected,
                    "No object named or selected.",
                ));
            }
        };
        let cursor = doc.view.cursor;
        for id in &ids {
            let o = doc.objects.get(*id).expect("resolved");
            let old_world = doc.world_matrix(o);
            let (scale, rotation, _) = old_world.to_scale_rotation_translation();
            let new_world = Mat4::from_scale_rotation_translation(scale, rotation, cursor);
            let shift = new_world.inverse() * old_world;
            set_world(doc, *id, new_world);
            if let Some(mesh) = doc.objects.get_mut(*id).and_then(|o| o.mesh.as_mut()) {
                for p in &mut Arc::make_mut(mesh).positions {
                    *p = shift.transform_point3(*p);
                }
            }
        }
        Ok(json!({ "moved": ids.len(), "origin": cursor }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneInstanceAdd {
    /// The mesh object to copy (ID or name). Instances follow its edits.
    pub source: String,
    /// World position [x, y, z]. Default the 3D crosshair.
    pub position: Option<[f32; 3]>,
    /// Euler degrees [x, y, z]. Default none.
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
    pub name: Option<String>,
}

impl Command for SceneInstanceAdd {
    const NAME: &'static str = "scene.instance_add";
    const DESCRIPTION: &'static str = "Place an instance of a mesh object. It draws the source's mesh, so editing the source changes every instance.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let source = object(doc, &self.source)?;
        let src = doc.objects.get(source).expect("resolved");
        if src.mesh.is_none() || src.instance_of.is_some() {
            return Err(CommandError::invalid(format!(
                "'{}' is not a mesh object, so it cannot be instanced.",
                src.name
            )));
        }
        let base = format!("{} instance", src.name);
        let name = unique(doc, self.name, &base)?;
        let mut o = Object::new(name, ObjectKind::Instance);
        o.instance_of = Some(source);
        o.order = last_order(doc, None);
        o.transform.translation = self.position.map_or(doc.view.cursor, Vec3::from);
        if let Some([x, y, z]) = self.rotation {
            o.transform.rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                x.to_radians(),
                y.to_radians(),
                z.to_radians(),
            );
        }
        if let Some(s) = self.scale {
            o.transform.scale = Vec3::from(s);
        }
        let t = o.transform;
        if !(t.translation.is_finite() && t.rotation.is_finite() && t.scale.is_finite()) {
            return Err(CommandError::invalid(
                "position, rotation and scale must be finite numbers.",
            ));
        }
        let id = doc.objects.insert(o);
        Ok(json!({ "id": id, "name": doc.objects.get(id).expect("inserted").name }))
    }
}

fn props(map: BTreeMap<String, PropValue>) -> Result<BTreeMap<String, PropValue>, CommandError> {
    if map.keys().any(|k| k.trim().is_empty()) {
        return Err(CommandError::invalid("Property names cannot be empty."));
    }
    Ok(map)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneMarkerAdd {
    /// Marker name, such as spawn_player. Becomes the glTF node name.
    pub name: String,
    #[serde(rename = "type")]
    pub kind: MarkerType,
    /// Custom key-values (text, number or true/false), exported as extras.
    #[serde(default)]
    pub props: BTreeMap<String, PropValue>,
    /// World position [x, y, z]. Default the 3D crosshair.
    pub position: Option<[f32; 3]>,
}

impl Command for SceneMarkerAdd {
    const NAME: &'static str = "scene.marker_add";
    const DESCRIPTION: &'static str = "Add a named marker (an empty point) of type spawn, pickup, trigger or custom, with key-value properties for the game.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let name = unique(doc, Some(self.name), "Marker")?;
        let mut o = Object::new(name, ObjectKind::Marker);
        o.marker_type = Some(self.kind);
        o.properties = props(self.props)?;
        o.order = last_order(doc, None);
        o.transform.translation = self.position.map_or(doc.view.cursor, Vec3::from);
        if !o.transform.translation.is_finite() {
            return Err(CommandError::invalid(
                "position must be three finite numbers.",
            ));
        }
        let id = doc.objects.insert(o);
        Ok(json!({ "id": id }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneSetCollision {
    pub objects: Vec<String>,
    /// Exported as extras.quarry.collision.
    pub collision: bool,
}

impl Command for SceneSetCollision {
    const NAME: &'static str = "scene.set_collision";
    const DESCRIPTION: &'static str = "Flag objects as collision meshes for the game (exported as extras.quarry.collision), hidden or visible.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        for id in objects(doc, &self.objects)? {
            doc.objects.get_mut(id).expect("resolved").collision = self.collision;
        }
        Ok(json!({ "changed": self.objects.len(), "collision": self.collision }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneSetProps {
    pub object: String,
    /// Properties to add or change.
    #[serde(default)]
    pub set: BTreeMap<String, PropValue>,
    /// Property names to remove.
    #[serde(default)]
    pub remove: Vec<String>,
}

impl Command for SceneSetProps {
    const NAME: &'static str = "scene.set_props";
    const DESCRIPTION: &'static str =
        "Set or remove custom key-value properties on an object (exported as extras for the game).";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = object(doc, &self.object)?;
        let set = props(self.set)?;
        let o = doc.objects.get_mut(id).expect("resolved");
        for k in &self.remove {
            o.properties.remove(k);
        }
        o.properties.extend(set);
        Ok(json!({ "properties": o.properties }))
    }
}
