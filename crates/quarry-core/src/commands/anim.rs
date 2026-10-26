//! Animation (FR-ANIM-01 to 05, 08). Clips belong to a skeleton; keys hold
//! each bone's pose relative to rest, or an IK chain's target and pole.
//! Headless playback is stepped, not real time.

use super::rig::{bone_index, skeleton_id};
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{
    BonePose, Channel, Clip, Document, Interp, Key, KeyValue, Skeleton, Track, TrackTarget,
};
use glam::{Quat, Vec3};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<AnimClipNew>();
    r.register::<AnimClipEdit>();
    r.register::<AnimClipDuplicate>();
    r.register::<AnimClipDelete>();
    r.register::<AnimKeySet>();
    r.register::<AnimKeyMove>();
    r.register::<AnimKeyDelete>();
    r.register::<AnimKeyCopy>();
    r.register::<AnimKeyPaste>();
    r.register::<AnimAutokey>();
    r.register::<AnimPlayhead>();
    r.register::<AnimPlay>();
    r.register::<AnimStop>();
    r.register::<AnimCrossfadePreview>();
    r.register::<QueryClip>();
}

/// Find a clip by ID or name (names are unique per skeleton; the first match wins).
fn clip_id(doc: &Document, reference: &str) -> Result<Id<Clip>, CommandError> {
    doc.clips.find(reference).map(|c| c.id).ok_or_else(|| {
        CommandError::not_found(format!(
            "No clip called '{reference}'. Call anim.clip_new first."
        ))
    })
}

fn active_clip(doc: &Document, reference: Option<&str>) -> Result<Id<Clip>, CommandError> {
    match reference {
        Some(r) => clip_id(doc, r),
        None => doc
            .view
            .anim
            .clip
            .filter(|c| doc.clips.get(*c).is_some())
            .ok_or_else(|| {
                CommandError::invalid("Name the clip, or pick one with anim.playhead first.")
            }),
    }
}

fn length_ok(length: f32) -> Result<f32, CommandError> {
    (length.is_finite() && length > 0.0 && length <= 3600.0)
        .then_some(length)
        .ok_or_else(|| {
            CommandError::new(
                ErrorCode::OutOfBounds,
                "length must be between 0 and 3600 seconds.",
            )
        })
}

