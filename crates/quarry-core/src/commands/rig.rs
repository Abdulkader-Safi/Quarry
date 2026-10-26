//! Rigging (FR-RIG-01 to 08). Bones live in skeleton space; skinned meshes are
//! bound in their object's space, so binding uses the object's world matrix.

use super::object;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{
    Bind, Bone, BonePose, Document, IkChain, Influences, MAX_BONES, Object, SelectMode, Skeleton,
    Skin, TrackTarget,
};
use glam::{EulerRot, Quat, Vec3};
use quarry_ops::rig::{auto_weights, normalise};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<RigSkeletonNew>();
    r.register::<RigBoneAdd>();
    r.register::<RigBoneEdit>();
    r.register::<RigBoneDelete>();
    r.register::<RigBindAuto>();
    r.register::<RigBindRigid>();
    r.register::<RigWeightPaint>();
    r.register::<RigIkAdd>();
    r.register::<RigIkSet>();
    r.register::<RigMirrorBones>();
    r.register::<RigMirrorWeights>();
    r.register::<RigPose>();
    r.register::<RigPoseReset>();
    r.register::<QuerySkeleton>();
}

pub(super) fn skeleton_id(doc: &Document, reference: &str) -> Result<Id<Skeleton>, CommandError> {
    doc.skeletons.find(reference).map(|s| s.id).ok_or_else(|| {
        CommandError::not_found(format!(
            "No skeleton called '{reference}'. Call rig.skeleton_new first."
        ))
    })
}

/// The skeleton to use: the named one, or the only one there is.
fn pick_skeleton(doc: &Document, reference: Option<&str>) -> Result<Id<Skeleton>, CommandError> {
    match reference {
        Some(r) => skeleton_id(doc, r),
        None if doc.skeletons.len() == 1 => Ok(doc.skeletons.ids().next().expect("one")),
        None => Err(CommandError::invalid(
            "Name the skeleton (there is more than one, or none).",
        )),
    }
}

pub(super) fn bone_index(s: &Skeleton, reference: &str) -> Result<u16, CommandError> {
    s.index(reference)
        .or_else(|| {
            reference
                .parse::<u16>()
                .ok()
                .filter(|i| (*i as usize) < s.bones.len())
        })
        .ok_or_else(|| {
            CommandError::not_found(format!(
                "'{}' has no bone '{reference}'. Call query.skeleton to list bones.",
                s.name
            ))
        })
}

fn vec3(v: [f32; 3], what: &str) -> Result<Vec3, CommandError> {
    let v = Vec3::from(v);
    v.is_finite()
        .then_some(v)
        .ok_or_else(|| CommandError::invalid(format!("{what} must be three finite numbers.")))
}

