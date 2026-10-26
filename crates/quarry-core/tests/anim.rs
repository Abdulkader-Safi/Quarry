//! M7 animation through the registry.

use glam::{Quat, Vec3};
use quarry_core::model::{Channel, KeyValue};
use quarry_core::{CommandRegistry, Source};
use serde_json::{Value, json};

fn run(r: &mut CommandRegistry, name: &str, args: Value) -> Value {
    r.execute(name, args, Source::Human)
        .unwrap_or_else(|e| panic!("{name}: {}", e.message))
}

/// A two-bone arm with a "walk" clip active.
fn arm() -> CommandRegistry {
    let mut r = CommandRegistry::new();
    run(&mut r, "rig.skeleton_new", json!({ "name": "rig" }));
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "upper", "head": [0, 0, 0], "tail": [1, 0, 0] }),
    );
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "lower", "parent": "upper", "tail": [2, 0, 0] }),
    );
    run(&mut r, "anim.clip_new", json!({ "name": "idle" }));
    run(
        &mut r,
        "anim.clip_new",
        json!({ "name": "walk", "length": 1.0 }),
    );
    r
}

fn rotation_of(r: &CommandRegistry, bone: usize) -> Quat {
    r.document()
        .skeletons
        .iter()
        .next()
        .unwrap()
        .pose_of(bone)
        .rotation
}

#[test]
fn clips_duplicate_and_change_length_without_touching_keys() {
    let mut r = arm();
    run(
        &mut r,
        "anim.key_set",
        json!({ "clip": "walk", "bone": "lower", "channel": "rotation", "time": 0.5, "value": [0, 0, 30] }),
    );
    run(
        &mut r,
        "anim.clip_duplicate",
        json!({ "clip": "walk", "name": "walk_alt" }),
    );
    let doc = r.document();
    let (walk, alt) = (
        doc.clips.by_name("walk").unwrap(),
        doc.clips.by_name("walk_alt").unwrap(),
    );
    assert_eq!(walk.tracks, alt.tracks, "keys and timings copied");
    run(
        &mut r,
        "anim.clip_edit",
        json!({ "clip": "walk", "length": 2.0 }),
    );
    let walk = r.document().clips.by_name("walk").unwrap();
    assert_eq!(walk.length, 2.0);
    assert_eq!(walk.tracks[0].keys[0].time, 0.5, "keys keep their times");
    let err = r
        .execute("anim.clip_new", json!({ "name": "walk" }), Source::Agent)
        .unwrap_err();
    assert_eq!(err.code, quarry_core::ErrorCode::Conflict);
}

#[test]
fn autokey_keys_a_rotation_at_the_playhead() {
    let mut r = arm();
    run(&mut r, "anim.autokey", json!({ "on": true }));
    run(&mut r, "anim.playhead", json!({ "time": 0.5 }));
    run(
        &mut r,
        "rig.pose",
        json!({ "bone": "lower", "rotation": [0, 0, 40] }),
    );
    let walk = r.document().clips.by_name("walk").unwrap();
    let track = walk
        .tracks
        .iter()
        .find(|t| t.channel == Channel::Rotation)
        .expect("rotation track");
    assert_eq!(track.keys.len(), 1);
    assert_eq!(track.keys[0].time, 0.5);
}

#[test]
fn copy_and_paste_shift_times_and_keep_values() {
    let mut r = arm();
    for (t, z) in [(0.0, 0), (0.5, 30), (1.0, 0)] {
        run(
            &mut r,
            "anim.key_set",
            json!({ "bone": "lower", "channel": "rotation", "time": t, "value": [0, 0, z] }),
        );
    }
    run(
        &mut r,
        "anim.clip_edit",
        json!({ "clip": "walk", "length": 3.0 }),
    );
    run(
        &mut r,
        "anim.key_paste",
        json!({ "start": 0.0, "end": 1.0, "at": 2.0 }),
    );
    let keys = &r.document().clips.by_name("walk").unwrap().tracks[0].keys;
    let times: Vec<f32> = keys.iter().map(|k| k.time).collect();
    assert_eq!(times, [0.0, 0.5, 1.0, 2.0, 2.5, 3.0]);
    assert_eq!(keys[1].value, keys[4].value);
}

#[test]
fn a_step_key_jumps_instead_of_blending() {
    let mut r = arm();
    run(
        &mut r,
        "anim.key_set",
        json!({ "bone": "lower", "channel": "rotation", "time": 0.0, "value": [0, 0, 0], "interp": "step" }),
    );
    run(
        &mut r,
        "anim.key_set",
        json!({ "bone": "lower", "channel": "rotation", "time": 0.5, "value": [0, 0, 90] }),
    );
    run(&mut r, "anim.playhead", json!({ "time": 0.49 }));
    assert!(
        rotation_of(&r, 1).angle_between(Quat::IDENTITY) < 1e-6,
        "held until the next key"
    );
    run(&mut r, "anim.playhead", json!({ "time": 0.5 }));
    assert!(
        (rotation_of(&r, 1)
            .angle_between(Quat::IDENTITY)
            .to_degrees()
            - 90.0)
            .abs()
            < 1e-3
    );
}

