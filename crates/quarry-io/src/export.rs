//! glTF export (FR-EXP-01, 03, 04, 05): one self-contained `.glb` per call by
//! default, or a `.gltf` + `.bin` + PNGs for debugging.
//!
//! One glTF mesh per mesh object, one primitive per image. Vertices are shared
//! wherever position, normal, UV and colour all match. Instances reuse their
//! source's mesh. Markers, collision flags and properties become node names
//! and `extras.quarry`.

use crate::glb::GlbWriter;
use glam::{Mat4, Vec2, Vec3};
use gltf_json::accessor::{ComponentType, Type};
use gltf_json::buffer::Target;
use gltf_json::material::{
    AlphaCutoff, AlphaMode, PbrBaseColorFactor, PbrMetallicRoughness, StrengthFactor,
};
use gltf_json::mesh::{Mode, Primitive, Semantic};
use gltf_json::scene::UnitQuaternion;
use gltf_json::texture::{Info, MagFilter, MinFilter, Sampler, WrappingMode};
use gltf_json::validation::Checked::Valid;
use gltf_json::{Index, Material, Node, Scene, Texture};
use quarry_core::command::ExportRequest;
use quarry_core::id::Id;
use quarry_core::model::{
    Document, ExportFormat, Filter, Image, Mesh, Object, ObjectKind, Skeleton,
};
use quarry_core::{CommandError, ErrorCode};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub fn export(doc: &Document, request: &ExportRequest) -> Result<Value, CommandError> {
    let separate = request.format == ExportFormat::GltfSeparate;
    let fail = |why: String| {
        CommandError::new(
            ErrorCode::Io,
            format!("Could not export to {}: {why}.", request.path.display()),
        )
    };
    let stem = request
        .path
        .file_stem()
        .map_or("export".into(), |s| s.to_string_lossy().into_owned());
    let mut out = Builder::new(doc, separate, &stem);
    out.build(&request.objects, request.scale);
    let stats = out.stats;
    let mut files = Vec::new();
    if separate {
        let dir = request.path.parent().unwrap_or(Path::new("."));
        for (name, png) in &out.loose_images {
            let path = dir.join(name);
            crate::settings::write_atomic(&path, png).map_err(|e| fail(e.to_string()))?;
            files.push(path);
        }
        let (mut root, bin) = out.writer.into_parts();
        let bin_name = format!("{stem}.bin");
        root.buffers[0].uri = Some(bin_name.clone());
        let bin_path = dir.join(bin_name);
        crate::settings::write_atomic(&bin_path, &bin).map_err(|e| fail(e.to_string()))?;
        files.push(bin_path);
        let json = root.to_vec_pretty().map_err(|e| fail(e.to_string()))?;
        crate::settings::write_atomic(&request.path, &json).map_err(|e| fail(e.to_string()))?;
    } else {
        let bytes = out.writer.finish().map_err(|e| fail(e.to_string()))?;
        crate::settings::write_atomic(&request.path, &bytes).map_err(|e| fail(e.to_string()))?;
    }
    files.push(request.path.clone());
    let report = check(&request.path);
    Ok(json!({
        "files": files,
        "nodes": stats.nodes,
        "meshes": stats.meshes,
        "vertices": stats.vertices,
        "triangles": stats.triangles,
        "materials": stats.materials,
        "report": report,
    }))
}

/// Read the export back with the `gltf` crate: a parse, index and accessor
/// check. CI also runs the Khronos validator on the goldens.
pub fn check(path: &Path) -> Value {
    match gltf::import(path) {
        Ok((doc, _, _)) => json!({
            "errors": [],
            "scenes": doc.scenes().count(),
            "nodes": doc.nodes().count(),
        }),
        Err(e) => json!({ "errors": [e.to_string()] }),
    }
}

#[derive(Clone, Copy, Default)]
struct Stats {
    nodes: usize,
    meshes: usize,
    vertices: usize,
    triangles: usize,
    materials: usize,
}

/// A skeleton's glTF skin and its joint node per bone.
type SkinNodes = (Index<gltf_json::Skin>, Vec<Index<Node>>);

struct Builder<'a> {
    doc: &'a Document,
    writer: GlbWriter,
    separate: bool,
    stem: String,
    loose_images: Vec<(String, Vec<u8>)>,
    meshes: HashMap<Id<Object>, Index<gltf_json::Mesh>>,
    /// Per skeleton: its glTF skin and one joint node per bone.
    skins: HashMap<Id<Skeleton>, SkinNodes>,
    /// Skeleton roots and skinned meshes, which sit at the top of the scene.
    extra_roots: Vec<Index<Node>>,
    materials: HashMap<Option<Id<Image>>, Index<Material>>,
    samplers: HashMap<bool, Index<Sampler>>,
    stats: Stats,
}

