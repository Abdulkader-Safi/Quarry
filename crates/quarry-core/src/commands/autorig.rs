//! Auto-rig helpers (FR-MCP-10): a named skeleton fitted to a mesh's bounds,
//! bound with automatic weights, optionally with a walk-cycle starter clip.
//! Every step is an ordinary command (`rig.skeleton_new`, `rig.bone_add`,
//! `rig.ik_add`, `rig.bind_auto`, `anim.clip_new`, `anim.key_set`), so a
//! person can do the same by hand, and the whole helper is one undo step.

use super::anim::{AnimClipNew, AnimKeySet};
use super::object;
use super::rig::{RigBindAuto, RigBoneAdd, RigIkAdd, RigSkeletonNew};
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context};
use glam::Vec3;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<RigBipedAuto>();
    r.register::<RigQuadrupedAuto>();
}

/// Run another command from JSON args, as a caller would.
fn call<C: Command + DeserializeOwned>(cx: &mut Context, args: Value) -> CommandResult {
    let command: C = serde_json::from_value(args)
        .map_err(|e| CommandError::invalid(format!("{}: {e}", C::NAME)))?;
    command.run(cx)
}

/// (name, parent, head, tail)
type Plan = Vec<(String, Option<String>, Vec3, Vec3)>;

/// The mesh object's world bounds.
fn bounds(cx: &mut Context, reference: &str) -> Result<(Vec3, Vec3), CommandError> {
    let doc = &cx.state.doc;
    let id = object(doc, reference)?;
    let o = doc.objects.get(id).expect("resolved");
    if doc.drawn_mesh(o).is_none() {
        return Err(CommandError::invalid(format!(
            "'{}' has no mesh to rig.",
            o.name
        )));
    }
    let (lo, hi) = doc
        .bounds([o])
        .ok_or_else(|| CommandError::invalid("That mesh is empty."))?;
    if hi.y - lo.y <= 1e-4 {
        return Err(CommandError::invalid(
            "That mesh is flat; rig a mesh with some height.",
        ));
    }
    Ok((lo, hi))
}

/// Build the planned bones, add IK on `ik` tips, bind the mesh.
fn build(
    cx: &mut Context,
    reference: &str,
    name: &str,
    plan: &Plan,
    ik: &[(&str, Vec3)],
) -> Result<String, CommandError> {
    let made = call::<RigSkeletonNew>(cx, json!({ "name": name }))?;
    let skeleton = made["name"].as_str().unwrap_or(name).to_string();
    for (bone, parent, head, tail) in plan {
        let mut args = json!({
            "skeleton": skeleton, "name": bone,
            "head": head.to_array(), "tail": tail.to_array(),
        });
        if let Some(p) = parent {
            args["parent"] = json!(p);
        }
        call::<RigBoneAdd>(cx, args)?;
    }
    for (tip, pole) in ik {
        call::<RigIkAdd>(
            cx,
            json!({ "skeleton": skeleton, "tip_bone": tip, "pole": pole.to_array() }),
        )?;
    }
    call::<RigBindAuto>(cx, json!({ "object": reference, "skeleton": skeleton }))?;
    Ok(skeleton)
}

