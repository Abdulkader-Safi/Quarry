//! The project data model, build spec §5 and Stage 05 "Data models".
//! Right-handed, Y up, metres. Rotations are quaternions. Time in seconds.
//! UVs are in pixels.

use crate::id::{Arena, Id, Keyed};
use glam::{Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const FORMAT_VERSION: u32 = 1;
pub const MAX_BONES: usize = 128;
pub const MAX_PALETTE_COLORS: usize = 256;

macro_rules! keyed {
    ($type:ty, $prefix:literal) => {
        impl Keyed for $type {
            const PREFIX: char = $prefix;
            fn id(&self) -> Id<Self> {
                self.id
            }
            fn set_id(&mut self, id: Id<Self>) {
                self.id = id;
            }
            fn name(&self) -> &str {
                &self.name
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub format_version: u32,
    pub name: String,
    pub settings: ProjectSettings,
    pub objects: Arena<Object>,
    /// Tilesets and textures together; `Image::role` tells them apart.
    pub images: Arena<Image>,
    pub palettes: Vec<Palette>,
    pub skeletons: Arena<Skeleton>,
    pub clips: Arena<Clip>,
    pub export_presets: Vec<ExportPreset>,
    /// Camera, crosshair and display toggles. Saved with the project, never
    /// part of undo.
    #[serde(default)]
    pub view: View,
    /// What the next mesh command acts on. Not saved; undo restores it with
    /// the mesh it belongs to.
    #[serde(skip)]
    pub selection: Selection,
}

impl Document {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.into(),
            settings: ProjectSettings::default(),
            objects: Arena::default(),
            images: Arena::default(),
            palettes: Vec::new(),
            skeletons: Arena::default(),
            clips: Arena::default(),
            export_presets: Vec::new(),
            view: View::default(),
            selection: Selection::default(),
        }
    }

    /// Add everything from `other` (an imported fragment) with fresh IDs and
    /// unique names, fixing every reference. Returns the new top-level objects.
    pub fn merge(&mut self, other: Document) -> Vec<Id<Object>> {
        use std::collections::HashMap;
        let mut images: HashMap<Id<Image>, Id<Image>> = HashMap::new();
        for mut image in other.images.iter().cloned() {
            let old = image.id;
            image.name = self.images.unique_name(&image.name);
            images.insert(old, self.images.insert(image));
        }
        let mut skeletons: HashMap<Id<Skeleton>, Id<Skeleton>> = HashMap::new();
        for mut skeleton in other.skeletons.iter().cloned() {
            let old = skeleton.id;
            skeleton.name = self.skeletons.unique_name(&skeleton.name);
            skeletons.insert(old, self.skeletons.insert(skeleton));
        }
        for mut clip in other.clips.iter().cloned() {
            match skeletons.get(&clip.skeleton) {
                Some(s) => clip.skeleton = *s,
                None => continue,
            }
            clip.name = self.clips.unique_name(&clip.name);
            self.clips.insert(clip);
        }
        // Objects first with new IDs, then their references.
        let mut objects: HashMap<Id<Object>, Id<Object>> = HashMap::new();
        for mut object in other.objects.iter().cloned() {
            let old = object.id;
            object.name = self.objects.unique_name(&object.name);
            objects.insert(old, self.objects.insert(object));
        }
        let base_order = self
            .objects
            .iter()
            .filter(|o| o.parent.is_none())
            .map(|o| o.order + 1)
            .max()
            .unwrap_or(0);
        let mut roots = Vec::new();
        for (old, new) in &objects {
            let o = self.objects.get_mut(*new).expect("just inserted");
            o.parent = o.parent.and_then(|p| objects.get(&p).copied());
            o.instance_of = o.instance_of.and_then(|s| objects.get(&s).copied());
            if let Some(skin) = &mut o.skin {
                match skeletons.get(&skin.skeleton) {
                    Some(s) => skin.skeleton = *s,
                    None => o.skin = None,
                }
            }
            if let Some(mesh) = &mut o.mesh {
                for face in &mut Arc::make_mut(mesh).faces {
                    face.image = face.image.and_then(|i| images.get(&i).copied());
                }
            }
            if o.parent.is_none() {
                o.order += base_order;
                roots.push((other.objects.get(*old).map_or(0, |x| x.order), *new));
            }
        }
        roots.sort();
        roots.into_iter().map(|(_, id)| id).collect()
    }

    /// World matrix of `object`, following parents (cycles cut at 64 levels).
    pub fn world_matrix(&self, object: &Object) -> glam::Mat4 {
        let mut matrix = glam::Mat4::IDENTITY;
        let mut current = Some(object);
        for _ in 0..64 {
            let Some(o) = current else { break };
            let t = &o.transform;
            matrix =
                glam::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation)
                    * matrix;
            current = o.parent.and_then(|p| self.objects.get(p));
        }
        matrix
    }

    /// The mesh an object draws: its own, or its source's for an instance.
    pub fn drawn_mesh<'a>(&'a self, object: &'a Object) -> Option<&'a Mesh> {
        match object.instance_of {
            Some(source) => self.objects.get(source).and_then(|s| s.mesh.as_deref()),
            None => object.mesh.as_deref(),
        }
    }

    /// World-space bounds of the given objects' meshes (origins for the rest).
    pub fn bounds<'a>(
        &self,
        objects: impl IntoIterator<Item = &'a Object>,
    ) -> Option<(Vec3, Vec3)> {
        let mut bounds: Option<(Vec3, Vec3)> = None;
        let mut add = |p: Vec3| {
            bounds = Some(match bounds {
                None => (p, p),
                Some((lo, hi)) => (lo.min(p), hi.max(p)),
            });
        };
        for object in objects {
            let world = self.world_matrix(object);
            match self.drawn_mesh(object) {
                Some(mesh) if !mesh.positions.is_empty() => {
                    mesh.positions
                        .iter()
                        .for_each(|p| add(world.transform_point3(*p)));
                }
                _ => add(world.transform_point3(Vec3::ZERO)),
            }
        }
        bounds
    }

    /// Rough bytes held by `self` that `other` does not share. History uses it
    /// to enforce its memory cap.
    pub fn bytes_not_shared_with(&self, other: &Document) -> usize {
        let objects: usize = self
            .objects
            .unshared(&other.objects)
            .map(|o| {
                let shared_mesh = match (
                    &o.mesh,
                    other.objects.get(o.id).and_then(|p| p.mesh.as_ref()),
                ) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    _ => false,
                };
                256 + if shared_mesh {
                    0
                } else {
                    o.mesh.as_deref().map_or(0, Mesh::bytes)
                }
            })
            .sum();
        let images: usize = self
            .images
            .unshared(&other.images)
            .map(|i| 256 + i.pixels.len())
            .sum();
        let skeletons: usize = self
            .skeletons
            .unshared(&other.skeletons)
            .map(|s| 256 + s.bones.len() * 64)
            .sum();
        let clips: usize = self
            .clips
            .unshared(&other.clips)
            .map(|c| 256 + c.tracks.iter().map(|t| t.keys.len() * 48).sum::<usize>())
            .sum();
        objects + images + skeletons + clips + 1024
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub tile_px: u32,
    pub filter: Filter,
    pub fps: u32,
    pub palette_lock: bool,
    pub grid_step: f32,
    /// The palette painting snaps to when palette_lock is on.
    #[serde(default)]
    pub active_palette: Option<String>,
}