impl<'a> Builder<'a> {
    fn new(doc: &'a Document, separate: bool, stem: &str) -> Self {
        Self {
            doc,
            writer: GlbWriter::default(),
            separate,
            stem: stem.to_string(),
            loose_images: Vec::new(),
            meshes: HashMap::new(),
            skins: HashMap::new(),
            extra_roots: Vec::new(),
            materials: HashMap::new(),
            samplers: HashMap::new(),
            stats: Stats::default(),
        }
    }

    fn build(&mut self, roots: &[Id<Object>], scale: f32) {
        let mut scene_nodes = Vec::new();
        for id in roots {
            let Some(object) = self.doc.objects.get(*id) else {
                continue;
            };
            // A root exported without its parent keeps its place in the world.
            let world = self.doc.world_matrix(object);
            let mut node = self.node(object, Some(world));
            node = scaled(node, scale);
            scene_nodes.push(self.writer.root.push(node));
        }
        // Children after their roots, breadth first, linked by index.
        let mut queue: Vec<(Id<Object>, Index<Node>)> = roots
            .iter()
            .zip(&scene_nodes)
            .map(|(id, node)| (*id, *node))
            .collect();
        while let Some((parent, parent_node)) = queue.pop() {
            let mut kids: Vec<&Object> = self
                .doc
                .objects
                .iter()
                .filter(|o| o.parent == Some(parent))
                .collect();
            kids.sort_by_key(|o| (o.order, o.id));
            for kid in kids {
                let node = self.node(kid, None);
                let index = self.writer.root.push(node);
                if self.writer.root.nodes[index.value()].skin.is_some() {
                    // glTF ignores a skinned node's place in the tree.
                    self.extra_roots.push(index);
                } else {
                    self.writer.root.nodes[parent_node.value()]
                        .children
                        .get_or_insert_with(Vec::new)
                        .push(index);
                }
                queue.push((kid.id, index));
            }
        }
        scene_nodes.append(&mut self.extra_roots);
        let scene = self.writer.root.push(Scene {
            extensions: None,
            extras: Default::default(),
            name: Some(self.doc.name.clone()),
            nodes: scene_nodes,
        });
        self.writer.root.scene = Some(scene);
        self.stats.nodes = self.writer.root.nodes.len();
        self.stats.materials = self.writer.root.materials.len();
    }

    /// A node for `object`, using `world` instead of its local transform when given.
    fn node(&mut self, object: &Object, world: Option<Mat4>) -> Node {
        let (scale, rotation, translation) = match world {
            Some(m) => m.to_scale_rotation_translation(),
            None => (
                object.transform.scale,
                object.transform.rotation,
                object.transform.translation,
            ),
        };
        let mesh_source = match object.kind {
            ObjectKind::Instance => object.instance_of,
            ObjectKind::Mesh => Some(object.id),
            _ => None,
        };
        let mesh = mesh_source.and_then(|source| self.mesh(source));
        // Skinned meshes are written in skeleton space with no transform.
        let skin = skinned(object).and_then(|sid| self.skin(sid));
        let (scale, rotation, translation) = match skin {
            Some(_) => (Vec3::ONE, glam::Quat::IDENTITY, Vec3::ZERO),
            None => (scale, rotation, translation),
        };
        let mut quarry = serde_json::Map::new();
        if let Some(kind) = object.marker_type {
            quarry.insert("marker".into(), json!(kind));
        }
        if object.collision {
            quarry.insert("collision".into(), json!(true));
        }
        if !object.properties.is_empty() {
            quarry.insert("props".into(), json!(object.properties));
        }
        let extras = (!quarry.is_empty())
            .then(|| serde_json::value::to_raw_value(&json!({ "quarry": quarry })).ok())
            .flatten();
        Node {
            name: Some(object.name.clone()),
            mesh,
            skin,
            translation: (translation != Vec3::ZERO).then(|| translation.into()),
            rotation: (rotation != glam::Quat::IDENTITY)
                .then(|| UnitQuaternion(rotation.normalize().into())),
            scale: (scale != Vec3::ONE).then(|| scale.into()),
            extras,
            ..Default::default()
        }
    }

