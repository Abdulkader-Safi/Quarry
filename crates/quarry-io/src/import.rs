//! glTF 2.0 / .glb import (FR-IMP-01) into a document fragment that the
//! registry merges in: one object per node (names kept, hierarchy kept),
//! base-colour textures as images, skins as skeletons, animations as clips.
//! Skin joints become bones, not objects. Cubic-spline keys keep their values
//! and import as linear (Quarry has no glTF tangents).

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use quarry_core::id::Id;
use quarry_core::model::{
    Bind, Bone, Channel, Clip, Corner, Document, Face, Filter, Image, ImageRole, Influences,
    Interp, Key, KeyValue, MAX_BONES, Mesh, Object, ObjectKind, Skeleton, Skin, Track, TrackTarget,
};
use quarry_core::{CommandError, ErrorCode};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub fn import(path: &Path) -> Result<Document, CommandError> {
    let (gltf, buffers, images) = gltf::import(path).map_err(|e| {
        CommandError::new(
            ErrorCode::Io,
            format!(
                "Could not import {}: {e}. Check it is a glTF 2.0 .gltf or .glb.",
                path.display()
            ),
        )
    })?;
    let name = path
        .file_stem()
        .map_or("import".into(), |s| s.to_string_lossy().into_owned());
    let mut doc = Document::new(name);
    let data = |b: gltf::Buffer| Some(&*buffers[b.index()]);

    // Images: only those a material uses as base colour.
    let mut image_ids: HashMap<usize, Id<Image>> = HashMap::new();
    let mut material_image: HashMap<usize, Id<Image>> = HashMap::new();
    for material in gltf.materials() {
        let Some(info) = material.pbr_metallic_roughness().base_color_texture() else {
            continue;
        };
        let texture = info.texture();
        let source = texture.source().index();
        let id = match image_ids.get(&source) {
            Some(id) => *id,
            None => {
                let raw = &images[source];
                let filter = match texture.sampler().mag_filter() {
                    Some(gltf::texture::MagFilter::Nearest) => Filter::Nearest,
                    _ => Filter::Linear,
                };
                let name = texture
                    .source()
                    .name()
                    .map_or(format!("image_{source}"), str::to_string);
                let id = doc.images.insert(Image {
                    id: Id::new(0, 0),
                    name: doc.images.unique_name(&name),
                    role: ImageRole::Texture,
                    size: [raw.width, raw.height],
                    tile_px: doc.settings.tile_px,
                    padding: 0,
                    zip_path: String::new(),
                    source_path: None,
                    source_hash: 0,
                    filter,
                    watch: false,
                    pixels: Arc::from(rgba8(raw)),
                });
                image_ids.insert(source, id);
                id
            }
        };
        if let Some(index) = material.index() {
            material_image.insert(index, id);
        }
    }

    // Skins: bones in joint order, rest pose from the inverse bind matrices.
    let mut joint_nodes: HashSet<usize> = HashSet::new();
    let mut skeleton_of_skin: HashMap<usize, (Id<Skeleton>, Vec<usize>)> = HashMap::new();
    for skin in gltf.skins() {
        let joints: Vec<gltf::Node> = skin.joints().collect();
        if joints.len() > MAX_BONES {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!(
                    "{} has a skin with {} joints; Quarry allows {MAX_BONES}.",
                    path.display(),
                    joints.len()
                ),
            ));
        }
        let ibms: Vec<Mat4> = skin
            .reader(data)
            .read_inverse_bind_matrices()
            .map(|m| m.map(|c| Mat4::from_cols_array_2d(&c)).collect())
            .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);
        let index_of: HashMap<usize, usize> = joints
            .iter()
            .enumerate()
            .map(|(i, j)| (j.index(), i))
            .collect();
        let heads: Vec<Vec3> = ibms
            .iter()
            .map(|m| m.inverse().transform_point3(Vec3::ZERO))
            .collect();
        let mut bones = Vec::with_capacity(joints.len());
        for (i, joint) in joints.iter().enumerate() {
            let parent = gltf
                .nodes()
                .find(|n| n.children().any(|c| c.index() == joint.index()))
                .and_then(|p| index_of.get(&p.index()))
                .map(|p| *p as u16);
            let children: Vec<Vec3> = joint
                .children()
                .filter_map(|c| index_of.get(&c.index()))
                .map(|c| heads[*c])
                .collect();
            let tail = if children.is_empty() {
                let up = ibms[i]
                    .inverse()
                    .transform_vector3(Vec3::Y)
                    .normalize_or(Vec3::Y);
                let length =
                    parent.map_or(0.25, |p| heads[i].distance(heads[p as usize]).max(0.05));
                heads[i] + up * length
            } else {
                children.iter().sum::<Vec3>() / children.len() as f32
            };
            bones.push(Bone {
                name: joint.name().map_or(format!("bone_{i}"), str::to_string),
                parent,
                head: heads[i],
                tail,
                roll: 0.0,
            });
            joint_nodes.insert(joint.index());
        }
        let name = skin.name().map_or("Armature".to_string(), str::to_string);
        let id = doc.skeletons.insert(Skeleton {
            id: Id::new(0, 0),
            name: doc.skeletons.unique_name(&name),
            bones,
            ik_chains: Vec::new(),
            pose: Vec::new(),
        });
        skeleton_of_skin.insert(
            skin.index(),
            (id, joints.iter().map(|j| j.index()).collect()),
        );
    }

    // Nodes: one object each, except skin joints.
    let mut objects: HashMap<usize, Id<Object>> = HashMap::new();
    let mut order = 0;
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            add_node(
                &mut doc,
                &node,
                None,
                &mut objects,
                &joint_nodes,
                &material_image,
                &skeleton_of_skin,
                &buffers,
                &mut order,
            )?;
        }
    }

    // Animations: one clip per animation per skeleton it moves.
    for animation in gltf.animations() {
        let mut tracks: BTreeMap<Id<Skeleton>, Vec<Track>> = BTreeMap::new();
        let mut length = 0.0f32;
        for channel in animation.channels() {
            let node = channel.target().node().index();
            let Some((skeleton, bone)) = skeleton_of_skin.values().find_map(|(s, joints)| {
                joints
                    .iter()
                    .position(|j| *j == node)
                    .map(|b| (*s, b as u16))
            }) else {
                continue; // node animation outside a skeleton: not supported yet
            };
            let reader = channel.reader(data);
            let Some(times) = reader.read_inputs() else {
                continue;
            };
            let times: Vec<f32> = times.collect();
            length = length.max(times.last().copied().unwrap_or(0.0));
            let sampler = channel.sampler().interpolation();
            let interp = match sampler {
                gltf::animation::Interpolation::Step => Interp::Step,
                _ => Interp::Linear,
            };
            let cubic = sampler == gltf::animation::Interpolation::CubicSpline;
            let (channel_kind, values): (Channel, Vec<KeyValue>) = match reader.read_outputs() {
                Some(gltf::animation::util::ReadOutputs::Translations(t)) => (
                    Channel::Translation,
                    t.map(|v| KeyValue::Vec3(Vec3::from(v))).collect(),
                ),
                Some(gltf::animation::util::ReadOutputs::Scales(s)) => (
                    Channel::Scale,
                    s.map(|v| KeyValue::Vec3(Vec3::from(v))).collect(),
                ),
                Some(gltf::animation::util::ReadOutputs::Rotations(r)) => (
                    Channel::Rotation,
                    r.into_f32()
                        .map(|q| KeyValue::Quat(Quat::from_array(q).normalize()))
                        .collect(),
                ),
                _ => continue,
            };
            // Cubic splines store in-tangent, value, out-tangent per key.
            let values: Vec<KeyValue> = if cubic {
                values.chunks(3).filter_map(|c| c.get(1).copied()).collect()
            } else {
                values
            };
            let keys = times
                .iter()
                .zip(values)
                .map(|(time, value)| Key {
                    time: *time,
                    value,
                    interp,
                    handles: None,
                })
                .collect();
            tracks.entry(skeleton).or_default().push(Track {
                target: TrackTarget::Bone(bone),
                channel: channel_kind,
                keys,
            });
        }
        for (skeleton, tracks) in tracks {
            let name = animation
                .name()
                .map_or(format!("clip_{}", animation.index()), str::to_string);
            doc.clips.insert(Clip {
                id: Id::new(0, 0),
                name: doc.clips.unique_name(&name),
                skeleton,
                length: length.max(1.0 / 30.0),
                looping: true,
                tracks,
            });
        }
    }
    Ok(doc)
}