/// Pixel-art defaults (FR-PROJ-07): 16 px tiles, nearest filter, 1 tile = 1 m.
impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            tile_px: 16,
            filter: Filter::Nearest,
            fps: 30,
            palette_lock: true,
            grid_step: 1.0,
            active_palette: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    pub camera: crate::camera::OrbitCamera,
    /// The 3D crosshair: where tiles, primitives, bones and markers go.
    pub cursor: Vec3,
    pub display: DisplayToggles,
    pub tile: TileTool,
    pub anim: AnimTool,
}

/// The active clip, playhead and auto-key switch (FR-ANIM-02, 04).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimTool {
    pub clip: Option<Id<Clip>>,
    /// Seconds.
    pub time: f32,
    pub autokey: bool,
}

/// Tile brush and placement plane (FR-TILE-02, 04, 05).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TileTool {
    pub brush: Option<Brush>,
    /// The plane's normal axis; Y is the floor.
    pub axis: Axis,
    /// Degrees about the plane's first axis.
    pub tilt: f32,
}

impl Default for TileTool {
    fn default() -> Self {
        Self {
            brush: None,
            axis: Axis::Y,
            tilt: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brush {
    pub tileset: Id<Image>,
    /// Top-left tile column and row, and the brush size in tiles.
    pub col: u32,
    pub row: u32,
    pub width: u32,
    pub height: u32,
    /// Quarter turns counter-clockwise.
    pub rotate: u8,
    /// Left-right.
    pub mirror: bool,
    /// Top-bottom.
    pub flip: bool,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SelectMode {
    #[default]
    Object,
    Vertex,
    Edge,
    Face,
}

/// Selected objects, or elements of one mesh object being edited (FR-MESH-02).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub mode: SelectMode,
    /// The mesh object whose vertices, edges or faces are selected.
    pub object: Option<Id<Object>>,
    pub objects: std::collections::BTreeSet<Id<Object>>,
    pub vertices: std::collections::BTreeSet<u32>,
    /// Vertex pairs, smaller index first.
    pub edges: std::collections::BTreeSet<(u32, u32)>,
    pub faces: std::collections::BTreeSet<u32>,
}

impl Selection {
    /// Drop anything that no longer exists (after deletes, undo or a new project).
    pub fn retain_valid(&mut self, doc_objects: &Arena<Object>) {
        self.objects.retain(|id| doc_objects.get(*id).is_some());
        let mesh = self
            .object
            .and_then(|id| doc_objects.get(id))
            .and_then(|o| o.mesh.as_deref());
        match mesh {
            Some(mesh) => {
                let (v, f) = (mesh.positions.len() as u32, mesh.faces.len() as u32);
                self.vertices.retain(|i| *i < v);
                self.edges.retain(|(a, b)| *a < v && *b < v);
                self.faces.retain(|i| *i < f);
            }
            None => {
                self.object = None;
                self.vertices.clear();
                self.edges.clear();
                self.faces.clear();
                if self.mode != SelectMode::Object {
                    self.mode = SelectMode::Object;
                }
            }
        }
    }

    /// Vertices touched by the selection, whatever the mode.
    pub fn touched_vertices(&self, mesh: &Mesh) -> std::collections::BTreeSet<u32> {
        let mut out = self.vertices.clone();
        for (a, b) in &self.edges {
            out.insert(*a);
            out.insert(*b);
        }
        for f in &self.faces {
            if let Some(face) = mesh.faces.get(*f as usize) {
                out.extend(face.corners.iter().map(|c| c.vertex));
            }
        }
        out
    }
}

/// FR-VIEW-05.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayToggles {
    pub wireframe: bool,
    pub textured: bool,
    pub vertex_color: bool,
    pub weights: bool,
    pub bones: bool,
    pub grid: bool,
    pub backface_culling: bool,
}

impl Default for DisplayToggles {
    fn default() -> Self {
        Self {
            wireframe: false,
            textured: true,
            vertex_color: false,
            weights: false,
            bones: true,
            grid: true,
            backface_culling: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Nearest,
    Linear,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: Id<Object>,
    pub name: String,
    pub kind: ObjectKind,
    pub parent: Option<Id<Object>>,
    /// Position among its siblings in the scene tree, lowest first.
    #[serde(default)]
    pub order: i32,
    pub transform: Transform,
    pub visible: bool,
    pub locked: bool,
    /// Shared until edited, so moving an object never copies its mesh.
    pub mesh: Option<Arc<Mesh>>,
    pub instance_of: Option<Id<Object>>,
    pub marker_type: Option<MarkerType>,
    pub collision: bool,
    pub properties: BTreeMap<String, PropValue>,
    pub skin: Option<Skin>,
}
keyed!(Object, 'o');

impl Object {
    pub fn new(name: impl Into<String>, kind: ObjectKind) -> Self {
        Self {
            id: Id::new(0, 0),
            name: name.into(),
            kind,
            parent: None,
            order: 0,
            transform: Transform::default(),
            visible: true,
            locked: false,
            mesh: None,
            instance_of: None,
            marker_type: None,
            collision: false,
            properties: BTreeMap::new(),
            skin: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Mesh,
    Instance,
    Marker,
    Layer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkerType {
    Spawn,
    Pickup,
    Trigger,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum PropValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skin {
    pub skeleton: Id<Skeleton>,
    pub bind: Bind,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bind {
    Weighted,
    Rigid { bone: u16 },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    /// Empty, or one entry per position.
    pub weights: Vec<Influences>,
    pub faces: Vec<Face>,
    pub mirror: Option<Axis>,
    /// UV seams as vertex pairs, smaller index first (FR-UV-02).
    #[serde(default)]
    pub seams: std::collections::BTreeSet<(u32, u32)>,
}

impl Mesh {
    pub fn bytes(&self) -> usize {
        self.positions.len() * 12
            + self.weights.len() * 24
            + self
                .faces
                .iter()
                .map(|f| 16 + f.corners.len() * 28)
                .sum::<usize>()
    }

    pub fn triangle_count(&self) -> usize {
        self.faces.iter().map(|f| f.corners.len() - 2).sum()
    }
}

/// Up to 4 bone influences, sorted by weight, summing to 1. Unused slots have weight 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Influences {
    pub bones: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Face {
    /// 3 or 4 corners.
    pub corners: Vec<Corner>,
    pub image: Option<Id<Image>>,
    /// 0 = flat.
    pub smooth_group: u8,
    pub tile: Option<u32>,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Corner {
    pub vertex: u32,
    /// Pixels, so resizing the image keeps the same texels (FR-ART-04).
    pub uv: Vec2,
    /// Linear RGBA.
    pub color: Vec4,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub id: Id<Image>,
    pub name: String,
    pub role: ImageRole,
    pub size: [u32; 2],
    pub tile_px: u32,
    pub padding: u32,
    /// `images/<id>.png` inside the `.quarry` file.
    pub zip_path: String,
    pub source_path: Option<String>,
    pub source_hash: u64,
    pub filter: Filter,
    /// Reload when the linked file changes (FR-ART-02).
    #[serde(default = "watch_default")]
    pub watch: bool,
    /// sRGB RGBA8, row-major. Lives in the PNG, not in project.json.
    #[serde(skip)]
    pub pixels: Arc<[u8]>,
}
keyed!(Image, 'i');

fn watch_default() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageRole {
    Tileset,
    Texture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    /// sRGB RGBA8, at most 256.
    pub colors: Vec<[u8; 4]>,
    pub weights: Option<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skeleton {
    pub id: Id<Skeleton>,
    pub name: String,
    /// At most 128.
    pub bones: Vec<Bone>,
    pub ik_chains: Vec<IkChain>,
    /// Current pose per bone, relative to rest (FR-RIG-08). Missing means rest.
    #[serde(default)]
    pub pose: Vec<BonePose>,
}
keyed!(Skeleton, 's');

/// A bone's offset from its rest pose, in the bone's own frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BonePose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for BonePose {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl BonePose {
    pub fn matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Skeleton {
    pub fn index(&self, name: &str) -> Option<u16> {
        self.bones
            .iter()
            .position(|b| b.name == name)
            .map(|i| i as u16)
    }

    /// Rest matrices in skeleton space.
    pub fn rest_world(&self) -> Vec<glam::Mat4> {
        self.bones
            .iter()
            .map(|b| quarry_ops::rig::rest_matrix(b.head, b.tail, b.roll))
            .collect()
    }

    /// Each bone's rest matrix relative to its parent's.
    pub fn rest_local(&self) -> Vec<glam::Mat4> {
        let world = self.rest_world();
        self.bones
            .iter()
            .enumerate()
            .map(|(i, b)| match b.parent {
                Some(p) => world[p as usize].inverse() * world[i],
                None => world[i],
            })
            .collect()
    }

    pub fn pose_of(&self, bone: usize) -> BonePose {
        self.pose.get(bone).copied().unwrap_or_default()
    }

    /// Posed matrices in skeleton space (forward kinematics).
    pub fn pose_world(&self) -> Vec<glam::Mat4> {
        let local = self.rest_local();
        let mut world: Vec<Option<glam::Mat4>> = vec![None; self.bones.len()];
        fn solve(
            s: &Skeleton,
            i: usize,
            local: &[glam::Mat4],
            world: &mut [Option<glam::Mat4>],
            depth: u32,
        ) -> glam::Mat4 {
            if let Some(m) = world[i] {
                return m;
            }
            let own = local[i] * s.pose_of(i).matrix();
            let m = match s.bones[i].parent {
                Some(p) if depth < 128 => solve(s, p as usize, local, world, depth + 1) * own,
                _ => own,
            };
            world[i] = Some(m);
            m
        }
        (0..self.bones.len())
            .map(|i| solve(self, i, &local, &mut world, 0))
            .collect()
    }

    /// Pose a two-bone chain so its tip reaches its target, bending toward
    /// its pole (rotations stored in the bones' own frames).
    pub fn solve_ik(&mut self, chain: usize) {
        let Some(c) = self.ik_chains.get(chain).cloned() else {
            return;
        };
        let lower = c.tip_bone as usize;
        let Some(upper) = self
            .bones
            .get(lower)
            .and_then(|b| b.parent)
            .map(|p| p as usize)
        else {
            return;
        };
        self.pose.resize(self.bones.len(), BonePose::default());
        let world = self.pose_world();
        let length = self.bones[lower].head.distance(self.bones[lower].tail);
        let root = world[upper].transform_point3(Vec3::ZERO);
        let mid = world[lower].transform_point3(Vec3::ZERO);
        let tip = world[lower].transform_point3(Vec3::Y * length);
        let (q1, q2) = quarry_ops::rig::two_bone_ik(root, mid, tip, c.target, c.pole);
        // A world rotation R at a bone whose world rotation is W is W⁻¹·R·W locally.
        let local = |w: glam::Mat4, r: Quat| {
            let (_, wr, _) = w.to_scale_rotation_translation();
            wr.inverse() * r * wr
        };
        self.pose[upper].rotation =
            (self.pose[upper].rotation * local(world[upper], q1)).normalize();
        let world = self.pose_world();
        self.pose[lower].rotation =
            (self.pose[lower].rotation * local(world[lower], q2)).normalize();
    }

    /// Skinning matrices: posed world times inverse rest world.
    pub fn skinning(&self) -> Vec<glam::Mat4> {
        self.pose_world()
            .iter()
            .zip(self.rest_world())
            .map(|(pose, rest)| *pose * rest.inverse())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    /// Unique per skeleton; `.L` / `.R` suffixes pair bones for mirroring.
    pub name: String,
    pub parent: Option<u16>,
    /// Rest pose in skeleton space.
    pub head: Vec3,
    pub tail: Vec3,
    pub roll: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IkChain {
    pub tip_bone: u16,
    pub target: Vec3,
    pub pole: Vec3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: Id<Clip>,
    /// Unique per skeleton; becomes the glTF animation name.
    pub name: String,
    pub skeleton: Id<Skeleton>,
    pub length: f32,
    #[serde(rename = "loop")]
    pub looping: bool,
    pub tracks: Vec<Track>,
}
keyed!(Clip, 'c');

impl Clip {
    /// The skeleton posed by this clip at `time`: keyed channels from their
    /// tracks, everything else at rest, then every IK chain solved (targets
    /// and poles from their tracks when keyed). Playback, preview and export
    /// all use this, so what plays is what exports.
    pub fn pose_at(&self, skeleton: &Skeleton, time: f32) -> Skeleton {
        use quarry_ops::anim::evaluate;
        let mut s = skeleton.clone();
        s.pose = vec![BonePose::default(); s.bones.len()];
        for track in &self.tracks {
            let vec_keys = || -> Vec<(f32, Vec3, quarry_ops::anim::Ease)> {
                track
                    .keys
                    .iter()
                    .filter_map(|k| match k.value {
                        KeyValue::Vec3(v) => Some((k.time, v, k.interp.ease())),
                        _ => None,
                    })
                    .collect()
            };
            match (track.target, track.channel) {
                (TrackTarget::Bone(b), Channel::Rotation) if (b as usize) < s.bones.len() => {
                    let keys: Vec<_> = track
                        .keys
                        .iter()
                        .filter_map(|k| match k.value {
                            KeyValue::Quat(q) => Some((k.time, q, k.interp.ease())),
                            _ => None,
                        })
                        .collect();
                    if let Some(q) = evaluate(&keys, time) {
                        s.pose[b as usize].rotation = q;
                    }
                }
                (TrackTarget::Bone(b), Channel::Translation) if (b as usize) < s.bones.len() => {
                    if let Some(v) = evaluate(&vec_keys(), time) {
                        s.pose[b as usize].translation = v;
                    }
                }
                (TrackTarget::Bone(b), Channel::Scale) if (b as usize) < s.bones.len() => {
                    if let Some(v) = evaluate(&vec_keys(), time) {
                        s.pose[b as usize].scale = v;
                    }
                }
                (TrackTarget::Ik(c), Channel::IkTarget) if (c as usize) < s.ik_chains.len() => {
                    if let Some(v) = evaluate(&vec_keys(), time) {
                        s.ik_chains[c as usize].target = v;
                    }
                }
                (TrackTarget::Ik(c), Channel::IkPole) if (c as usize) < s.ik_chains.len() => {
                    if let Some(v) = evaluate(&vec_keys(), time) {
                        s.ik_chains[c as usize].pole = v;
                    }
                }
                _ => {}
            }
        }
        for chain in 0..s.ik_chains.len() {
            s.solve_ik(chain);
        }
        s
    }
}

impl Interp {
    pub fn ease(self) -> quarry_ops::anim::Ease {
        use quarry_ops::anim::Ease;
        match self {
            Interp::Step => Ease::Step,
            Interp::Linear => Ease::Linear,
            Interp::EaseIn => Ease::In,
            Interp::EaseOut => Ease::Out,
            // ponytail: bezier plays as ease-in-out until the graph editor
            // (FR-ANIM-06, optional) gives it handles.
            Interp::EaseInOut | Interp::Bezier => Ease::InOut,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub target: TrackTarget,
    pub channel: Channel,
    /// Sorted by time.
    pub keys: Vec<Key>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackTarget {
    Bone(u16),
    /// Index into the skeleton's IK chains.
    Ik(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Translation,
    Rotation,
    Scale,
    IkTarget,
    IkPole,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub time: f32,
    pub value: KeyValue,
    pub interp: Interp,
    /// Bezier only, from the graph editor.
    pub handles: Option<[Vec2; 2]>,
}

/// Translation, scale and IK keys are vectors; rotation keys are quaternions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyValue {
    Vec3(Vec3),
    Quat(Quat),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Interp {
    Step,
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bezier,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportPreset {
    pub name: String,
    /// Relative to the project file's folder.
    pub path: String,
    pub format: ExportFormat,
    pub selection: ExportSelection,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Glb,
    GltfSeparate,
    Obj,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportSelection {
    All,
    Selected,
    Objects(Vec<String>),
}