    fn mesh(&mut self, id: Id<Object>) -> Option<Index<gltf_json::Mesh>> {
        if let Some(index) = self.meshes.get(&id) {
            return Some(*index);
        }
        let object = self.doc.objects.get(id)?;
        let mesh = object.mesh.as_deref().filter(|m| !m.faces.is_empty())?;
        let mut by_image: BTreeMap<Option<Id<Image>>, Vec<usize>> = BTreeMap::new();
        for (i, f) in mesh.faces.iter().enumerate() {
            by_image.entry(f.image).or_default().push(i);
        }
        let bake = skinned(object).map(|_| self.doc.world_matrix(object));
        let mut primitives = Vec::new();
        for (image, faces) in by_image {
            let material = self.material(image);
            primitives.push(self.primitive(mesh, &faces, image, material, bake));
        }
        let index = self.writer.root.push(gltf_json::Mesh {
            extensions: None,
            extras: Default::default(),
            name: Some(object.name.clone()),
            primitives,
            weights: None,
        });
        self.stats.meshes += 1;
        self.meshes.insert(id, index);
        Some(index)
    }

    fn primitive(
        &mut self,
        mesh: &Mesh,
        faces: &[usize],
        image: Option<Id<Image>>,
        material: Index<Material>,
        bake: Option<Mat4>,
    ) -> Primitive {
        let size = image
            .and_then(|i| self.doc.images.get(i))
            .map_or(Vec2::splat(self.doc.settings.tile_px as f32), |i| {
                Vec2::new(i.size[0] as f32, i.size[1] as f32)
            });
        let normals = vertex_normals(mesh);

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut out_normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut joints: Vec<[u8; 4]> = Vec::new();
        let mut weights: Vec<[f32; 4]> = Vec::new();
        let mut seen: HashMap<[u32; 20], u32> = HashMap::new();
        let normal_bake = bake.map(|m| m.inverse().transpose());
        for &f in faces {
            let face = &mesh.faces[f];
            let corner_ids: Vec<u32> = face
                .corners
                .iter()
                .map(|c| {
                    let p = mesh.positions[c.vertex as usize];
                    let n = normals[&(f, c.vertex)];
                    let (p, n) = match (bake, normal_bake) {
                        (Some(m), Some(nm)) => (
                            m.transform_point3(p),
                            nm.transform_vector3(n).normalize_or(n),
                        ),
                        _ => (p, n),
                    };
                    let skin = mesh
                        .weights
                        .get(c.vertex as usize)
                        .filter(|_| bake.is_some());
                    let (j, w) = skin.map_or(([0u8; 4], [0f32; 4]), |s| {
                        (s.bones.map(|b| b as u8), s.weights)
                    });
                    // Untextured faces carry no UVs, so they do not split vertices.
                    let uv = if image.is_some() {
                        c.uv / size
                    } else {
                        Vec2::ZERO
                    };
                    let key = [
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        n.x.to_bits(),
                        n.y.to_bits(),
                        n.z.to_bits(),
                        uv.x.to_bits(),
                        uv.y.to_bits(),
                        c.color.x.to_bits(),
                        c.color.y.to_bits(),
                        c.color.z.to_bits(),
                        c.color.w.to_bits(),
                        u32::from_le_bytes(j),
                        w[0].to_bits(),
                        w[1].to_bits(),
                        w[2].to_bits(),
                        w[3].to_bits(),
                        0,
                        0,
                        0,
                    ];
                    *seen.entry(key).or_insert_with(|| {
                        positions.push(p.into());
                        out_normals.push(n.into());
                        uvs.push(uv.into());
                        colors.push(c.color.into());
                        joints.push(j);
                        weights.push(w);
                        (positions.len() - 1) as u32
                    })
                })
                .collect();
            for k in 1..corner_ids.len() - 1 {
                indices.extend([corner_ids[0], corner_ids[k], corner_ids[k + 1]]);
            }
        }
        self.stats.vertices += positions.len();
        self.stats.triangles += indices.len() / 3;

        let w = &mut self.writer;
        let mut attributes = BTreeMap::new();
        attributes.insert(
            Valid(Semantic::Positions),
            w.accessor(
                &positions,
                ComponentType::F32,
                Type::Vec3,
                Some(Target::ArrayBuffer),
                true,
            ),
        );
        attributes.insert(
            Valid(Semantic::Normals),
            w.accessor(
                &out_normals,
                ComponentType::F32,
                Type::Vec3,
                Some(Target::ArrayBuffer),
                false,
            ),
        );
        if image.is_some() {
            attributes.insert(
                Valid(Semantic::TexCoords(0)),
                w.accessor(
                    &uvs,
                    ComponentType::F32,
                    Type::Vec2,
                    Some(Target::ArrayBuffer),
                    false,
                ),
            );
        }
        if bake.is_some() {
            attributes.insert(
                Valid(Semantic::Joints(0)),
                w.accessor(
                    &joints,
                    ComponentType::U8,
                    Type::Vec4,
                    Some(Target::ArrayBuffer),
                    false,
                ),
            );
            attributes.insert(
                Valid(Semantic::Weights(0)),
                w.accessor(
                    &weights,
                    ComponentType::F32,
                    Type::Vec4,
                    Some(Target::ArrayBuffer),
                    false,
                ),
            );
        }
        if colors.iter().any(|c| *c != [1.0; 4]) {
            attributes.insert(
                Valid(Semantic::Colors(0)),
                w.accessor(
                    &colors,
                    ComponentType::F32,
                    Type::Vec4,
                    Some(Target::ArrayBuffer),
                    false,
                ),
            );
        }
        let indices = if positions.len() <= u16::MAX as usize {
            let small: Vec<u16> = indices.iter().map(|i| *i as u16).collect();
            w.accessor(
                &small,
                ComponentType::U16,
                Type::Scalar,
                Some(Target::ElementArrayBuffer),
                false,
            )
        } else {
            w.accessor(
                &indices,
                ComponentType::U32,
                Type::Scalar,
                Some(Target::ElementArrayBuffer),
                false,
            )
        };
        Primitive {
            attributes,
            extensions: None,
            extras: Default::default(),
            indices: Some(indices),
            material: Some(material),
            mode: Valid(Mode::Triangles),
            targets: None,
        }
    }