#[allow(clippy::too_many_arguments)]
fn add_node(
    doc: &mut Document,
    node: &gltf::Node,
    parent: Option<Id<Object>>,
    objects: &mut HashMap<usize, Id<Object>>,
    joints: &HashSet<usize>,
    material_image: &HashMap<usize, Id<Image>>,
    skins: &HashMap<usize, (Id<Skeleton>, Vec<usize>)>,
    buffers: &[gltf::buffer::Data],
    order: &mut i32,
) -> Result<(), CommandError> {
    if joints.contains(&node.index()) {
        // Joints are bones; whatever hangs under them keeps the joint's parent.
        for child in node.children() {
            add_node(
                doc,
                &child,
                parent,
                objects,
                joints,
                material_image,
                skins,
                buffers,
                order,
            )?;
        }
        return Ok(());
    }
    let mesh = node
        .mesh()
        .map(|m| read_mesh(doc, &m, material_image, buffers));
    let kind = if mesh.is_some() {
        ObjectKind::Mesh
    } else {
        ObjectKind::Layer
    };
    let name = node
        .name()
        .map_or(format!("node_{}", node.index()), str::to_string);
    let mut object = Object::new(doc.objects.unique_name(&name), kind);
    let (t, r, s) = node.transform().decomposed();
    object.transform.translation = Vec3::from(t);
    object.transform.rotation = Quat::from_array(r).normalize();
    object.transform.scale = Vec3::from(s);
    object.parent = parent;
    object.order = *order;
    *order += 1;
    object.mesh = mesh.map(Arc::new);
    // Quarry's own extras come back as markers, collision and properties.
    if let Some(quarry) = node
        .extras()
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw.get()).ok())
        .and_then(|v| v.get("quarry").cloned())
    {
        if let Some(kind) = quarry
            .get("marker")
            .and_then(|m| serde_json::from_value(m.clone()).ok())
        {
            object.kind = ObjectKind::Marker;
            object.marker_type = Some(kind);
        }
        object.collision = quarry
            .get("collision")
            .and_then(|c| c.as_bool())
            .unwrap_or(false);
        if let Some(props) = quarry
            .get("props")
            .and_then(|p| serde_json::from_value(p.clone()).ok())
        {
            object.properties = props;
        }
    }
    if let Some((skeleton, _)) = node.skin().and_then(|s| skins.get(&s.index())) {
        object.skin = Some(Skin {
            skeleton: *skeleton,
            bind: Bind::Weighted,
        });
    }
    let id = doc.objects.insert(object);
    objects.insert(node.index(), id);
    for child in node.children() {
        add_node(
            doc,
            &child,
            Some(id),
            objects,
            joints,
            material_image,
            skins,
            buffers,
            order,
        )?;
    }
    Ok(())
}

