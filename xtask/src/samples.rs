//! `cargo run -p xtask -- samples`: rebuild `assets/samples/` from code.
//!
//! - `tiles-16px.png`: an 8x8 grid of 16 px tiles drawn here, so it is CC0
//!   with no third-party art in it.
//! - `sample-robot.quarry`: a low-poly robot (six-sided limbs, a faceted
//!   head) built only through registry commands, what an agent or a person
//!   does, with a 13-bone skeleton, leg IK, a painted texture and "idle" and
//!   "walk" clips.

use quarry_core::{CommandRegistry, Source};
use quarry_io::project::QuarryFiles;
use serde_json::{Value, json};
use std::path::Path;

pub fn build(root: &Path) {
    let out = root.join("assets/samples");
    std::fs::create_dir_all(&out).expect("samples folder");
    let tiles = out.join("tiles-16px.png");
    tileset().save(&tiles).expect("write the tileset");
    robot(&out.join("sample-robot.quarry"));
    println!("wrote {} and sample-robot.quarry", tiles.display());
}

/// 8x8 tiles of 16 px: grass, dirt, stone, brick, wood, metal, water, and
/// plain colours, each with a little texture so filtering is easy to see.
fn tileset() -> image::RgbaImage {
    let base: [[u8; 3]; 8] = [
        [86, 140, 62],   // grass
        [128, 92, 60],   // dirt
        [128, 128, 132], // stone
        [160, 72, 52],   // brick
        [150, 108, 64],  // wood
        [110, 120, 136], // metal
        [60, 104, 168],  // water
        [220, 210, 180], // sand
    ];
    image::RgbaImage::from_fn(128, 128, |x, y| {
        let (col, row) = ((x / 16) as usize, (y / 16) as usize);
        let (u, v) = (x % 16, y % 16);
        let [r, g, b] = base[row];
        // Columns vary the pattern: plain, speckle, bricks, planks, panels, edges…
        let shade: i32 = match col {
            0 => 0,
            1 => {
                if (u * 7 + v * 13) % 11 == 0 {
                    -24
                } else {
                    0
                }
            }
            2 => {
                if v % 8 == 0 || (u + if v / 8 % 2 == 0 { 0 } else { 8 }) % 16 == 0 {
                    -40
                } else {
                    0
                }
            }
            3 => {
                if u % 4 == 0 {
                    -28
                } else if (v * 3 + u) % 17 == 0 {
                    -12
                } else {
                    0
                }
            }
            4 => {
                if u == 0 || v == 0 {
                    30
                } else if u == 15 || v == 15 {
                    -36
                } else {
                    0
                }
            }
            5 => {
                if (u + v) % 6 == 0 {
                    18
                } else {
                    0
                }
            }
            6 => -((u as i32 - 8).abs() + (v as i32 - 8).abs()) * 3,
            _ => {
                if u < 2 || v < 2 {
                    -50
                } else {
                    10
                }
            }
        };
        let c = |ch: u8| (ch as i32 + shade).clamp(0, 255) as u8;
        image::Rgba([c(r), c(g), c(b), 255])
    })
}

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("sample robot, {name}: {}", e.message))
}