    /// The glTF skin for a skeleton: joint nodes in rest pose under a root
    /// node named after the skeleton, and inverse bind matrices.
    fn skin(&mut self, sid: Id<Skeleton>) -> Option<Index<gltf_json::Skin>> {
        if let Some((skin, _)) = self.skins.get(&sid) {
            return Some(*skin);
        }
        let s = self
            .doc
            .skeletons
            .get(sid)
            .filter(|s| !s.bones.is_empty())?;
        let local = s.rest_local();
        let joints: Vec<Index<Node>> = s
            .bones
            .iter()
            .zip(&local)
            .map(|(bone, m)| {
                let (scale, rotation, translation) = m.to_scale_rotation_translation();
                self.writer.root.push(Node {
                    name: Some(bone.name.clone()),
                    translation: Some(translation.into()),
                    rotation: Some(UnitQuaternion(rotation.normalize().into())),
                    scale: (!scale.abs_diff_eq(Vec3::ONE, 1e-6)).then(|| scale.into()),
                    ..Default::default()
                })
            })
            .collect();
        let mut roots = Vec::new();
        for (i, bone) in s.bones.iter().enumerate() {
            match bone.parent {
                Some(p) => self.writer.root.nodes[joints[p as usize].value()]
                    .children
                    .get_or_insert_with(Vec::new)
                    .push(joints[i]),
                None => roots.push(joints[i]),
            }
        }
        let root = self.writer.root.push(Node {
            name: Some(s.name.clone()),
            children: Some(roots),
            ..Default::default()
        });
        self.extra_roots.push(root);
        let ibm: Vec<[f32; 16]> = s
            .rest_world()
            .iter()
            .map(|m| m.inverse().to_cols_array())
            .collect();
        let ibm = self
            .writer
            .accessor(&ibm, ComponentType::F32, Type::Mat4, None, false);
        let skin = self.writer.root.push(gltf_json::Skin {
            extensions: None,
            extras: Default::default(),
            inverse_bind_matrices: Some(ibm),
            joints: joints.clone(),
            name: Some(s.name.clone()),
            skeleton: Some(root),
        });
        self.skins.insert(sid, (skin, joints));
        Some(skin)
    }

    fn material(&mut self, image: Option<Id<Image>>) -> Index<Material> {
        if let Some(index) = self.materials.get(&image) {
            return *index;
        }
        let doc = self.doc;
        let found = image
            .and_then(|i| doc.images.get(i))
            .filter(|i| i.pixels.len() == (i.size[0] * i.size[1] * 4) as usize);
        let (texture, cutout, name) = match found {
            Some(img) => {
                let cutout = img.pixels.chunks_exact(4).any(|p| p[3] < 255);
                (Some(self.texture(img)), cutout, img.name.clone())
            }
            None => (None, false, "untextured".into()),
        };
        let index = self.writer.root.push(Material {
            alpha_cutoff: cutout.then_some(AlphaCutoff(0.5)),
            alpha_mode: Valid(if cutout {
                AlphaMode::Mask
            } else {
                AlphaMode::Opaque
            }),
            double_sided: false,
            name: Some(name),
            pbr_metallic_roughness: PbrMetallicRoughness {
                base_color_factor: PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
                base_color_texture: texture.map(|index| Info {
                    index,
                    tex_coord: 0,
                    extensions: None,
                    extras: Default::default(),
                }),
                metallic_factor: StrengthFactor(0.0),
                roughness_factor: StrengthFactor(1.0),
                metallic_roughness_texture: None,
                extensions: None,
                extras: Default::default(),
            },
            ..Default::default()
        });
        self.materials.insert(image, index);
        index
    }