/// Mirror a `.L` / `.R` (or `_L` / `_R`) name; None when it has no side.
pub fn mirror_name(name: &str) -> Option<String> {
    for (a, b) in [
        (".L", ".R"),
        (".R", ".L"),
        ("_L", "_R"),
        ("_R", "_L"),
        (".l", ".r"),
        (".r", ".l"),
    ] {
        if let Some(stem) = name.strip_suffix(a) {
            return Some(format!("{stem}{b}"));
        }
    }
    None
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigSkeletonNew {
    /// Default "Armature", made unique.
    pub name: Option<String>,
}

impl Command for RigSkeletonNew {
    const NAME: &'static str = "rig.skeleton_new";
    const DESCRIPTION: &'static str = "Create an empty skeleton to add bones to.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let name = doc
            .skeletons
            .unique_name(self.name.as_deref().unwrap_or("Armature"));
        let id = doc.skeletons.insert(Skeleton {
            id: Id::new(0, 0),
            name: name.clone(),
            bones: Vec::new(),
            ik_chains: Vec::new(),
            pose: Vec::new(),
        });
        Ok(json!({ "id": id, "name": name }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigBoneAdd {
    pub skeleton: Option<String>,
    /// Unique in the skeleton; end with .L or .R for mirroring. Default "Bone".
    pub name: Option<String>,
    /// Parent bone name. The new bone starts at the parent's tail unless `head` is given.
    pub parent: Option<String>,
    /// Head [x, y, z]. Default the parent's tail, or the 3D crosshair.
    pub head: Option<[f32; 3]>,
    /// Tail [x, y, z]. Default 0.5 m above the head.
    pub tail: Option<[f32; 3]>,
}

impl Command for RigBoneAdd {
    const NAME: &'static str = "rig.bone_add";
    const DESCRIPTION: &'static str = "Add a bone: chained off a parent (its head on the parent's tail) or standing at the 3D crosshair.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let cursor = doc.view.cursor;
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        if s.bones.len() >= MAX_BONES {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!("A skeleton holds at most {MAX_BONES} bones."),
            ));
        }
        let parent = self
            .parent
            .as_deref()
            .map(|p| bone_index(s, p))
            .transpose()?;
        let head = match self.head {
            Some(h) => vec3(h, "head")?,
            None => parent.map_or(cursor, |p| s.bones[p as usize].tail),
        };
        let tail = match self.tail {
            Some(t) => vec3(t, "tail")?,
            None => head + Vec3::Y * 0.5,
        };
        if head.distance(tail) < 1e-4 {
            return Err(CommandError::invalid("A bone's head and tail must differ."));
        }
        let explicit = self.name.is_some();
        let base = self.name.unwrap_or_else(|| "Bone".into());
        if explicit && s.index(&base).is_some() {
            return Err(CommandError::new(
                ErrorCode::Conflict,
                format!("'{}' already has a bone called '{base}'.", s.name),
            ));
        }
        let name = unique_bone(s, &base);
        let s = doc.skeletons.get_mut(sid).expect("resolved");
        s.bones.push(Bone {
            name: name.clone(),
            parent,
            head,
            tail,
            roll: 0.0,
        });
        Ok(
            json!({ "skeleton": sid, "bone": name, "index": s.bones.len() - 1, "head": head, "tail": tail }),
        )
    }
}