fn robot(path: &Path) {
    let mut r = CommandRegistry::new();
    r.set_storage(Box::new(QuarryFiles::default()));
    run(
        &mut r,
        "project.new",
        json!({ "name": "Sample robot", "path": path }),
    );

    // Body parts, standing about 2 m tall, facing +Z: six-sided limbs, a
    // faceted head, a box torso for the painted chest panel.
    let parts = [
        ("Hips", "cylinder", [0.0, 1.0, 0.0], [0.5, 0.2, 0.32]),
        ("Torso", "box", [0.0, 1.35, 0.0], [0.6, 0.5, 0.35]),
        ("Head", "sphere", [0.0, 1.8, 0.0], [0.36, 0.32, 0.32]),
        (
            "UpperArm.L",
            "cylinder",
            [0.42, 1.45, 0.0],
            [0.13, 0.4, 0.13],
        ),
        (
            "LowerArm.L",
            "cylinder",
            [0.72, 1.45, 0.0],
            [0.11, 0.4, 0.11],
        ),
        ("Thigh.L", "cylinder", [0.15, 0.7, 0.0], [0.17, 0.4, 0.17]),
        ("Shin.L", "cylinder", [0.15, 0.25, 0.0], [0.15, 0.45, 0.15]),
    ];
    let mirrored = parts
        .iter()
        .filter(|p| p.0.ends_with(".L"))
        .map(|&(name, kind, at, size)| {
            (name.replace(".L", ".R"), kind, [-at[0], at[1], at[2]], size)
        })
        .collect::<Vec<_>>();
    let all = parts
        .iter()
        .map(|&(name, kind, at, size)| (name.to_string(), kind, at, size))
        .chain(mirrored);
    for (name, kind, at, size) in all {
        run(
            &mut r,
            "mesh.add_primitive",
            json!({ "kind": kind, "segments": 6, "name": name, "position": at, "size": size }),
        );
        if name.contains("Arm") {
            // Cylinders stand on Y; arms run along X.
            run(&mut r, "select.by_ids", json!({ "objects": [name] }));
            run(&mut r, "mesh.transform", json!({ "rotate": [0, 0, 90] }));
        }
    }
    run(&mut r, "select.none", json!({}));

    // One painted 32x32 texture on the torso, with a chest panel and a light.
    run(
        &mut r,
        "select.mode",
        json!({ "mode": "face", "object": "Torso" }),
    );
    run(&mut r, "paint.palette_lock", json!({ "lock": false }));
    run(
        &mut r,
        "paint.texture_new",
        json!({ "width": 32, "height": 32, "color": [110, 120, 136, 255], "name": "robot" }),
    );
    run(&mut r, "uv.unwrap", json!({}));
    run(
        &mut r,
        "paint.rect",
        json!({ "image": "robot", "x": 4, "y": 4, "width": 8, "height": 8, "rgba": [60, 66, 80, 255] }),
    );
    run(
        &mut r,
        "paint.rect",
        json!({ "image": "robot", "x": 6, "y": 6, "width": 2, "height": 2, "rgba": [255, 196, 40, 255] }),
    );
    run(&mut r, "select.mode", json!({ "mode": "object" }));

    // 13 bones: root, spine, chest, neck, head, and per side upper and lower
    // arm, thigh and shin.
    run(&mut r, "rig.skeleton_new", json!({ "name": "robot_rig" }));
    let bones = [
        ("root", None, [0.0, 0.9, 0.0], [0.0, 1.1, 0.0]),
        ("spine", Some("root"), [0.0, 1.1, 0.0], [0.0, 1.3, 0.0]),
        ("chest", Some("spine"), [0.0, 1.3, 0.0], [0.0, 1.6, 0.0]),
        ("neck", Some("chest"), [0.0, 1.6, 0.0], [0.0, 1.65, 0.0]),
        ("head", Some("neck"), [0.0, 1.65, 0.0], [0.0, 1.95, 0.0]),
        (
            "upper_arm.L",
            Some("chest"),
            [0.22, 1.45, 0.0],
            [0.62, 1.45, 0.0],
        ),
        (
            "lower_arm.L",
            Some("upper_arm.L"),
            [0.62, 1.45, 0.0],
            [0.92, 1.45, 0.0],
        ),
        ("thigh.L", Some("root"), [0.15, 0.9, 0.0], [0.15, 0.5, 0.0]),
        (
            "shin.L",
            Some("thigh.L"),
            [0.15, 0.5, 0.0],
            [0.15, 0.02, 0.0],
        ),
    ];
    for (name, parent, head, tail) in bones {
        let mut args = json!({ "name": name, "head": head, "tail": tail });
        if let Some(p) = parent {
            args["parent"] = json!(p);
        }
        run(&mut r, "rig.bone_add", args);
    }
    run(&mut r, "rig.mirror_bones", json!({}));
    let skeleton = r.document().skeletons.iter().next().unwrap();
    assert_eq!(skeleton.bones.len(), 13, "the sample robot has 13 bones");

    for (part, bone) in [
        ("Hips", "root"),
        ("Torso", "chest"),
        ("Head", "head"),
        ("UpperArm.L", "upper_arm.L"),
        ("LowerArm.L", "lower_arm.L"),
        ("UpperArm.R", "upper_arm.R"),
        ("LowerArm.R", "lower_arm.R"),
        ("Thigh.L", "thigh.L"),
        ("Shin.L", "shin.L"),
        ("Thigh.R", "thigh.R"),
        ("Shin.R", "shin.R"),
    ] {
        run(
            &mut r,
            "rig.bind_rigid",
            json!({ "object": part, "bone": bone }),
        );
    }
    for side in ["L", "R"] {
        let x = if side == "L" { 0.15 } else { -0.15 };
        run(
            &mut r,
            "rig.ik_add",
            json!({ "tip_bone": format!("shin.{side}"), "target": [x, 0.02, 0.0], "pole": [x, 0.5, 2.0] }),
        );
    }

    // idle: a slow breath in the chest and a small head turn.
    run(
        &mut r,
        "anim.clip_new",
        json!({ "name": "idle", "length": 2.0 }),
    );
    for (t, chest, head) in [(0.0, 0.0, 0.0), (1.0, 3.0, 6.0), (2.0, 0.0, 0.0)] {
        run(
            &mut r,
            "anim.key_set",
            json!({ "bone": "chest", "channel": "rotation", "time": t, "value": [chest, 0, 0], "interp": "ease_in_out" }),
        );
        run(
            &mut r,
            "anim.key_set",
            json!({ "bone": "head", "channel": "rotation", "time": t, "value": [0, head, 0], "interp": "ease_in_out" }),
        );
    }
    // walk: feet step through IK targets, arms swing against them, the body bobs.
    run(
        &mut r,
        "anim.clip_new",
        json!({ "name": "walk", "length": 1.0 }),
    );
    let step = |phase: f32| -> ([f32; 3], [f32; 3]) {
        // Forward/back 0.25 m, lifting 0.12 m on the forward swing.
        let (s, c) = (
            (phase * std::f32::consts::TAU).sin(),
            (phase * std::f32::consts::TAU).cos(),
        );
        ([0.0, 0.02 + 0.12 * s.max(0.0), 0.25 * c], [0.0, 0.0, 0.0])
    };
    for i in 0..=4 {
        let t = i as f32 * 0.25;
        for (side, x, offset) in [("L", 0.15, 0.0), ("R", -0.15, 0.5)] {
            let (mut foot, _) = step(t + offset);
            foot[0] = x;
            run(
                &mut r,
                "anim.key_set",
                json!({ "bone": format!("shin.{side}"), "channel": "ik_target", "time": t, "value": foot }),
            );
            let swing = 25.0 * ((t + offset + 0.5) * std::f32::consts::TAU).cos();
            run(
                &mut r,
                "anim.key_set",
                json!({ "bone": format!("upper_arm.{side}"), "channel": "rotation", "time": t, "value": [0, 0, swing] }),
            );
        }
        let bob = 0.03 * (t * 2.0 * std::f32::consts::TAU).cos().abs();
        run(
            &mut r,
            "anim.key_set",
            json!({ "bone": "root", "channel": "translation", "time": t, "value": [0, bob, 0] }),
        );
    }
    run(
        &mut r,
        "anim.playhead",
        json!({ "clip": "idle", "time": 0 }),
    );
    run(&mut r, "view.camera", json!({ "preset": "iso" }));
    run(&mut r, "view.frame", json!({}));
    run(&mut r, "project.save", json!({}));
}