    fn texture(&mut self, image: &Image) -> Index<Texture> {
        let nearest = image.filter == Filter::Nearest;
        let sampler = *self.samplers.entry(nearest).or_insert_with(|| {
            self.writer.root.push(Sampler {
                mag_filter: Some(Valid(if nearest {
                    MagFilter::Nearest
                } else {
                    MagFilter::Linear
                })),
                min_filter: Some(Valid(if nearest {
                    MinFilter::Nearest
                } else {
                    MinFilter::Linear
                })),
                name: None,
                wrap_s: Valid(WrappingMode::Repeat),
                wrap_t: Valid(WrappingMode::Repeat),
                extensions: None,
                extras: Default::default(),
            })
        });
        let rgba = image::RgbaImage::from_raw(image.size[0], image.size[1], image.pixels.to_vec())
            .expect("size checked");
        let mut png = std::io::Cursor::new(Vec::new());
        rgba.write_to(&mut png, image::ImageFormat::Png)
            .expect("PNG encodes in memory");
        let png = png.into_inner();
        let source = if self.separate {
            let file = format!("{}_{}.png", self.stem, safe_name(&image.name));
            self.loose_images.push((file.clone(), png));
            self.writer.root.push(gltf_json::Image {
                buffer_view: None,
                mime_type: None,
                name: Some(image.name.clone()),
                uri: Some(file),
                extensions: None,
                extras: Default::default(),
            })
        } else {
            let view = self.writer.view(&png, None);
            self.writer.root.push(gltf_json::Image {
                buffer_view: Some(view),
                mime_type: Some(gltf_json::image::MimeType("image/png".into())),
                name: Some(image.name.clone()),
                uri: None,
                extensions: None,
                extras: Default::default(),
            })
        };
        self.writer.root.push(Texture {
            name: Some(image.name.clone()),
            sampler: Some(sampler),
            source,
            extensions: None,
            extras: Default::default(),
        })
    }
}

/// The skeleton a mesh object is skinned to, when it has usable weights.
fn skinned(object: &Object) -> Option<Id<Skeleton>> {
    let mesh = object.mesh.as_deref()?;
    let skin = object.skin.as_ref()?;
    (object.kind == ObjectKind::Mesh
        && !mesh.positions.is_empty()
        && mesh.weights.len() == mesh.positions.len())
    .then_some(skin.skeleton)
}

fn scaled(mut node: Node, scale: f32) -> Node {
    if scale != 1.0 {
        node.translation = node.translation.map(|t| t.map(|v| v * scale));
        let s = node.scale.unwrap_or([1.0; 3]);
        node.scale = Some(s.map(|v| v * scale));
    }
    node
}

fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Normal for each (face, vertex): the face's own for flat faces, the average
/// of its smoothing group's faces at that vertex otherwise (FR-MESH-09).
fn vertex_normals(mesh: &Mesh) -> HashMap<(usize, u32), Vec3> {
    let face_normal = |f: &quarry_core::model::Face| {
        let p: Vec<Vec3> = f
            .corners
            .iter()
            .map(|c| mesh.positions[c.vertex as usize])
            .collect();
        let mut n = Vec3::ZERO;
        for i in 0..p.len() {
            n += p[i].cross(p[(i + 1) % p.len()]);
        }
        n
    };
    let mut smooth: HashMap<(u8, u32), Vec3> = HashMap::new();
    let mut flat: Vec<Vec3> = Vec::with_capacity(mesh.faces.len());
    for f in &mesh.faces {
        let n = face_normal(f);
        flat.push(n.normalize_or(Vec3::Y));
        if f.smooth_group != 0 {
            for c in &f.corners {
                *smooth.entry((f.smooth_group, c.vertex)).or_default() += n;
            }
        }
    }
    let mut out = HashMap::new();
    for (i, f) in mesh.faces.iter().enumerate() {
        for c in &f.corners {
            let n = if f.smooth_group == 0 {
                flat[i]
            } else {
                smooth[&(f.smooth_group, c.vertex)].normalize_or(flat[i])
            };
            out.insert((i, c.vertex), n);
        }
    }
    out
}
