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
        }
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
    /// sRGB RGBA8, row-major. Lives in the PNG, not in project.json.
    #[serde(skip)]
    pub pixels: Arc<[u8]>,
}
keyed!(Image, 'i');

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
}
keyed!(Skeleton, 's');

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