fn unique_clip_name(
    doc: &Document,
    skeleton: Id<Skeleton>,
    name: &str,
    except: Option<Id<Clip>>,
) -> Result<(), CommandError> {
    if name.trim().is_empty() {
        return Err(CommandError::invalid("A clip name cannot be empty."));
    }
    if doc
        .clips
        .iter()
        .any(|c| c.skeleton == skeleton && c.name == name && Some(c.id) != except)
    {
        return Err(CommandError::new(
            ErrorCode::Conflict,
            format!("This skeleton already has a clip called '{name}'."),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimClipNew {
    /// Skeleton ID or name. Default the only skeleton.
    pub skeleton: Option<String>,
    /// Becomes the glTF animation name, such as idle or walk.
    pub name: String,
    /// Seconds. Default 1.
    pub length: Option<f32>,
    /// Default true.
    #[serde(rename = "loop")]
    pub looping: Option<bool>,
}

impl Command for AnimClipNew {
    const NAME: &'static str = "anim.clip_new";
    const DESCRIPTION: &'static str =
        "Make a named clip for a skeleton and make it the active clip.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let skeleton = match &self.skeleton {
            Some(s) => skeleton_id(doc, s)?,
            None if doc.skeletons.len() == 1 => doc.skeletons.ids().next().expect("one"),
            None => {
                return Err(CommandError::invalid(
                    "Name the skeleton (there is more than one, or none).",
                ));
            }
        };
        unique_clip_name(doc, skeleton, &self.name, None)?;
        let length = length_ok(self.length.unwrap_or(1.0))?;
        let id = doc.clips.insert(Clip {
            id: Id::new(0, 0),
            name: self.name.clone(),
            skeleton,
            length,
            looping: self.looping.unwrap_or(true),
            tracks: Vec::new(),
        });
        doc.view.anim.clip = Some(id);
        doc.view.anim.time = 0.0;
        Ok(json!({ "id": id, "name": self.name }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimClipEdit {
    pub clip: String,
    pub name: Option<String>,
    /// New length in seconds. Keys keep their times and values.
    pub length: Option<f32>,
    #[serde(rename = "loop")]
    pub looping: Option<bool>,
}

impl Command for AnimClipEdit {
    const NAME: &'static str = "anim.clip_edit";
    const DESCRIPTION: &'static str =
        "Rename a clip, change its length (keys keep their times) or turn looping on or off.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = clip_id(doc, &self.clip)?;
        let skeleton = doc.clips.get(id).expect("resolved").skeleton;
        if let Some(n) = &self.name {
            unique_clip_name(doc, skeleton, n, Some(id))?;
        }
        let length = self.length.map(length_ok).transpose()?;
        let c = doc.clips.get_mut(id).expect("resolved");
        if let Some(n) = self.name {
            c.name = n;
        }
        if let Some(l) = length {
            c.length = l;
        }
        if let Some(l) = self.looping {
            c.looping = l;
        }
        Ok(json!({ "name": c.name, "length": c.length, "loop": c.looping }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimClipDuplicate {
    pub clip: String,
    /// Default the clip's name with _copy.
    pub name: Option<String>,
}

impl Command for AnimClipDuplicate {
    const NAME: &'static str = "anim.clip_duplicate";
    const DESCRIPTION: &'static str = "Copy a clip with all its keys under a new name.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = clip_id(doc, &self.clip)?;
        let mut copy = doc.clips.get(id).expect("resolved").clone();
        copy.name = self.name.unwrap_or_else(|| format!("{}_copy", copy.name));
        unique_clip_name(doc, copy.skeleton, &copy.name, None)?;
        let name = copy.name.clone();
        let new = doc.clips.insert(copy);
        Ok(json!({ "id": new, "name": name }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimClipDelete {
    pub clip: String,
}

impl Command for AnimClipDelete {
    const NAME: &'static str = "anim.clip_delete";
    const DESCRIPTION: &'static str = "Delete a clip and its keys. Undo brings it back.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = clip_id(doc, &self.clip)?;
        doc.clips.remove(id);
        if doc.view.anim.clip == Some(id) {
            doc.view.anim.clip = None;
        }
        Ok(json!({ "deleted": self.clip }))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyChannel {
    Translation,
    Rotation,
    Scale,
    IkTarget,
    IkPole,
}

impl KeyChannel {
    fn channel(self) -> Channel {
        match self {
            KeyChannel::Translation => Channel::Translation,
            KeyChannel::Rotation => Channel::Rotation,
            KeyChannel::Scale => Channel::Scale,
            KeyChannel::IkTarget => Channel::IkTarget,
            KeyChannel::IkPole => Channel::IkPole,
        }
    }
    fn is_ik(self) -> bool {
        matches!(self, KeyChannel::IkTarget | KeyChannel::IkPole)
    }
}

/// Where a key goes: a bone channel or an IK chain (named by its tip bone).
fn track_target(
    doc: &Document,
    clip: Id<Clip>,
    bone: &str,
    channel: KeyChannel,
) -> Result<TrackTarget, CommandError> {
    let s = doc
        .skeletons
        .get(doc.clips.get(clip).expect("resolved").skeleton)
        .ok_or_else(|| CommandError::not_found("The clip's skeleton is gone."))?;
    let b = bone_index(s, bone)?;
    if channel.is_ik() {
        let chain = s
            .ik_chains
            .iter()
            .position(|c| c.tip_bone == b)
            .ok_or_else(|| {
                CommandError::not_found(format!(
                    "No IK chain ends at '{bone}'. Call rig.ik_add first."
                ))
            })?;
        Ok(TrackTarget::Ik(chain as u16))
    } else {
        Ok(TrackTarget::Bone(b))
    }
}

/// Insert or replace a key, keeping keys sorted by time.
pub(super) fn upsert(clip: &mut Clip, target: TrackTarget, channel: Channel, key: Key) {
    let index = match clip
        .tracks
        .iter()
        .position(|t| t.target == target && t.channel == channel)
    {
        Some(i) => i,
        None => {
            clip.tracks.push(Track {
                target,
                channel,
                keys: Vec::new(),
            });
            clip.tracks.len() - 1
        }
    };
    let keys = &mut clip.tracks[index].keys;
    match keys.iter().position(|k| (k.time - key.time).abs() < 1e-4) {
        Some(i) => keys[i] = key,
        None => {
            let at = keys.partition_point(|k| k.time < key.time);
            keys.insert(at, key);
        }
    }
}

/// Key the current pose of a bone (rotation, and translation if moved) into
/// the active clip at the playhead, when auto-key is on.
pub(super) fn autokey_bone(
    doc: &mut Document,
    skeleton: Id<Skeleton>,
    bone: u16,
    translated: bool,
) {
    let tool = doc.view.anim;
    let Some(clip) = tool
        .clip
        .filter(|c| doc.clips.get(*c).is_some_and(|c| c.skeleton == skeleton))
    else {
        return;
    };
    if !tool.autokey {
        return;
    }
    let pose = doc
        .skeletons
        .get(skeleton)
        .map(|s| s.pose_of(bone as usize))
        .unwrap_or_default();
    let c = doc.clips.get_mut(clip).expect("checked");
    upsert(
        c,
        TrackTarget::Bone(bone),
        Channel::Rotation,
        Key {
            time: tool.time,
            value: KeyValue::Quat(pose.rotation),
            interp: Interp::Linear,
            handles: None,
        },
    );
    if translated {
        upsert(
            c,
            TrackTarget::Bone(bone),
            Channel::Translation,
            Key {
                time: tool.time,
                value: KeyValue::Vec3(pose.translation),
                interp: Interp::Linear,
                handles: None,
            },
        );
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimKeySet {
    /// Clip ID or name. Default the active clip.
    pub clip: Option<String>,
    /// Bone name; for IK channels, the chain's tip bone.
    pub bone: String,
    pub channel: KeyChannel,
    /// Seconds. Default the playhead.
    pub time: Option<f32>,
    /// [x, y, z] for translation, scale and IK; Euler degrees [x, y, z] for
    /// rotation. Default the current pose.
    pub value: Option<[f32; 3]>,
    /// step, linear, ease_in, ease_out or ease_in_out. Default linear.
    pub interp: Option<Interp>,
}

impl Command for AnimKeySet {
    const NAME: &'static str = "anim.key_set";
    const DESCRIPTION: &'static str = "Set a key on a bone channel (translation, rotation, scale) or an IK chain's target or pole, at a time, with an interpolation. Without a value it keys the current pose.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let clip = active_clip(doc, self.clip.as_deref())?;
        let time = self.time.unwrap_or(doc.view.anim.time);
        if !(time.is_finite() && time >= 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "time must be 0 seconds or later.",
            ));
        }
        let target = track_target(doc, clip, &self.bone, self.channel)?;
        let s = doc
            .skeletons
            .get(doc.clips.get(clip).expect("resolved").skeleton)
            .expect("checked");
        let value = match (self.channel, self.value, target) {
            (KeyChannel::Rotation, Some([x, y, z]), _) => KeyValue::Quat(Quat::from_euler(
                glam::EulerRot::XYZ,
                x.to_radians(),
                y.to_radians(),
                z.to_radians(),
            )),
            (_, Some(v), _) => KeyValue::Vec3(Vec3::from(v)),
            (KeyChannel::Rotation, None, TrackTarget::Bone(b)) => {
                KeyValue::Quat(s.pose_of(b as usize).rotation)
            }
            (KeyChannel::Translation, None, TrackTarget::Bone(b)) => {
                KeyValue::Vec3(s.pose_of(b as usize).translation)
            }
            (KeyChannel::Scale, None, TrackTarget::Bone(b)) => {
                KeyValue::Vec3(s.pose_of(b as usize).scale)
            }
            (KeyChannel::IkTarget, None, TrackTarget::Ik(c)) => {
                KeyValue::Vec3(s.ik_chains[c as usize].target)
            }
            (KeyChannel::IkPole, None, TrackTarget::Ik(c)) => {
                KeyValue::Vec3(s.ik_chains[c as usize].pole)
            }
            _ => {
                return Err(CommandError::invalid(
                    "That channel does not fit that bone.",
                ));
            }
        };
        let finite = match value {
            KeyValue::Vec3(v) => v.is_finite(),
            KeyValue::Quat(q) => q.is_finite(),
        };
        if !finite {
            return Err(CommandError::invalid("value must be finite numbers."));
        }
        let c = doc.clips.get_mut(clip).expect("resolved");
        upsert(
            c,
            target,
            self.channel.channel(),
            Key {
                time,
                value,
                interp: self.interp.unwrap_or(Interp::Linear),
                handles: None,
            },
        );
        Ok(json!({ "clip": clip, "time": time }))
    }
}

fn track_index(c: &Clip, target: TrackTarget, channel: Channel) -> Result<usize, CommandError> {
    c.tracks
        .iter()
        .position(|t| t.target == target && t.channel == channel)
        .ok_or_else(|| CommandError::not_found("That channel has no keys in this clip."))
}

fn key_at(keys: &[Key], time: f32) -> Result<usize, CommandError> {
    keys.iter()
        .position(|k| (k.time - time).abs() < 1e-4)
        .ok_or_else(|| {
            CommandError::not_found(format!("No key at {time} s. Call query.clip to list keys."))
        })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimKeyMove {
    pub clip: Option<String>,
    pub bone: String,
    pub channel: KeyChannel,
    pub from: f32,
    pub to: f32,
}

impl Command for AnimKeyMove {
    const NAME: &'static str = "anim.key_move";
    const DESCRIPTION: &'static str =
        "Move a key to another time (replacing any key already there).";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let clip = active_clip(doc, self.clip.as_deref())?;
        if !(self.to.is_finite() && self.to >= 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "to must be 0 seconds or later.",
            ));
        }
        let target = track_target(doc, clip, &self.bone, self.channel)?;
        let c = doc.clips.get_mut(clip).expect("resolved");
        let t = track_index(c, target, self.channel.channel())?;
        let i = key_at(&c.tracks[t].keys, self.from)?;
        let mut key = c.tracks[t].keys.remove(i);
        key.time = self.to;
        upsert(c, target, self.channel.channel(), key);
        Ok(json!({ "time": self.to }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimKeyDelete {
    pub clip: Option<String>,
    pub bone: String,
    pub channel: KeyChannel,
    pub time: f32,
}

impl Command for AnimKeyDelete {
    const NAME: &'static str = "anim.key_delete";
    const DESCRIPTION: &'static str = "Delete one key. A channel with no keys left goes back to rest (or, for IK, the chain's own target).";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let clip = active_clip(doc, self.clip.as_deref())?;
        let target = track_target(doc, clip, &self.bone, self.channel)?;
        let c = doc.clips.get_mut(clip).expect("resolved");
        let t = track_index(c, target, self.channel.channel())?;
        let i = key_at(&c.tracks[t].keys, self.time)?;
        c.tracks[t].keys.remove(i);
        if c.tracks[t].keys.is_empty() {
            c.tracks.remove(t);
        }
        Ok(json!({ "deleted": self.time }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimKeyCopy {
    pub clip: Option<String>,
    /// Copy keys from this time ...
    pub start: f32,
    /// ... to this time, inclusive.
    pub end: f32,
}

impl Command for AnimKeyCopy {
    const NAME: &'static str = "anim.key_copy";
    const DESCRIPTION: &'static str =
        "List the keys of every channel between two times (what anim.key_paste copies).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let clip = active_clip(doc, self.clip.as_deref())?;
        let c = doc.clips.get(clip).expect("resolved");
        let tracks: Vec<_> = c
            .tracks
            .iter()
            .map(|t| json!({ "target": t.target, "channel": t.channel, "keys": t.keys.iter().filter(|k| k.time >= self.start - 1e-4 && k.time <= self.end + 1e-4).collect::<Vec<_>>() }))
            .collect();
        Ok(json!({ "clip": clip, "tracks": tracks }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimKeyPaste {
    /// Clip to paste into. Default the active clip.
    pub clip: Option<String>,
    /// Clip to copy from. Default the same clip.
    pub from_clip: Option<String>,
    pub start: f32,
    pub end: f32,
    /// Where the first copied time lands, in seconds.
    pub at: f32,
}

impl Command for AnimKeyPaste {
    const NAME: &'static str = "anim.key_paste";
    const DESCRIPTION: &'static str = "Copy the keys between start and end (of this or another clip of the same skeleton) to start at another time. Values stay, times shift.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let to = active_clip(doc, self.clip.as_deref())?;
        let from = match &self.from_clip {
            Some(f) => clip_id(doc, f)?,
            None => to,
        };
        let source = doc.clips.get(from).expect("resolved").clone();
        if source.skeleton != doc.clips.get(to).expect("resolved").skeleton {
            return Err(CommandError::invalid(
                "Both clips must belong to the same skeleton.",
            ));
        }
        if !(self.at.is_finite() && self.at >= 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "at must be 0 seconds or later.",
            ));
        }
        let shift = self.at - self.start;
        let c = doc.clips.get_mut(to).expect("resolved");
        let mut pasted = 0;
        for t in &source.tracks {
            for k in t
                .keys
                .iter()
                .filter(|k| k.time >= self.start - 1e-4 && k.time <= self.end + 1e-4)
            {
                let mut key = k.clone();
                key.time = (k.time + shift).max(0.0);
                upsert(c, t.target, t.channel, key);
                pasted += 1;
            }
        }
        Ok(json!({ "pasted": pasted }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimAutokey {
    pub on: bool,
}

impl Command for AnimAutokey {
    const NAME: &'static str = "anim.autokey";
    const DESCRIPTION: &'static str = "Turn auto-key on or off: when on, posing a bone or moving an IK target keys it in the active clip at the playhead.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        cx.doc().view.anim.autokey = self.on;
        Ok(json!({ "autokey": self.on }))
    }
}

/// Show `clip` at `time` on its skeleton (the pose only; IK chain targets stay).
fn show(doc: &mut Document, clip: Id<Clip>, time: f32) -> Result<Vec<BonePose>, CommandError> {
    let c = doc.clips.get(clip).expect("resolved").clone();
    let s = doc
        .skeletons
        .get(c.skeleton)
        .ok_or_else(|| CommandError::not_found("The clip's skeleton is gone."))?;
    let posed = c.pose_at(s, time);
    doc.skeletons.get_mut(c.skeleton).expect("checked").pose = posed.pose.clone();
    doc.view.anim.clip = Some(clip);
    doc.view.anim.time = time;
    Ok(posed.pose)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimPlayhead {
    /// Seconds.
    pub time: f32,
    /// Make this the active clip. Default the active clip.
    pub clip: Option<String>,
}

impl Command for AnimPlayhead {
    const NAME: &'static str = "anim.playhead";
    const DESCRIPTION: &'static str =
        "Scrub: move the playhead and show the clip's pose at that time.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        if !(self.time.is_finite() && self.time >= 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "time must be 0 seconds or later.",
            ));
        }
        let doc = cx.doc();
        let clip = active_clip(doc, self.clip.as_deref())?;
        show(doc, clip, self.time)?;
        Ok(json!({ "clip": clip, "time": self.time }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimPlay {
    pub clip: Option<String>,
    /// Frames to advance at the project frame rate. Default 1.
    pub frames: Option<u32>,
}

impl Command for AnimPlay {
    const NAME: &'static str = "anim.play";
    const DESCRIPTION: &'static str = "Advance playback by frames at the project frame rate, wrapping looping clips (the window plays in real time; headless steps).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let clip = active_clip(doc, self.clip.as_deref())?;
        let fps = doc.settings.fps.max(1) as f32;
        let c = doc.clips.get(clip).expect("resolved");
        let frames_in_clip = (c.length * fps).round().max(1.0);
        let start = if doc.view.anim.clip == Some(clip) {
            doc.view.anim.time
        } else {
            0.0
        };
        let frame = (start * fps).round() + self.frames.unwrap_or(1) as f32;
        // A loop of N frames shows frames 0..N-1: frame N is frame 0 again.
        let frame = if c.looping {
            frame.rem_euclid(frames_in_clip)
        } else {
            frame.min(frames_in_clip)
        };
        let time = frame / fps;
        show(doc, clip, time)?;
        Ok(json!({ "clip": clip, "time": time, "frame": frame as u32 }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimStop {
    pub clip: Option<String>,
}

impl Command for AnimStop {
    const NAME: &'static str = "anim.stop";
    const DESCRIPTION: &'static str =
        "Stop playback and go back to the start of the clip (not the rest pose).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let clip = active_clip(doc, self.clip.as_deref())?;
        show(doc, clip, 0.0)?;
        Ok(json!({ "clip": clip, "time": 0.0 }))
    }
}

/// Blend two poses bone by bone (Cairn's animator Fade: linear weight).
pub fn blend(a: &[BonePose], b: &[BonePose], w: f32) -> Vec<BonePose> {
    use quarry_ops::anim::Keyable;
    a.iter()
        .zip(b)
        .map(|(a, b)| BonePose {
            translation: a.translation.lerp(b.translation, w),
            rotation: Quat::blend(a.rotation, b.rotation, w),
            scale: a.scale.lerp(b.scale, w),
        })
        .collect()
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimCrossfadePreview {
    pub from: String,
    pub to: String,
    /// Fade length in seconds; 0 is a hard cut.
    pub fade: f32,
    /// Seconds since the fade started.
    pub time: f32,
}

impl Command for AnimCrossfadePreview {
    const NAME: &'static str = "anim.crossfade_preview";
    const DESCRIPTION: &'static str = "Preview fading from one clip into another (as Cairn's animator Fade does): the blended pose at a time after the fade starts. Changes nothing.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let (a, b) = (clip_id(doc, &self.from)?, clip_id(doc, &self.to)?);
        let (ca, cb) = (
            doc.clips.get(a).expect("resolved"),
            doc.clips.get(b).expect("resolved"),
        );
        if ca.skeleton != cb.skeleton {
            return Err(CommandError::invalid(
                "Both clips must belong to the same skeleton.",
            ));
        }
        if !(self.fade.is_finite() && self.fade >= 0.0 && self.time.is_finite() && self.time >= 0.0)
        {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "fade and time must be 0 or more seconds.",
            ));
        }
        let s = doc
            .skeletons
            .get(ca.skeleton)
            .expect("clips keep their skeleton");
        let local = |c: &Clip, t: f32| {
            if c.looping {
                t.rem_euclid(c.length.max(1e-6))
            } else {
                t.min(c.length)
            }
        };
        let pa = ca.pose_at(s, local(ca, self.time)).pose;
        let pb = cb.pose_at(s, local(cb, self.time)).pose;
        let weight = if self.fade == 0.0 {
            1.0
        } else {
            (self.time / self.fade).clamp(0.0, 1.0)
        };
        let pose = blend(&pa, &pb, weight);
        let bones: serde_json::Map<String, serde_json::Value> = s
            .bones
            .iter()
            .zip(&pose)
            .map(|(b, p)| (b.name.clone(), json!(p)))
            .collect();
        Ok(json!({ "weight": weight, "bones": bones }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryClip {
    pub clip: String,
}

impl Command for QueryClip {
    const NAME: &'static str = "query.clip";
    const DESCRIPTION: &'static str = "A clip's length, loop flag and its tracks with keys (bone or IK chain, channel, times, values, interpolation).";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let c = doc.clips.get(clip_id(doc, &self.clip)?).expect("resolved");
        let s = doc.skeletons.get(c.skeleton);
        let name = |t: &TrackTarget| match (t, s) {
            (TrackTarget::Bone(b), Some(s)) => s
                .bones
                .get(*b as usize)
                .map_or("?".into(), |b| b.name.clone()),
            (TrackTarget::Ik(i), Some(s)) => s
                .ik_chains
                .get(*i as usize)
                .and_then(|c| s.bones.get(c.tip_bone as usize))
                .map_or("?".into(), |b| format!("IK {}", b.name)),
            _ => "?".into(),
        };
        let tracks: Vec<_> = c
            .tracks
            .iter()
            .map(|t| json!({ "target": name(&t.target), "channel": t.channel, "keys": t.keys }))
            .collect();
        Ok(json!({ "name": c.name, "length": c.length, "loop": c.looping, "tracks": tracks }))
    }
}
