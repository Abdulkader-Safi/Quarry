//! Rigging maths (FR-RIG-02, 05, 06): bone frames, the one weight normaliser,
//! distance-based auto weights and analytic two-bone IK.
//!
//! A bone's frame (Blender's convention): origin at the head, +Y along
//! head → tail, rotated by `roll` about that axis.

use glam::{Mat4, Quat, Vec3};

/// World (skeleton-space) rest matrix of a bone.
pub fn rest_matrix(head: Vec3, tail: Vec3, roll: f32) -> Mat4 {
    let dir = (tail - head).normalize_or(Vec3::Y);
    let aim = Quat::from_rotation_arc(Vec3::Y, dir);
    Mat4::from_rotation_translation(aim * Quat::from_rotation_y(roll), head)
}

/// Keep the 4 largest influences, drop the rest, and scale them to sum to 1.
/// Every weight edit goes through here so the FR-RIG-05 rule cannot drift.
/// Returns (bones, weights) heaviest first; all-zero input gives all zeros.
pub fn normalise(influences: &[(u16, f32)]) -> ([u16; 4], [f32; 4]) {
    let mut merged: Vec<(u16, f32)> = Vec::new();
    for &(bone, w) in influences {
        if !(w.is_finite() && w > 0.0) {
            continue;
        }
        match merged.iter_mut().find(|(b, _)| *b == bone) {
            Some(entry) => entry.1 += w,
            None => merged.push((bone, w)),
        }
    }
    merged.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    merged.truncate(4);
    let total: f32 = merged.iter().map(|(_, w)| w).sum();
    let mut bones = [0u16; 4];
    let mut weights = [0f32; 4];
    for (k, (b, w)) in merged.iter().enumerate() {
        bones[k] = *b;
        weights[k] = if total > 0.0 { w / total } else { 0.0 };
    }
    // Put rounding error on the heaviest so the sum is 1 to the last bit we can.
    if total > 0.0 {
        let sum: f32 = weights.iter().sum();
        weights[0] += 1.0 - sum;
    }
    (bones, weights)
}

/// Distance from `p` to the segment a–b.
pub fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Auto weights by distance to each bone (FR-RIG-02). A steep falloff
/// (1 / d⁴) makes each vertex follow its nearest bone, blending only where
/// bones meet; weights under 2% are dropped before normalising.
pub fn auto_weights(p: Vec3, bones: &[(Vec3, Vec3)]) -> ([u16; 4], [f32; 4]) {
    let raw: Vec<(u16, f32)> = bones
        .iter()
        .enumerate()
        .map(|(i, (h, t))| {
            let d = segment_distance(p, *h, *t).max(1e-4);
            (i as u16, 1.0 / (d * d * d * d))
        })
        .collect();
    let total: f32 = raw.iter().map(|(_, w)| w).sum();
    let kept: Vec<(u16, f32)> = raw
        .into_iter()
        .filter(|(_, w)| *w / total >= 0.02)
        .collect();
    normalise(&kept)
}

