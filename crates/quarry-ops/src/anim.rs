//! Animation curves (FR-ANIM-03, 04, 10): evaluating keys, sampling at the
//! project frame rate and removing keys a straight line already predicts.

use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    Step,
    Linear,
    In,
    Out,
    InOut,
}

impl Ease {
    /// Progress through a segment, 0..1 in, shaped 0..1 out.
    pub fn shape(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Step => 0.0,
            Ease::Linear => t,
            Ease::In => t * t,
            Ease::Out => 1.0 - (1.0 - t) * (1.0 - t),
            Ease::InOut => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// A value that can be keyed and blended.
pub trait Keyable: Copy {
    fn blend(a: Self, b: Self, t: f32) -> Self;
    /// How far apart two values are, for key reduction: metres or degrees.
    fn gap(a: Self, b: Self) -> f32;
}

impl Keyable for Vec3 {
    fn blend(a: Self, b: Self, t: f32) -> Self {
        a.lerp(b, t)
    }
    fn gap(a: Self, b: Self) -> f32 {
        a.distance(b)
    }
}

impl Keyable for Quat {
    fn blend(a: Self, b: Self, t: f32) -> Self {
        // Shortest way round.
        let b = if a.dot(b) < 0.0 { -b } else { b };
        a.slerp(b, t).normalize()
    }
    fn gap(a: Self, b: Self) -> f32 {
        a.angle_between(b).to_degrees()
    }
}

/// Value at `time` from keys sorted by time. Each key's ease shapes the
/// segment that starts at it. Before the first key and after the last, hold.
pub fn evaluate<T: Keyable>(keys: &[(f32, T, Ease)], time: f32) -> Option<T> {
    let first = keys.first()?;
    if time <= first.0 {
        return Some(first.1);
    }
    let last = keys.last()?;
    if time >= last.0 {
        return Some(last.1);
    }
    let i = keys.partition_point(|k| k.0 <= time) - 1;
    let (t0, v0, ease) = keys[i];
    let (t1, v1, _) = keys[i + 1];
    let span = (t1 - t0).max(1e-9);
    Some(T::blend(v0, v1, ease.shape((time - t0) / span)))
}

/// Sample times from 0 to `length` inclusive at `fps`.
pub fn sample_times(length: f32, fps: u32) -> Vec<f32> {
    let fps = fps.max(1) as f32;
    let frames = (length * fps).round().max(0.0) as usize;
    (0..=frames).map(|f| (f as f32 / fps).min(length)).collect()
}

/// Drop samples a straight line through their kept neighbours predicts to
/// within `tolerance` (in [`Keyable::gap`] units). Every original sample stays
/// within the tolerance of the reduced curve. A constant track keeps 2 keys.
pub fn reduce<T: Keyable>(samples: &[(f32, T)], tolerance: f32) -> Vec<(f32, T)> {
    if samples.len() <= 2 {
        return samples.to_vec();
    }
    let mut kept = vec![samples[0]];
    let mut start = 0;
    while start < samples.len() - 1 {
        // Reach as far as the line from `start` still covers every sample between.
        let mut end = start + 1;
        while end + 1 < samples.len() {
            let candidate = end + 1;
            let (t0, v0) = samples[start];
            let (t1, v1) = samples[candidate];
            let fits = (start + 1..candidate).all(|k| {
                let (t, v) = samples[k];
                T::gap(T::blend(v0, v1, (t - t0) / (t1 - t0).max(1e-9)), v) <= tolerance
            });
            if !fits {
                break;
            }
            end = candidate;
        }
        kept.push(samples[end]);
        start = end;
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_and_steps() {
        let keys = [
            (0.0, Vec3::ZERO, Ease::Linear),
            (1.0, Vec3::X, Ease::Linear),
        ];
        assert_eq!(evaluate(&keys, 0.25), Some(Vec3::new(0.25, 0.0, 0.0)));
        let step = [(0.0, Vec3::ZERO, Ease::Step), (1.0, Vec3::X, Ease::Linear)];
        assert_eq!(
            evaluate(&step, 0.99),
            Some(Vec3::ZERO),
            "a step key holds until the next"
        );
        assert_eq!(evaluate(&step, 1.0), Some(Vec3::X));
        assert!(Ease::In.shape(0.5) < 0.5 && Ease::Out.shape(0.5) > 0.5);
        assert_eq!(Ease::InOut.shape(0.5), 0.5);
        assert_eq!(evaluate::<Vec3>(&[], 0.0), None);
    }

    #[test]
    fn rotations_take_the_short_way() {
        let a = Quat::from_rotation_y(170f32.to_radians());
        let b = -Quat::from_rotation_y(-170f32.to_radians());
        let mid = Quat::blend(a, b, 0.5);
        assert!(
            (mid.angle_between(Quat::from_rotation_y(180f32.to_radians()))).to_degrees() < 0.01
        );
    }

    #[test]
    fn a_held_track_reduces_to_two_keys_and_curves_stay_in_tolerance() {
        let times = sample_times(1.0, 30);
        assert_eq!(times.len(), 31);
        let held: Vec<(f32, Vec3)> = times.iter().map(|t| (*t, Vec3::ONE)).collect();
        assert_eq!(reduce(&held, 1e-4).len(), 2);

        let keys = [
            (0.0, Quat::IDENTITY, Ease::InOut),
            (1.0, Quat::from_rotation_z(1.2), Ease::Linear),
        ];
        let curve: Vec<(f32, Quat)> = times
            .iter()
            .map(|t| (*t, evaluate(&keys, *t).unwrap()))
            .collect();
        let reduced = reduce(&curve, 0.1);
        assert!(reduced.len() < curve.len() && reduced.len() > 2);
        for (t, v) in &curve {
            let k = reduced
                .partition_point(|r| r.0 <= *t)
                .clamp(1, reduced.len() - 1);
            let (a, b) = (reduced[k - 1], reduced[k]);
            let back = Quat::blend(a.1, b.1, (t - a.0) / (b.0 - a.0));
            assert!(Quat::gap(back, *v) <= 0.1 + 1e-3, "error at {t}");
        }
    }
}
