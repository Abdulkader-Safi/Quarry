//! Orbit camera with the FR-VIEW-02 presets and FR-VIEW-03 framing.
//! Right-handed, Y up, metres.

use glam::camera::rh::proj::directx as proj;
use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preset {
    Front,
    Side,
    Top,
    /// Classic isometric: a cube shows three equal faces.
    Iso,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    /// Radians around Y; 0 looks from +Z towards -Z.
    pub yaw: f32,
    /// Radians above the horizon.
    pub pitch: f32,
    pub ortho: bool,
    /// Vertical field of view in radians (perspective only).
    pub fov: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 8.0,
            yaw: 35f32.to_radians(),
            pitch: 25f32.to_radians(),
            ortho: false,
            fov: 50f32.to_radians(),
        }
    }
}

/// Pitch that makes the three faces of a cube equal: atan(1/√2) ≈ 35.26°.
fn iso_pitch() -> f32 {
    (1.0 / 2f32.sqrt()).atan()
}

impl OrbitCamera {
    pub fn preset(preset: Preset) -> Self {
        let (yaw, pitch, ortho) = match preset {
            Preset::Front => (0.0, 0.0, true),
            Preset::Side => (std::f32::consts::FRAC_PI_2, 0.0, true),
            Preset::Top => (0.0, std::f32::consts::FRAC_PI_2 - 1e-4, true),
            Preset::Iso => (std::f32::consts::FRAC_PI_4, iso_pitch(), true),
        };
        Self {
            yaw,
            pitch,
            ortho,
            ..Self::default()
        }
    }

    pub fn eye(&self) -> Vec3 {
        let dir = Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        );
        self.target + dir * self.distance
    }

    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Y)
    }

    /// Projection for a viewport of `aspect` = width / height, depth 0..1.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let far = self.distance * 20.0 + 100.0;
        if self.ortho {
            // Orthographic height matches what perspective shows at the target.
            let half_h = self.distance * (self.fov * 0.5).tan();
            let half_w = half_h * aspect;
            proj::orthographic(-half_w, half_w, -half_h, half_h, -far, far)
        } else {
            proj::perspective(self.fov, aspect, (self.distance * 0.01).max(0.01), far)
        }
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }

    /// Rotate around the target (radians).
    pub fn orbit(&mut self, d_yaw: f32, d_pitch: f32) {
        let limit = std::f32::consts::FRAC_PI_2 - 1e-3;
        self.yaw += d_yaw;
        self.pitch = (self.pitch + d_pitch).clamp(-limit, limit);
    }

    /// Slide the target in the view plane; `dx`, `dy` in metres.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let view = self.view();
        let right = view.row(0).truncate();
        let up = view.row(1).truncate();
        self.target += right * dx + up * dy;
    }

    /// Multiply the distance; below 1 zooms in.
    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(0.05, 10_000.0);
    }

    /// Centre `min..max` and back off until it fits the view.
    pub fn frame(&mut self, min: Vec3, max: Vec3) {
        self.target = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(0.25);
        self.distance = radius / (self.fov * 0.5).tan() * 1.1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_screen(m: Mat4, p: Vec3) -> glam::Vec2 {
        let clip = m * p.extend(1.0);
        glam::Vec2::new(clip.x, clip.y) / clip.w
    }

    /// FR-VIEW-02: the iso preset shows a cube's three visible faces at equal size.
    #[test]
    fn iso_cube_shows_three_equal_faces() {
        let cam = OrbitCamera::preset(Preset::Iso);
        let m = cam.view_proj(1.0);
        let area = |quad: [Vec3; 4]| {
            let s = quad.map(|p| to_screen(m, p));
            let mut a = 0.0;
            for i in 0..4 {
                let (p, q) = (s[i], s[(i + 1) % 4]);
                a += p.x * q.y - q.x * p.y;
            }
            (a * 0.5).abs()
        };
        let top = area([
            Vec3::new(0., 1., 0.),
            Vec3::new(1., 1., 0.),
            Vec3::new(1., 1., 1.),
            Vec3::new(0., 1., 1.),
        ]);
        let front = area([
            Vec3::new(0., 0., 1.),
            Vec3::new(1., 0., 1.),
            Vec3::new(1., 1., 1.),
            Vec3::new(0., 1., 1.),
        ]);
        let side = area([
            Vec3::new(1., 0., 0.),
            Vec3::new(1., 0., 1.),
            Vec3::new(1., 1., 1.),
            Vec3::new(1., 1., 0.),
        ]);
        assert!((top - front).abs() / top < 1e-3, "top {top} front {front}");
        assert!((top - side).abs() / top < 1e-3, "top {top} side {side}");
    }

    /// FR-VIEW-03: framing a far point puts it in the centre of the view.
    #[test]
    fn framing_centres_a_far_point() {
        let mut cam = OrbitCamera::default();
        let p = Vec3::new(40.0, -3.0, 25.0);
        cam.frame(p, p);
        let s = to_screen(cam.view_proj(16.0 / 10.0), p);
        assert!(s.length() < 1e-4, "{s}");
    }

    #[test]
    fn orbit_never_flips_over_the_pole() {
        let mut cam = OrbitCamera::default();
        cam.orbit(0.0, 10.0);
        assert!(cam.pitch < std::f32::consts::FRAC_PI_2);
        let before = cam.target;
        cam.pan(1.0, 0.0);
        assert!((cam.target - before).length() > 0.99);
    }
}