fn read_mesh(
    doc: &Document,
    mesh: &gltf::Mesh,
    material_image: &HashMap<usize, Id<Image>>,
    buffers: &[gltf::buffer::Data],
) -> Mesh {
    let mut out = Mesh::default();
    let mut skinned = false;
    for primitive in mesh.primitives() {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            continue;
        }
        let reader = primitive.reader(|b| Some(&*buffers[b.index()]));
        let Some(positions) = reader.read_positions() else {
            continue;
        };
        let base = out.positions.len() as u32;
        out.positions.extend(positions.map(Vec3::from));
        let count = out.positions.len() - base as usize;
        let image = primitive
            .material()
            .index()
            .and_then(|m| material_image.get(&m))
            .copied();
        let size = image
            .and_then(|i| doc.images.get(i))
            .map_or(Vec2::splat(doc.settings.tile_px as f32), |i| {
                Vec2::new(i.size[0] as f32, i.size[1] as f32)
            });
        let uvs: Vec<Vec2> = reader
            .read_tex_coords(0)
            .map(|t| t.into_f32().map(|v| Vec2::from(v) * size).collect())
            .unwrap_or_else(|| vec![Vec2::ZERO; count]);
        let colors: Vec<Vec4> = reader
            .read_colors(0)
            .map(|c| c.into_rgba_f32().map(Vec4::from).collect())
            .unwrap_or_else(|| vec![Vec4::ONE; count]);
        let joints: Option<Vec<[u16; 4]>> = reader.read_joints(0).map(|j| j.into_u16().collect());
        let weights: Option<Vec<[f32; 4]>> = reader.read_weights(0).map(|w| w.into_f32().collect());
        if let (Some(j), Some(w)) = (&joints, &weights) {
            if !skinned && base > 0 {
                out.weights.resize(base as usize, Influences::default());
            }
            skinned = true;
            out.weights
                .extend(j.iter().zip(w).map(|(j, w)| influences(*j, *w)));
        } else if skinned {
            out.weights
                .extend(std::iter::repeat_n(Influences::default(), count));
        }
        let indices: Vec<u32> = reader
            .read_indices()
            .map(|i| i.into_u32().collect())
            .unwrap_or_else(|| (0..count as u32).collect());
        for tri in indices.chunks_exact(3) {
            if tri.iter().any(|i| *i as usize >= count) {
                continue;
            }
            out.faces.push(Face {
                corners: tri
                    .iter()
                    .map(|i| Corner {
                        vertex: base + i,
                        uv: uvs.get(*i as usize).copied().unwrap_or(Vec2::ZERO),
                        color: colors.get(*i as usize).copied().unwrap_or(Vec4::ONE),
                    })
                    .collect(),
                image,
                smooth_group: 0,
                tile: None,
                hidden: false,
            });
        }
    }
    out
}