#[test]
fn a_one_second_loop_wraps_without_repeating_a_frame_and_scrub_matches() {
    let mut r = arm();
    run(
        &mut r,
        "anim.key_set",
        json!({ "bone": "lower", "channel": "rotation", "time": 0.0, "value": [0, 0, 0], "interp": "ease_in_out" }),
    );
    run(
        &mut r,
        "anim.key_set",
        json!({ "bone": "lower", "channel": "rotation", "time": 1.0, "value": [0, 0, 60] }),
    );
    run(&mut r, "anim.stop", json!({}));
    let mut frames = Vec::new();
    for _ in 0..300 {
        frames.push(
            run(&mut r, "anim.play", json!({}))["frame"]
                .as_u64()
                .unwrap(),
        );
    }
    assert!(
        frames.iter().all(|f| *f < 30),
        "frame 30 (= frame 0) is never shown"
    );
    assert_eq!(frames.iter().filter(|f| **f == 0).count(), 10, "ten loops");
    // Scrubbing to a frame shows the pose playback showed there.
    run(&mut r, "anim.stop", json!({}));
    for _ in 0..7 {
        run(&mut r, "anim.play", json!({}));
    }
    let played = rotation_of(&r, 1);
    run(&mut r, "anim.playhead", json!({ "time": 7.0 / 30.0 }));
    assert!(played.angle_between(rotation_of(&r, 1)) < 1e-5);
    run(&mut r, "anim.stop", json!({}));
    assert_eq!(
        r.document().view.anim.time,
        0.0,
        "stop returns to the clip start"
    );
}

#[test]
fn crossfade_blends_over_the_fade_and_changes_nothing() {
    let mut r = arm();
    run(
        &mut r,
        "anim.key_set",
        json!({ "clip": "walk", "bone": "lower", "channel": "rotation", "time": 0, "value": [0, 0, 80] }),
    );
    let before = (r.document().clone(), r.history().entries().len());
    let half = run(
        &mut r,
        "anim.crossfade_preview",
        json!({ "from": "idle", "to": "walk", "fade": 0.25, "time": 0.125 }),
    );
    assert_eq!(half["weight"], 0.5);
    let q: Quat = serde_json::from_value(half["bones"]["lower"]["rotation"].clone()).unwrap();
    assert!(
        (q.angle_between(Quat::IDENTITY).to_degrees() - 40.0).abs() < 1e-3,
        "halfway through the fade"
    );
    let cut = run(
        &mut r,
        "anim.crossfade_preview",
        json!({ "from": "idle", "to": "walk", "fade": 0, "time": 0 }),
    );
    assert_eq!(cut["weight"], 1.0, "fade 0 is a hard cut");
    assert_eq!((r.document().clone(), r.history().entries().len()), before);
}

#[test]
fn keyed_ik_targets_place_the_foot() {
    let mut r = CommandRegistry::new();
    run(&mut r, "rig.skeleton_new", json!({}));
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "thigh", "head": [0, 2, 0], "tail": [0, 1, 0] }),
    );
    run(
        &mut r,
        "rig.bone_add",
        json!({ "name": "shin", "parent": "thigh", "tail": [0, 0, 0] }),
    );
    run(
        &mut r,
        "rig.ik_add",
        json!({ "tip_bone": "shin", "pole": [0, 1, 5] }),
    );
    run(&mut r, "anim.clip_new", json!({ "name": "step" }));
    let targets = [
        (0.0, [0.0, 0.0, 0.0]),
        (0.5, [0.0, 0.5, 0.6]),
        (1.0, [0.0, 0.0, 0.0]),
    ];
    for (t, v) in targets {
        run(
            &mut r,
            "anim.key_set",
            json!({ "bone": "shin", "channel": "ik_target", "time": t, "value": v }),
        );
    }
    let doc = r.document();
    let s = doc.skeletons.iter().next().unwrap();
    let clip = doc.clips.by_name("step").unwrap();
    assert!(clip.tracks.iter().all(|t| t.channel == Channel::IkTarget));
    for (t, v) in targets {
        let posed = clip.pose_at(s, t);
        let foot = posed.pose_world()[1].transform_point3(Vec3::Y);
        assert!(
            foot.distance(Vec3::from(v)) < 1e-3,
            "foot at {foot} for {t}"
        );
    }
    assert!(matches!(clip.tracks[0].keys[1].value, KeyValue::Vec3(_)));
}
