//! M0 skin spike: a 3-bone bending bar written with our GLB writer.
//!
//! `QUARRY_UPDATE_GOLDENS=1 cargo test -p quarry-io --test skin_bar` rewrites
//! `tests/goldens/skin-bar.glb`; otherwise the output must match it byte for byte.

use glam::{Mat4, Quat, Vec3};
use gltf_json::accessor::{ComponentType, Type};
use gltf_json::animation::{Channel, Interpolation, Property, Sampler, Target as AnimTarget};
use gltf_json::buffer::Target;
use gltf_json::mesh::{Mode, Primitive, Semantic};
use gltf_json::validation::Checked::Valid;
use gltf_json::{Animation, Index, Mesh, Node, Scene, Skin};
use quarry_io::glb::GlbWriter;
use std::collections::BTreeMap;
use std::path::PathBuf;

const HALF: f32 = 0.1; // bar is 0.2 m square
const RINGS: usize = 7; // y = 0.0, 0.5 .. 3.0
const BEND_DEGREES: f32 = 40.0;

/// Bone influences per ring: a hard bone in the middle, an even blend at each joint.
fn ring_weights(ring: usize) -> ([u8; 4], [f32; 4]) {
    match ring {
        0 | 1 => ([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
        2 => ([0, 1, 0, 0], [0.5, 0.5, 0.0, 0.0]),
        3 => ([1, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
        4 => ([1, 2, 0, 0], [0.5, 0.5, 0.0, 0.0]),
        _ => ([2, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
    }
}

fn build_bar() -> Vec<u8> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut joints: Vec<[u8; 4]> = Vec::new();
    let mut weights: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    // Four sides, each a strip of quads with its own vertices so normals stay flat.
    let corners = [[-HALF, HALF], [HALF, HALF], [HALF, -HALF], [-HALF, -HALF]]; // (x, z)
    for side in 0..4 {
        let a = corners[side];
        let b = corners[(side + 1) % 4];
        let normal = Vec3::new(a[0] + b[0], 0.0, a[1] + b[1]).normalize();
        let base = positions.len() as u16;
        for ring in 0..RINGS {
            let y = ring as f32 * 0.5;
            let (j, w) = ring_weights(ring);
            for c in [a, b] {
                positions.push([c[0], y, c[1]]);
                normals.push(normal.into());
                joints.push(j);
                weights.push(w);
            }
        }
        for ring in 0..RINGS as u16 - 1 {
            let (a0, b0, a1, b1) = (
                base + ring * 2,
                base + ring * 2 + 1,
                base + ring * 2 + 2,
                base + ring * 2 + 3,
            );
            indices.extend_from_slice(&[a0, b0, b1, a0, b1, a1]);
        }
    }
    // Caps, wound so they face away from the bar.
    for (ring, ny) in [(0, -1.0f32), (RINGS - 1, 1.0)] {
        let y = ring as f32 * 0.5;
        let (j, w) = ring_weights(ring);
        let base = positions.len() as u16;
        for c in corners {
            positions.push([c[0], y, c[1]]);
            normals.push([0.0, ny, 0.0]);
            joints.push(j);
            weights.push(w);
        }
        if ny > 0.0 {
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    let mut glb = GlbWriter::default();
    let position = glb.accessor(
        &positions,
        ComponentType::F32,
        Type::Vec3,
        Some(Target::ArrayBuffer),
        true,
    );
    let normal = glb.accessor(
        &normals,
        ComponentType::F32,
        Type::Vec3,
        Some(Target::ArrayBuffer),
        false,
    );
    let joint = glb.accessor(
        &joints,
        ComponentType::U8,
        Type::Vec4,
        Some(Target::ArrayBuffer),
        false,
    );
    let weight = glb.accessor(
        &weights,
        ComponentType::F32,
        Type::Vec4,
        Some(Target::ArrayBuffer),
        false,
    );
    let index = glb.accessor(
        &indices,
        ComponentType::U16,
        Type::Scalar,
        Some(Target::ElementArrayBuffer),
        false,
    );

    let attributes = BTreeMap::from([
        (Valid(Semantic::Positions), position),
        (Valid(Semantic::Normals), normal),
        (Valid(Semantic::Joints(0)), joint),
        (Valid(Semantic::Weights(0)), weight),
    ]);
    let mesh = glb.root.push(Mesh {
        extensions: None,
        extras: Default::default(),
        name: Some("bar".into()),
        primitives: vec![Primitive {
            attributes,
            extensions: None,
            extras: Default::default(),
            indices: Some(index),
            material: None,
            mode: Valid(Mode::Triangles),
            targets: None,
        }],
        weights: None,
    });

    // Bones: a chain up +Y, 1 m each. Node indices: 0 mesh, 1..=3 bones.
    let bone = |name: &str, y: f32, child: Option<u32>| Node {
        name: Some(name.into()),
        translation: Some([0.0, y, 0.0]),
        children: child.map(|c| vec![Index::new(c)]),
        ..Default::default()
    };
    let skin_index = Index::<Skin>::new(0);
    glb.root.push(Node {
        name: Some("bar".into()),
        mesh: Some(mesh),
        skin: Some(skin_index),
        ..Default::default()
    });
    let root_bone = glb.root.push(bone("bone.0", 0.0, Some(2)));
    glb.root.push(bone("bone.1", 1.0, Some(3)));
    glb.root.push(bone("bone.2", 1.0, None));

    let inverse_binds: Vec<[f32; 16]> = (0..3)
        .map(|i| Mat4::from_translation(Vec3::new(0.0, -(i as f32), 0.0)).to_cols_array())
        .collect();
    let ibm = glb.accessor(&inverse_binds, ComponentType::F32, Type::Mat4, None, false);
    glb.root.push(Skin {
        extensions: None,
        extras: Default::default(),
        inverse_bind_matrices: Some(ibm),
        joints: (1..=3).map(Index::new).collect(),
        name: Some("bar_rig".into()),
        skeleton: Some(root_bone),
    });

    // One clip: bones 1 and 2 bend around Z and back over 2 s.
    let times = [0.0f32, 1.0, 2.0];
    let bend = Quat::from_rotation_z(BEND_DEGREES.to_radians());
    let rotations: Vec<[f32; 4]> = [Quat::IDENTITY, bend, Quat::IDENTITY]
        .iter()
        .map(|q| q.to_array())
        .collect();
    let input = glb.accessor(&times, ComponentType::F32, Type::Scalar, None, true);
    let output = glb.accessor(&rotations, ComponentType::F32, Type::Vec4, None, false);
    let sampler = Sampler {
        extensions: None,
        extras: Default::default(),
        input,
        interpolation: Valid(Interpolation::Linear),
        output,
    };
    let channel = |node: u32| Channel {
        sampler: Index::new(0),
        target: AnimTarget {
            extensions: None,
            extras: Default::default(),
            node: Index::new(node),
            path: Valid(Property::Rotation),
        },
        extensions: None,
        extras: Default::default(),
    };
    glb.root.push(Animation {
        extensions: None,
        extras: Default::default(),
        channels: vec![channel(2), channel(3)],
        name: Some("bend".into()),
        samplers: vec![sampler],
    });

    let scene = glb.root.push(Scene {
        extensions: None,
        extras: Default::default(),
        name: Some("bar".into()),
        nodes: vec![Index::new(0), root_bone],
    });
    glb.root.scene = Some(scene);

    glb.finish().expect("glTF JSON serialises")
}

#[test]
fn skin_bar_matches_golden_and_bends() {
    let bytes = build_bar();
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/goldens/skin-bar.glb");
    if std::env::var_os("QUARRY_UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &bytes).unwrap();
    }
    let expected = std::fs::read(&golden).expect("golden exists; set QUARRY_UPDATE_GOLDENS=1");
    assert!(bytes == expected, "skin-bar.glb differs from the golden");

    let (doc, buffers, _) = gltf::import_slice(&bytes).expect("glTF reader accepts it");
    let skin = doc.skins().next().expect("one skin");
    assert_eq!(skin.joints().count(), 3);
    let clips: Vec<_> = doc.animations().filter_map(|a| a.name()).collect();
    assert_eq!(clips, ["bend"]);

    let primitive = doc.meshes().next().unwrap().primitives().next().unwrap();
    let reader = primitive.reader(|b| Some(&buffers[b.index()]));
    let positions: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
    let weights: Vec<[f32; 4]> = reader.read_weights(0).unwrap().into_f32().collect();
    let joints: Vec<[u16; 4]> = reader.read_joints(0).unwrap().into_u16().collect();
    for w in &weights {
        assert!(
            (w.iter().sum::<f32>() - 1.0).abs() < 1e-6,
            "weights sum to 1"
        );
    }

    // CPU-skin the bar at the clip's peak (t = 1 s): the top must swing to -X.
    let bend = Quat::from_rotation_z(BEND_DEGREES.to_radians());
    let world1 = Mat4::from_rotation_translation(bend, Vec3::Y);
    let world = [
        Mat4::IDENTITY,
        world1,
        world1 * Mat4::from_rotation_translation(bend, Vec3::Y),
    ];
    let skinned = |i: usize| {
        let p = Vec3::from(positions[i]);
        (0..4).fold(Vec3::ZERO, |acc, k| {
            let j = joints[i][k] as usize;
            let bind = Mat4::from_translation(Vec3::new(0.0, -(j as f32), 0.0));
            acc + (world[j] * bind).transform_point3(p) * weights[i][k]
        })
    };
    let top = (0..positions.len())
        .max_by(|&a, &b| positions[a][1].total_cmp(&positions[b][1]))
        .unwrap();
    let moved = skinned(top);
    assert!(moved.x < -1.0, "top of the bar swings left, got {moved}");
    let bottom = (0..positions.len())
        .min_by(|&a, &b| positions[a][1].total_cmp(&positions[b][1]))
        .unwrap();
    assert!(
        skinned(bottom).distance(Vec3::from(positions[bottom])) < 1e-6,
        "root ring stays put"
    );
}