/// Top 4 by weight, normalised to sum 1, heaviest first (FR-RIG-05).
fn influences(joints: [u16; 4], weights: [f32; 4]) -> Influences {
    let mut pairs: Vec<(u16, f32)> = joints
        .into_iter()
        .zip(weights)
        .filter(|(_, w)| *w > 0.0)
        .collect();
    pairs.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f32 = pairs.iter().map(|p| p.1).sum();
    let mut out = Influences::default();
    for (k, (j, w)) in pairs.into_iter().take(4).enumerate() {
        out.bones[k] = j;
        out.weights[k] = if total > 0.0 { w / total } else { 0.0 };
    }
    out
}

/// glTF image data to sRGB RGBA8.
fn rgba8(raw: &gltf::image::Data) -> Vec<u8> {
    use gltf::image::Format::*;
    let n = (raw.width * raw.height) as usize;
    let px = &raw.pixels;
    let at16 = |i: usize| px.get(i * 2 + 1).copied().unwrap_or(0);
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n {
        let rgba = match raw.format {
            R8 => [px[i], px[i], px[i], 255],
            R8G8 => [px[i * 2], px[i * 2 + 1], 0, 255],
            R8G8B8 => [px[i * 3], px[i * 3 + 1], px[i * 3 + 2], 255],
            R8G8B8A8 => [px[i * 4], px[i * 4 + 1], px[i * 4 + 2], px[i * 4 + 3]],
            R16 => [at16(i), at16(i), at16(i), 255],
            R16G16 => [at16(i * 2), at16(i * 2 + 1), 0, 255],
            R16G16B16 => [at16(i * 3), at16(i * 3 + 1), at16(i * 3 + 2), 255],
            R16G16B16A16 => [
                at16(i * 4),
                at16(i * 4 + 1),
                at16(i * 4 + 2),
                at16(i * 4 + 3),
            ],
            _ => [128, 128, 128, 255],
        };
        out.extend_from_slice(&rgba);
    }
    out
}