/// A looping walk: feet (IK targets) step forward and back, lifting at the
/// passing position; `swing` bones rock about X against each other.
fn walk(
    cx: &mut Context,
    skeleton: &str,
    feet: &[(&str, Vec3, f32)],
    stride: f32,
    lift: f32,
    forward: Vec3,
    swing: &[(&str, f32)],
) -> CommandResult {
    call::<AnimClipNew>(
        cx,
        json!({ "skeleton": skeleton, "name": "walk", "length": 1.0 }),
    )?;
    for step in 0..=4 {
        let t = step as f32 / 4.0;
        for (tip, rest, phase) in feet {
            let a = (t + phase) * std::f32::consts::TAU;
            let foot = *rest + forward * (stride * a.cos()) + Vec3::Y * (lift * a.sin().max(0.0));
            call::<AnimKeySet>(
                cx,
                json!({ "bone": tip, "channel": "ik_target", "time": t, "value": foot.to_array(), "interp": "ease_in_out" }),
            )?;
        }
        for (bone, phase) in swing {
            let deg = 20.0 * ((t + phase) * std::f32::consts::TAU).cos();
            call::<AnimKeySet>(
                cx,
                json!({ "bone": bone, "channel": "rotation", "time": t, "value": [deg, 0, 0], "interp": "ease_in_out" }),
            )?;
        }
    }
    Ok(json!({ "clip": "walk" }))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigBipedAuto {
    /// The humanoid mesh object (ID or name), standing on its feet, facing +Z.
    pub object: String,
    /// Skeleton name. Default "<object> rig".
    pub name: Option<String>,
    /// Also make a looping "walk" starter clip. Default false.
    pub walk: Option<bool>,
}

impl Command for RigBipedAuto {
    const NAME: &'static str = "rig.biped_auto";
    const DESCRIPTION: &'static str = "Rig a humanoid mesh in one go: a 15-bone skeleton fitted to its bounds (hips, spine, chest, neck, head; upper_arm, lower_arm, hand, thigh, shin as .L/.R), leg IK, automatic weights, and optionally a walk starter clip. Arms are placed out in a T when the mesh is wide, else down the sides. Built from ordinary rig commands; one undo step.";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        let (lo, hi) = bounds(cx, &self.object)?;
        let h = hi.y - lo.y;
        let c = (lo + hi) * 0.5;
        let at = |side: f32, f: f32, x: f32| Vec3::new(c.x + side * x, lo.y + f * h, c.z);
        let mut plan: Plan = vec![
            ("hips".into(), None, at(0.0, 0.50, 0.0), at(0.0, 0.58, 0.0)),
            (
                "spine".into(),
                Some("hips".into()),
                at(0.0, 0.58, 0.0),
                at(0.0, 0.68, 0.0),
            ),
            (
                "chest".into(),
                Some("spine".into()),
                at(0.0, 0.68, 0.0),
                at(0.0, 0.80, 0.0),
            ),
            (
                "neck".into(),
                Some("chest".into()),
                at(0.0, 0.80, 0.0),
                at(0.0, 0.86, 0.0),
            ),
            (
                "head".into(),
                Some("neck".into()),
                at(0.0, 0.86, 0.0),
                at(0.0, 1.0, 0.0),
            ),
        ];
        let reach = (hi.x - lo.x) * 0.5;
        let t_pose = reach > 0.28 * h;
        let shoulder = 0.09 * h;
        for (side, s) in [(1.0, "L"), (-1.0, "R")] {
            // Arms: out to the mesh's reach in a T, or hanging at its sides.
            let (elbow, wrist, tip) = if t_pose {
                let along = |f: f32| at(side, 0.80, shoulder + (reach - shoulder) * f);
                (along(0.45), along(0.85), along(0.98))
            } else {
                let x = (reach * 0.85).max(shoulder);
                (at(side, 0.62, x), at(side, 0.47, x), at(side, 0.40, x))
            };
            let arm = [
                (
                    format!("upper_arm.{s}"),
                    "chest".to_string(),
                    at(side, 0.80, shoulder),
                    elbow,
                ),
                (
                    format!("lower_arm.{s}"),
                    format!("upper_arm.{s}"),
                    elbow,
                    wrist,
                ),
                (format!("hand.{s}"), format!("lower_arm.{s}"), wrist, tip),
            ];
            plan.extend(arm.map(|(n, p, a, b)| (n, Some(p), a, b)));
            let hip = 0.08 * h;
            // A little forward at the knee, so the leg knows which way to bend.
            let knee = at(side, 0.27, hip) + Vec3::Z * (0.02 * h);
            plan.push((
                format!("thigh.{s}"),
                Some("hips".into()),
                at(side, 0.50, hip),
                knee,
            ));
            plan.push((
                format!("shin.{s}"),
                Some(format!("thigh.{s}")),
                knee,
                at(side, 0.03, hip),
            ));
        }
        let name = self.name.unwrap_or_else(|| format!("{} rig", self.object));
        let ik: Vec<(&str, Vec3)> = vec![
            ("shin.L", at(1.0, 0.27, 0.08) + Vec3::Z * h),
            ("shin.R", at(-1.0, 0.27, 0.08) + Vec3::Z * h),
        ];
        let skeleton = build(cx, &self.object, &name, &plan, &ik)?;
        let mut reply = json!({ "skeleton": skeleton, "bones": plan.len(), "arms": if t_pose { "t" } else { "down" } });
        if self.walk == Some(true) {
            let foot = |side: f32| at(side, 0.03, 0.08 * h);
            walk(
                cx,
                &skeleton,
                &[("shin.L", foot(1.0), 0.0), ("shin.R", foot(-1.0), 0.5)],
                0.12 * h,
                0.06 * h,
                Vec3::Z,
                &[("upper_arm.L", 0.5), ("upper_arm.R", 0.0)],
            )?;
            reply["clip"] = json!("walk");
        }
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RigQuadrupedAuto {
    /// The four-legged mesh object (ID or name), standing, head toward the
    /// + end of its longer horizontal axis.
    pub object: String,
    /// Skeleton name. Default "<object> rig".
    pub name: Option<String>,
    /// Also make a looping "walk" starter clip. Default false.
    pub walk: Option<bool>,
}

impl Command for RigQuadrupedAuto {
    const NAME: &'static str = "rig.quadruped_auto";
    const DESCRIPTION: &'static str = "Rig a four-legged mesh in one go: 14 bones fitted to its bounds (hips, spine, chest, neck, head, tail; front_upper, front_lower, back_upper, back_lower as .L/.R), leg IK, automatic weights, and optionally a walk starter clip. Built from ordinary rig commands; one undo step.";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        let (lo, hi) = bounds(cx, &self.object)?;
        let size = hi - lo;
        let c = (lo + hi) * 0.5;
        let h = size.y;
        // The body runs along the longer horizontal axis; legs splay along the other.
        let (along, across, len, wide) = if size.z >= size.x {
            (Vec3::Z, Vec3::X, size.z, size.x)
        } else {
            (Vec3::X, Vec3::Z, size.x, size.z)
        };
        let at = |f: f32, y: f32, side: f32| {
            Vec3::new(c.x, lo.y + y * h, c.z)
                + along * (f * len * 0.5)
                + across * (side * wide * 0.3)
        };
        let mut plan: Plan = vec![
            (
                "hips".into(),
                None,
                at(-0.55, 0.65, 0.0),
                at(-0.2, 0.67, 0.0),
            ),
            (
                "spine".into(),
                Some("hips".into()),
                at(-0.2, 0.67, 0.0),
                at(0.2, 0.68, 0.0),
            ),
            (
                "chest".into(),
                Some("spine".into()),
                at(0.2, 0.68, 0.0),
                at(0.55, 0.7, 0.0),
            ),
            (
                "neck".into(),
                Some("chest".into()),
                at(0.55, 0.7, 0.0),
                at(0.72, 0.85, 0.0),
            ),
            (
                "head".into(),
                Some("neck".into()),
                at(0.72, 0.85, 0.0),
                at(0.98, 0.9, 0.0),
            ),
            (
                "tail".into(),
                Some("hips".into()),
                at(-0.55, 0.65, 0.0),
                at(-0.98, 0.6, 0.0),
            ),
        ];
        let mut ik = Vec::new();
        let mut feet = Vec::new();
        for (end, f, parent, phase) in [("front", 0.5, "chest", 0.0), ("back", -0.5, "hips", 0.5)] {
            for (side, s) in [(1.0, "L"), (-1.0, "R")] {
                let (upper, lower) = (format!("{end}_upper.{s}"), format!("{end}_lower.{s}"));
                // Knees bend back on the front legs and forward on the back, like most animals.
                let bend = if end == "front" { -0.05 } else { 0.05 };
                let knee = at(f + bend, 0.32, side);
                plan.push((upper.clone(), Some(parent.into()), at(f, 0.62, side), knee));
                plan.push((lower.clone(), Some(upper), knee, at(f, 0.02, side)));
                ik.push((lower.clone(), knee + along * (bend.signum() * len)));
                // Diagonal pairs move together.
                let p = if side > 0.0 {
                    phase
                } else {
                    (phase + 0.5) % 1.0
                };
                feet.push((lower, at(f, 0.02, side), p));
            }
        }
        let name = self.name.unwrap_or_else(|| format!("{} rig", self.object));
        let ik_refs: Vec<(&str, Vec3)> = ik.iter().map(|(n, p)| (n.as_str(), *p)).collect();
        let skeleton = build(cx, &self.object, &name, &plan, &ik_refs)?;
        let mut reply = json!({ "skeleton": skeleton, "bones": plan.len() });
        if self.walk == Some(true) {
            let feet: Vec<(&str, Vec3, f32)> =
                feet.iter().map(|(n, p, f)| (n.as_str(), *p, *f)).collect();
            walk(
                cx,
                &skeleton,
                &feet,
                0.08 * len,
                0.06 * h,
                along,
                &[("tail", 0.0)],
            )?;
            reply["clip"] = json!("walk");
        }
        Ok(reply)
    }
}