/// Rotations that bend a two-bone chain so its tip reaches `target`, with the
/// middle joint bending towards `pole` (FR-RIG-06). Inputs are world positions
/// of the chain's root, middle joint and tip in the current pose. Returns the
/// world-space rotation to apply at the root, then at the middle joint (after
/// the first). Out-of-reach targets stop at full extension; nothing is NaN.
pub fn two_bone_ik(root: Vec3, mid: Vec3, tip: Vec3, target: Vec3, pole: Vec3) -> (Quat, Quat) {
    let (a, b) = (root.distance(mid), mid.distance(tip));
    if a < 1e-6 || b < 1e-6 {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    let to_target = target - root;
    let reach = to_target.length().clamp((a - b).abs() + 1e-4, a + b - 1e-4);
    let dir = to_target.normalize_or((tip - root).normalize_or(Vec3::Y));

    // Plane of the bend: contains the target direction and leans toward the pole.
    let pole_dir = (pole - root) - dir * (pole - root).dot(dir);
    let fallback = {
        let bent = (mid - root) - dir * (mid - root).dot(dir);
        if bent.length() > 1e-5 {
            bent
        } else {
            dir.any_orthonormal_vector()
        }
    };
    let bend = pole_dir.normalize_or(fallback.normalize_or(dir.any_orthonormal_vector()));

    // Law of cosines: the angle at the root between the target line and the upper bone.
    let cos_root = ((a * a + reach * reach - b * b) / (2.0 * a * reach)).clamp(-1.0, 1.0);
    let angle = cos_root.acos();
    let new_mid = root + (dir * angle.cos() + bend * angle.sin()) * a;
    let new_tip = root + dir * reach;

    let first = Quat::from_rotation_arc((mid - root).normalize(), (new_mid - root).normalize());
    let moved_tip = new_mid + first * (tip - mid);
    let second = Quat::from_rotation_arc(
        (moved_tip - new_mid).normalize(),
        (new_tip - new_mid).normalize(),
    );
    (first, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_influences_become_four_summing_to_one() {
        let (bones, weights) = normalise(&[(0, 0.3), (1, 0.25), (2, 0.2), (3, 0.15), (4, 0.1)]);
        assert_eq!(bones, [0, 1, 2, 3]);
        assert!((weights.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!(weights.windows(2).all(|w| w[0] >= w[1]), "heaviest first");
        assert_eq!(normalise(&[]).1, [0.0; 4]);
        assert_eq!(normalise(&[(7, 0.5), (7, 0.5)]).0[0], 7, "duplicates merge");
    }

    #[test]
    fn rest_frame_points_y_along_the_bone() {
        let m = rest_matrix(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 2.0), 0.0);
        assert!(m.transform_vector3(Vec3::Y).distance(Vec3::Z) < 1e-6);
        assert!(m.transform_point3(Vec3::ZERO).distance(Vec3::X) < 1e-6);
    }

    #[test]
    fn auto_weights_leave_the_shoulder_on_the_upper_arm() {
        let arm = [(Vec3::ZERO, Vec3::X), (Vec3::X, Vec3::new(2.0, 0.0, 0.0))];
        let (bones, weights) = auto_weights(Vec3::new(0.2, 0.1, 0.0), &arm);
        assert_eq!(
            (bones[0], weights[0]),
            (0, 1.0),
            "shoulder: only the upper bone"
        );
        let (_, w) = auto_weights(Vec3::new(1.0, 0.1, 0.0), &arm);
        assert!(w[0] < 0.6 && w[1] > 0.4, "the elbow blends");
    }

    #[test]
    fn ik_reaches_the_target_with_the_knee_toward_the_pole() {
        let (root, mid, tip) = (
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
        );
        let target = Vec3::new(0.0, 0.6, 0.6);
        let pole = Vec3::new(0.0, 1.0, 5.0);
        let (q1, q2) = two_bone_ik(root, mid, tip, target, pole);
        let new_mid = root + q1 * (mid - root);
        let new_tip = new_mid + q2 * (q1 * (tip - mid));
        assert!(new_tip.distance(target) < 1e-4, "tip at {new_tip}");
        assert!(new_mid.z > 0.0, "knee bends toward the pole");
        assert!(
            (new_mid.distance(root) - 1.0).abs() < 1e-5
                && (new_tip.distance(new_mid) - 1.0).abs() < 1e-5,
            "bones keep their length"
        );
    }

    #[test]
    fn unreachable_targets_stop_at_full_extension() {
        let (root, mid, tip) = (
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 1.0),
        );
        let target = Vec3::new(10.0, 2.0, 0.0);
        let (q1, q2) = two_bone_ik(root, mid, tip, target, Vec3::new(0.0, 0.0, 5.0));
        let new_mid = root + q1 * (mid - root);
        let new_tip = new_mid + q2 * (q1 * (tip - mid));
        assert!(new_tip.is_finite() && new_mid.is_finite());
        assert!(
            (new_tip.distance(root) - 2.0).abs() < 1e-3,
            "straight, not stretched"
        );
        assert!(new_tip.x > 1.99);
    }
}