fn unique_bone(s: &Skeleton, base: &str) -> String {
    if s.index(base).is_none() {
        return base.into();
    }
    (1..)
        .map(|n| format!("{base}.{n:03}"))
        .find(|n| s.index(n).is_none())
        .expect("free")
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigBoneEdit {
    pub skeleton: Option<String>,
    pub bone: String,
    pub name: Option<String>,
    /// New parent bone name; an empty string makes it a root bone.
    pub parent: Option<String>,
    pub head: Option<[f32; 3]>,
    pub tail: Option<[f32; 3]>,
    /// Roll about the bone's own axis, in degrees.
    pub roll: Option<f32>,
}

impl Command for RigBoneEdit {
    const NAME: &'static str = "rig.bone_edit";
    const DESCRIPTION: &'static str = "Rename a bone, change its parent, move its head or tail, or set its roll. Loops and duplicate names are refused.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        let i = bone_index(s, &self.bone)?;
        let parent = match self.parent.as_deref() {
            None => s.bones[i as usize].parent,
            Some("") => None,
            Some(p) => {
                let p = bone_index(s, p)?;
                // Walk up from the new parent; meeting this bone means a loop.
                let mut cur = Some(p);
                while let Some(c) = cur {
                    if c == i {
                        return Err(CommandError::new(
                            ErrorCode::Conflict,
                            "That parent is this bone or one of its children.",
                        ));
                    }
                    cur = s.bones[c as usize].parent;
                }
                Some(p)
            }
        };
        if let Some(n) = &self.name
            && s.index(n).is_some_and(|j| j != i)
        {
            return Err(CommandError::new(
                ErrorCode::Conflict,
                format!("'{}' already has a bone called '{n}'.", s.name),
            ));
        }
        let mut bone = s.bones[i as usize].clone();
        bone.parent = parent;
        if let Some(n) = self.name {
            bone.name = n;
        }
        if let Some(h) = self.head {
            bone.head = vec3(h, "head")?;
        }
        if let Some(t) = self.tail {
            bone.tail = vec3(t, "tail")?;
        }
        if let Some(r) = self.roll {
            bone.roll = r.to_radians();
        }
        if bone.head.distance(bone.tail) < 1e-4 {
            return Err(CommandError::invalid("A bone's head and tail must differ."));
        }
        doc.skeletons.get_mut(sid).expect("resolved").bones[i as usize] = bone.clone();
        Ok(json!({ "bone": bone.name, "parent": parent }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigBoneDelete {
    pub skeleton: Option<String>,
    pub bone: String,
}

impl Command for RigBoneDelete {
    const NAME: &'static str = "rig.bone_delete";
    const DESCRIPTION: &'static str = "Delete a bone. Its children move to its parent, its weights go to its parent, and IK chains and keys on it are removed.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        let gone = bone_index(s, &self.bone)?;
        let heir = s.bones[gone as usize].parent;
        // Old index → new index (None for the deleted bone).
        let remap = |b: u16| -> Option<u16> {
            match b.cmp(&gone) {
                std::cmp::Ordering::Less => Some(b),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => Some(b - 1),
            }
        };
        let s = doc.skeletons.get_mut(sid).expect("resolved");
        s.bones.remove(gone as usize);
        if (gone as usize) < s.pose.len() {
            s.pose.remove(gone as usize);
        }
        for b in &mut s.bones {
            b.parent = b
                .parent
                .and_then(|p| if p == gone { heir } else { Some(p) })
                .and_then(remap);
        }
        let chains = std::mem::take(&mut s.ik_chains);
        s.ik_chains = chains
            .into_iter()
            .filter_map(|mut c| {
                c.tip_bone = remap(c.tip_bone)?;
                Some(c)
            })
            .collect();
        // Keys on the bone go; later bones shift down.
        let clip_ids: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.skeleton == sid)
            .map(|c| c.id)
            .collect();
        for cid in clip_ids {
            let clip = doc.clips.get_mut(cid).expect("listed");
            clip.tracks.retain_mut(|t| match t.target {
                TrackTarget::Bone(b) => match remap(b) {
                    Some(n) => {
                        t.target = TrackTarget::Bone(n);
                        true
                    }
                    None => false,
                },
                TrackTarget::Ik(_) => true,
            });
        }
        // Weights on the bone pass to its parent (or drop), then renormalise.
        let bound: Vec<Id<Object>> = doc
            .objects
            .iter()
            .filter(|o| o.skin.as_ref().is_some_and(|k| k.skeleton == sid))
            .map(|o| o.id)
            .collect();
        for oid in bound {
            let o = doc.objects.get_mut(oid).expect("listed");
            if let Some(Skin {
                bind: Bind::Rigid { bone },
                ..
            }) = &mut o.skin
            {
                *bone = remap(*bone).or(heir.and_then(remap)).unwrap_or(0);
            }
            if let Some(mesh) = &mut o.mesh {
                for w in &mut Arc::make_mut(mesh).weights {
                    let pairs: Vec<(u16, f32)> = (0..4)
                        .filter_map(|k| {
                            let b = if w.bones[k] == gone {
                                heir?
                            } else {
                                w.bones[k]
                            };
                            Some((remap(b)?, w.weights[k]))
                        })
                        .collect();
                    let (bones, weights) = normalise(&pairs);
                    *w = Influences { bones, weights };
                }
            }
        }
        Ok(json!({ "deleted": self.bone }))
    }
}

/// The object as a skinned mesh: (id, object world matrix, vertices in skeleton space).
fn skinnable(
    doc: &Document,
    reference: &str,
) -> Result<(Id<Object>, glam::Mat4, Vec<Vec3>), CommandError> {
    let id = object(doc, reference)?;
    let o = doc.objects.get(id).expect("resolved");
    let mesh = o
        .mesh
        .as_deref()
        .filter(|m| !m.positions.is_empty())
        .ok_or_else(|| CommandError::invalid(format!("'{}' has no mesh to bind.", o.name)))?;
    let world = doc.world_matrix(o);
    Ok((
        id,
        world,
        mesh.positions
            .iter()
            .map(|p| world.transform_point3(*p))
            .collect(),
    ))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigBindAuto {
    pub object: String,
    pub skeleton: Option<String>,
}

impl Command for RigBindAuto {
    const NAME: &'static str = "rig.bind_auto";
    const DESCRIPTION: &'static str = "Bind a mesh to a skeleton with automatic weights from each vertex's distance to the bones (at most 4 per vertex, summing to 1).";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        if s.bones.is_empty() {
            return Err(CommandError::invalid(
                "That skeleton has no bones yet. Call rig.bone_add first.",
            ));
        }
        let segments: Vec<(Vec3, Vec3)> = s.bones.iter().map(|b| (b.head, b.tail)).collect();
        let (id, _, points) = skinnable(doc, &self.object)?;
        let mut weights = Vec::with_capacity(points.len());
        for (i, p) in points.iter().enumerate() {
            if i % 4096 == 0 {
                cx.step(i as f32 / points.len() as f32)?;
            }
            let (bones, w) = auto_weights(*p, &segments);
            weights.push(Influences { bones, weights: w });
        }
        let o = cx.doc().objects.get_mut(id).expect("resolved");
        o.skin = Some(Skin {
            skeleton: sid,
            bind: Bind::Weighted,
        });
        Arc::make_mut(o.mesh.as_mut().expect("checked")).weights = weights;
        Ok(json!({ "object": id, "skeleton": sid, "vertices": points.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigBindRigid {
    pub object: String,
    pub bone: String,
    pub skeleton: Option<String>,
    /// Bind only the edited mesh's selected faces (their vertices) to the bone.
    #[serde(default)]
    pub selected_faces: bool,
}

impl Command for RigBindRigid {
    const NAME: &'static str = "rig.bind_rigid";
    const DESCRIPTION: &'static str = "Bind a whole object (or its selected faces) to one bone at 100%, so it moves rigidly with that bone.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let bone = bone_index(doc.skeletons.get(sid).expect("resolved"), &self.bone)?;
        let id = object(doc, &self.object)?;
        let faces: BTreeSet<u32> = doc.selection.faces.clone();
        let editing = doc.selection.mode == SelectMode::Face && doc.selection.object == Some(id);
        let o = doc.objects.get_mut(id).expect("resolved");
        let Some(mesh) = o.mesh.as_mut() else {
            return Err(CommandError::invalid(format!(
                "'{}' has no mesh to bind.",
                o.name
            )));
        };
        let mesh = Arc::make_mut(mesh);
        let one = Influences {
            bones: [bone, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        };
        if self.selected_faces {
            if !editing || faces.is_empty() {
                return Err(CommandError::new(
                    ErrorCode::NothingSelected,
                    "No faces of that object are selected.",
                ));
            }
            mesh.weights.resize(mesh.positions.len(), one);
            for f in &faces {
                for c in &mesh.faces[*f as usize].corners {
                    mesh.weights[c.vertex as usize] = one;
                }
            }
            o.skin = Some(Skin {
                skeleton: sid,
                bind: Bind::Weighted,
            });
        } else {
            mesh.weights = vec![one; mesh.positions.len()];
            o.skin = Some(Skin {
                skeleton: sid,
                bind: Bind::Rigid { bone },
            });
        }
        Ok(json!({ "object": id, "bone": self.bone }))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WeightMode {
    /// Raise the bone's weight by `value`.
    Add,
    /// Lower the bone's weight by `value`; the rest goes to the nearest other bone.
    Subtract,
    /// Make the bone's weight exactly `value`.
    Set,
    /// Blend each vertex's weights with its neighbours' by `value` (0..1).
    Smooth,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigWeightPaint {
    pub object: String,
    pub bone: String,
    pub mode: WeightMode,
    /// 0..1.
    pub value: f32,
    /// Vertex indices. Default the edited mesh's selected vertices.
    pub vertices: Option<Vec<u32>>,
}

impl Command for RigWeightPaint {
    const NAME: &'static str = "rig.weight_paint";
    const DESCRIPTION: &'static str = "Paint bone weights on vertices: add, subtract, set or smooth. Every vertex keeps at most 4 bones summing to 1.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if !(self.value.is_finite() && (0.0..=1.0).contains(&self.value)) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "value must be between 0 and 1.",
            ));
        }
        let doc = cx.doc();
        let id = object(doc, &self.object)?;
        let o = doc.objects.get(id).expect("resolved");
        let sid = o.skin.as_ref().map(|s| s.skeleton).ok_or_else(|| {
            CommandError::invalid(format!(
                "'{}' is not bound to a skeleton. Call rig.bind_auto or rig.bind_rigid first.",
                o.name
            ))
        })?;
        let s = doc.skeletons.get(sid).expect("bound skeleton exists");
        let bone = bone_index(s, &self.bone)?;
        let segments: Vec<(Vec3, Vec3)> = s.bones.iter().map(|b| (b.head, b.tail)).collect();
        let world = doc.world_matrix(o);
        let mesh = o.mesh.as_deref().expect("bound objects have meshes");
        let vertices: Vec<u32> = match self.vertices {
            Some(v) => v,
            None if doc.selection.object == Some(id) => {
                doc.selection.touched_vertices(mesh).into_iter().collect()
            }
            None => Vec::new(),
        };
        if vertices.is_empty() {
            return Err(CommandError::new(
                ErrorCode::NothingSelected,
                "No vertices given or selected.",
            ));
        }
        if let Some(v) = vertices
            .iter()
            .find(|v| **v as usize >= mesh.positions.len())
        {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!("'{}' has no vertex {v}.", o.name),
            ));
        }
        let neighbours: BTreeMap<u32, BTreeSet<u32>> = if matches!(self.mode, WeightMode::Smooth) {
            let mut n: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
            for f in &mesh.faces {
                for k in 0..f.corners.len() {
                    let (a, b) = (
                        f.corners[k].vertex,
                        f.corners[(k + 1) % f.corners.len()].vertex,
                    );
                    n.entry(a).or_default().insert(b);
                    n.entry(b).or_default().insert(a);
                }
            }
            n
        } else {
            BTreeMap::new()
        };
        let positions: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|p| world.transform_point3(*p))
            .collect();
        let o = doc.objects.get_mut(id).expect("resolved");
        let mesh = Arc::make_mut(o.mesh.as_mut().expect("checked"));
        mesh.weights
            .resize(mesh.positions.len(), Influences::default());
        let before = mesh.weights.clone();
        let pairs = |w: &Influences| -> Vec<(u16, f32)> {
            (0..4)
                .filter(|k| w.weights[*k] > 0.0)
                .map(|k| (w.bones[k], w.weights[k]))
                .collect()
        };
        for v in &vertices {
            let current = pairs(&before[*v as usize]);
            let own: f32 = current
                .iter()
                .filter(|(b, _)| *b == bone)
                .map(|(_, w)| w)
                .sum();
            let others: Vec<(u16, f32)> = current
                .iter()
                .copied()
                .filter(|(b, _)| *b != bone)
                .collect();
            let other_total: f32 = others.iter().map(|(_, w)| w).sum();
            let target = match self.mode {
                WeightMode::Add => (own + self.value).min(1.0),
                WeightMode::Subtract => (own - self.value).max(0.0),
                WeightMode::Set => self.value,
                WeightMode::Smooth => {
                    let mut sum: BTreeMap<u16, f32> = BTreeMap::new();
                    let ring = neighbours.get(v).map_or(0, |n| n.len()).max(1) as f32;
                    for n in neighbours.get(v).into_iter().flatten() {
                        for (b, w) in pairs(&before[*n as usize]) {
                            *sum.entry(b).or_default() += w / ring;
                        }
                    }
                    let blended: Vec<(u16, f32)> = current
                        .iter()
                        .map(|(b, w)| (*b, w * (1.0 - self.value)))
                        .chain(sum.into_iter().map(|(b, w)| (b, w * self.value)))
                        .collect();
                    let (bones, weights) = normalise(&blended);
                    mesh.weights[*v as usize] = Influences { bones, weights };
                    continue;
                }
            };
            // The rest of the vertex's weight is shared by the other bones as
            // before; with none left, the nearest other bone takes it.
            let rest = 1.0 - target;
            let mut result: Vec<(u16, f32)> = vec![(bone, target)];
            if other_total > 0.0 {
                result.extend(others.iter().map(|(b, w)| (*b, w / other_total * rest)));
            } else if rest > 0.0 {
                let nearest = (0..segments.len() as u16)
                    .filter(|b| *b != bone)
                    .min_by(|a, b| {
                        let d = |i: u16| {
                            quarry_ops::rig::segment_distance(
                                positions[*v as usize],
                                segments[i as usize].0,
                                segments[i as usize].1,
                            )
                        };
                        d(*a).total_cmp(&d(*b))
                    });
                match nearest {
                    Some(n) => result.push((n, rest)),
                    None => result[0].1 = 1.0, // a one-bone skeleton keeps everything
                }
            }
            let (bones, weights) = normalise(&result);
            mesh.weights[*v as usize] = Influences { bones, weights };
        }
        if let Some(Skin {
            bind: bind @ Bind::Rigid { .. },
            ..
        }) = &mut o.skin
        {
            *bind = Bind::Weighted;
        }
        Ok(json!({ "vertices": vertices.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigIkAdd {
    pub skeleton: Option<String>,
    /// The chain's end bone (a shin or forearm); its parent is the chain's upper bone.
    pub tip_bone: String,
    /// Where the tip should reach [x, y, z]. Default the tip's tail.
    pub target: Option<[f32; 3]>,
    /// Which way the middle joint bends [x, y, z]. Default 1 m in front of it.
    pub pole: Option<[f32; 3]>,
}

impl Command for RigIkAdd {
    const NAME: &'static str = "rig.ik_add";
    const DESCRIPTION: &'static str = "Add a two-bone IK chain ending at a bone, with a target the tip reaches for and a pole the middle joint points at.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        let tip = bone_index(s, &self.tip_bone)?;
        if s.bones[tip as usize].parent.is_none() {
            return Err(CommandError::invalid(
                "The tip bone needs a parent: an IK chain is two bones.",
            ));
        }
        if s.ik_chains.iter().any(|c| c.tip_bone == tip) {
            return Err(CommandError::new(
                ErrorCode::Conflict,
                "That bone already ends an IK chain. Use rig.ik_set.",
            ));
        }
        let bone = &s.bones[tip as usize];
        let target = match self.target {
            Some(t) => vec3(t, "target")?,
            None => bone.tail,
        };
        let pole = match self.pole {
            Some(p) => vec3(p, "pole")?,
            None => bone.head + Vec3::Z,
        };
        let s = doc.skeletons.get_mut(sid).expect("resolved");
        s.ik_chains.push(IkChain {
            tip_bone: tip,
            target,
            pole,
        });
        Ok(json!({ "chain": s.ik_chains.len() - 1, "tip_bone": self.tip_bone }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigIkSet {
    pub skeleton: Option<String>,
    /// The chain's tip bone name.
    pub tip_bone: String,
    pub target: Option<[f32; 3]>,
    pub pole: Option<[f32; 3]>,
}

impl Command for RigIkSet {
    const NAME: &'static str = "rig.ik_set";
    const DESCRIPTION: &'static str = "Move an IK chain's target or pole and pose the chain so its tip reaches the target (stopping at full stretch if out of reach).";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        let tip = bone_index(s, &self.tip_bone)?;
        let chain = s
            .ik_chains
            .iter()
            .position(|c| c.tip_bone == tip)
            .ok_or_else(|| {
                CommandError::not_found(format!(
                    "No IK chain ends at '{}'. Call rig.ik_add first.",
                    self.tip_bone
                ))
            })?;
        let target = self.target.map(|t| vec3(t, "target")).transpose()?;
        let pole = self.pole.map(|p| vec3(p, "pole")).transpose()?;
        let s = doc.skeletons.get_mut(sid).expect("resolved");
        if let Some(t) = target {
            s.ik_chains[chain].target = t;
        }
        if let Some(p) = pole {
            s.ik_chains[chain].pole = p;
        }
        s.solve_ik(chain);
        let target_now = s.ik_chains[chain].target;
        let world = s.pose_world();
        let b = &s.bones[tip as usize];
        let reached = world[tip as usize].transform_point3(Vec3::Y * b.head.distance(b.tail));
        let gap = reached.distance(s.ik_chains[chain].target);
        let mut reply = json!({ "tip": reached, "gap": gap });
        if gap > 1e-3 {
            reply["warning"] =
                json!("The target is out of reach; the chain stops at full stretch.");
        }
        // Auto-key the target (IK is keyed by its target and baked on export).
        let tool = doc.view.anim;
        if tool.autokey
            && let Some(clip) = tool
                .clip
                .filter(|c| doc.clips.get(*c).is_some_and(|c| c.skeleton == sid))
        {
            let key = crate::model::Key {
                time: tool.time,
                value: crate::model::KeyValue::Vec3(target_now),
                interp: crate::model::Interp::Linear,
                handles: None,
            };
            let clip = doc.clips.get_mut(clip).expect("checked");
            super::anim::upsert(
                clip,
                TrackTarget::Ik(chain as u16),
                crate::model::Channel::IkTarget,
                key,
            );
        }
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigMirrorBones {
    pub skeleton: Option<String>,
}

impl Command for RigMirrorBones {
    const NAME: &'static str = "rig.mirror_bones";
    const DESCRIPTION: &'static str = "Copy every .L bone to a .R bone mirrored across X (and the other way), with matching parents. Existing partners are updated.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get_mut(sid).expect("resolved");
        let flip = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
        let mut made = Vec::new();
        let sources: Vec<usize> = (0..s.bones.len())
            .filter(|i| s.bones[*i].name.ends_with(".L") || s.bones[*i].name.ends_with("_L"))
            .collect();
        let mut unmatched = Vec::new();
        for i in sources {
            let bone = s.bones[i].clone();
            let name = mirror_name(&bone.name).expect("has a side");
            let parent = bone.parent.map(|p| {
                let pname = &s.bones[p as usize].name;
                mirror_name(pname).and_then(|m| s.index(&m)).unwrap_or(p)
            });
            let mirrored = Bone {
                name: name.clone(),
                parent,
                head: flip(bone.head),
                tail: flip(bone.tail),
                roll: -bone.roll,
            };
            match s.index(&name) {
                Some(j) => s.bones[j as usize] = mirrored,
                None if s.bones.len() < MAX_BONES => {
                    s.bones.push(mirrored);
                    made.push(name);
                }
                None => unmatched.push(name),
            }
        }
        // Parents that were created later in the loop: fix them now.
        for i in 0..s.bones.len() {
            if let Some(src) = mirror_name(&s.bones[i].name).and_then(|n| s.index(&n))
                && let Some(sp) = s.bones[src as usize].parent
                && let Some(mp) = mirror_name(&s.bones[sp as usize].name).and_then(|n| s.index(&n))
                && s.bones[i].name.ends_with(".R")
            {
                s.bones[i].parent = Some(mp);
            }
        }
        let mut reply = json!({ "created": made });
        if !unmatched.is_empty() {
            reply["warning"] = json!(format!("No room for: {}.", unmatched.join(", ")));
        }
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigMirrorWeights {
    pub object: String,
}

impl Command for RigMirrorWeights {
    const NAME: &'static str = "rig.mirror_weights";
    const DESCRIPTION: &'static str = "Copy weights from the +X side of a mesh to its mirror vertices on -X, swapping .L bones for .R. Bones without a partner are named in a warning.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = object(doc, &self.object)?;
        let o = doc.objects.get(id).expect("resolved");
        let sid = o.skin.as_ref().map(|k| k.skeleton).ok_or_else(|| {
            CommandError::invalid(format!("'{}' is not bound to a skeleton.", o.name))
        })?;
        let s = doc.skeletons.get(sid).expect("bound").clone();
        let swap: Vec<Option<u16>> = s
            .bones
            .iter()
            .map(|b| match mirror_name(&b.name) {
                Some(m) => s.index(&m),
                None => Some(s.index(&b.name).expect("own")),
            })
            .collect();
        let o = doc.objects.get_mut(id).expect("resolved");
        let mesh = Arc::make_mut(o.mesh.as_mut().expect("bound"));
        mesh.weights
            .resize(mesh.positions.len(), Influences::default());
        let mut unmatched: BTreeSet<String> = BTreeSet::new();
        let mut copied = 0;
        for v in 0..mesh.positions.len() {
            let p = mesh.positions[v];
            if p.x <= 1e-5 {
                continue;
            }
            let target = Vec3::new(-p.x, p.y, p.z);
            let Some(twin) = mesh
                .positions
                .iter()
                .position(|q| q.distance(target) < 1e-4)
            else {
                continue;
            };
            let w = mesh.weights[v];
            let pairs: Vec<(u16, f32)> = (0..4)
                .filter(|k| w.weights[*k] > 0.0)
                .filter_map(|k| match swap[w.bones[k] as usize] {
                    Some(b) => Some((b, w.weights[k])),
                    None => {
                        unmatched.insert(s.bones[w.bones[k] as usize].name.clone());
                        None
                    }
                })
                .collect();
            let (bones, weights) = normalise(&pairs);
            mesh.weights[twin] = Influences { bones, weights };
            copied += 1;
        }
        let mut reply = json!({ "vertices": copied });
        if !unmatched.is_empty() {
            reply["warning"] = json!(format!(
                "These bones have no mirrored partner, so their weights were not copied: {}.",
                unmatched.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigPose {
    pub skeleton: Option<String>,
    pub bone: String,
    /// Rotation from rest, Euler degrees [x, y, z] in the bone's frame.
    pub rotation: Option<[f32; 3]>,
    /// Offset from rest in the bone's frame [x, y, z].
    pub translation: Option<[f32; 3]>,
}

impl Command for RigPose {
    const NAME: &'static str = "rig.pose";
    const DESCRIPTION: &'static str = "Pose a bone: rotate it (Euler degrees in its own frame) or move it from its rest position. Children follow.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get_mut(sid).expect("resolved");
        let i = bone_index(s, &self.bone)? as usize;
        s.pose.resize(s.bones.len(), BonePose::default());
        if let Some(r) = self.rotation {
            let r = vec3(r, "rotation")?;
            s.pose[i].rotation = Quat::from_euler(
                EulerRot::XYZ,
                r.x.to_radians(),
                r.y.to_radians(),
                r.z.to_radians(),
            );
        }
        if let Some(t) = self.translation {
            s.pose[i].translation = vec3(t, "translation")?;
        }
        let pose = s.pose[i];
        super::anim::autokey_bone(doc, sid, i as u16, self.translation.is_some());
        Ok(json!({ "bone": self.bone, "pose": pose }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigPoseReset {
    pub skeleton: Option<String>,
}

impl Command for RigPoseReset {
    const NAME: &'static str = "rig.pose_reset";
    const DESCRIPTION: &'static str = "Put every bone back in its rest pose.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        doc.skeletons.get_mut(sid).expect("resolved").pose.clear();
        Ok(json!({ "skeleton": sid }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuerySkeleton {
    pub skeleton: Option<String>,
}

impl Command for QuerySkeleton {
    const NAME: &'static str = "query.skeleton";
    const DESCRIPTION: &'static str = "A skeleton's bones (index, name, parent, head, tail, roll), IK chains and which objects are bound to it.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let sid = pick_skeleton(doc, self.skeleton.as_deref())?;
        let s = doc.skeletons.get(sid).expect("resolved");
        let bones: Vec<_> = s
            .bones
            .iter()
            .enumerate()
            .map(|(i, b)| json!({ "index": i, "name": b.name, "parent": b.parent.map(|p| &s.bones[p as usize].name), "head": b.head, "tail": b.tail, "roll": b.roll.to_degrees() }))
            .collect();
        let bound: Vec<_> = doc
            .objects
            .iter()
            .filter(|o| o.skin.as_ref().is_some_and(|k| k.skeleton == sid))
            .map(|o| &o.name)
            .collect();
        Ok(
            json!({ "id": sid, "name": s.name, "bones": bones, "ik_chains": s.ik_chains, "bound": bound }),
        )
    }
}
