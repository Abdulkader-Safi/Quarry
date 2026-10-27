//! The transform gizmo: X, Y and Z handles on the selection's pivot. Dragging
//! a handle moves, rotates or scales along that axis. While dragging, the
//! change is applied live and the previous step undone, so the whole drag
//! lands as one undo entry.

use crate::viewport::Viewport;
use glam::Vec3;
use gpui_kit::*;

/// Handle length on screen, in logical pixels.
const LENGTH: f32 = 80.0;
/// How close the pointer must be to a handle to grab it.
const GRAB: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Move,
    Rotate,
    Scale,
}

/// One axis handle in window coordinates.
#[derive(Clone, Copy)]
pub struct Handle {
    pub axis: usize,
    pub from: Point<Pixels>,
    pub to: Point<Pixels>,
    /// Screen pixels per metre along this axis at the pivot.
    pub px_per_metre: f32,
}

pub struct Drag {
    pub kind: Kind,
    pub axis: usize,
    pub start: Point<Pixels>,
    /// Unit screen direction of the handle.
    pub dir: (f32, f32),
    pub px_per_metre: f32,
    /// The amount applied so far (metres, degrees or a factor).
    pub applied: Option<f32>,
}

pub fn handles(view: &Viewport, pivot: Vec3) -> Option<[Handle; 3]> {
    let from = view.project(pivot)?;
    let mut out = [Handle {
        axis: 0,
        from,
        to: from,
        px_per_metre: 0.0,
    }; 3];
    for (axis, slot) in out.iter_mut().enumerate() {
        let unit = [Vec3::X, Vec3::Y, Vec3::Z][axis];
        let tip = view.project(pivot + unit)?;
        let (dx, dy) = (f32::from(tip.x - from.x), f32::from(tip.y - from.y));
        let len = (dx * dx + dy * dy).sqrt();
        // An axis pointing at the camera still gets a stub to grab.
        let shown = if len < 1e-3 {
            (0.0, 0.0)
        } else {
            (dx / len, dy / len)
        };
        *slot = Handle {
            axis,
            from,
            to: point(from.x + px(shown.0 * LENGTH), from.y + px(shown.1 * LENGTH)),
            px_per_metre: len.max(1e-3),
        };
    }
    Some(out)
}

/// The handle under the pointer, if any.
pub fn hit(handles: &[Handle; 3], at: Point<Pixels>) -> Option<Handle> {
    handles
        .iter()
        .map(|h| (distance_to_segment(at, h.from, h.to), *h))
        .filter(|(d, h)| {
            *d <= GRAB
                && f32::from(h.to.x - h.from.x).abs() + f32::from(h.to.y - h.from.y).abs() > 4.0
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, h)| h)
}

impl Drag {
    pub fn new(kind: Kind, h: Handle, at: Point<Pixels>) -> Self {
        let (dx, dy) = (f32::from(h.to.x - h.from.x), f32::from(h.to.y - h.from.y));
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        Self {
            kind,
            axis: h.axis,
            start: at,
            dir: (dx / len, dy / len),
            px_per_metre: h.px_per_metre,
            applied: None,
        }
    }

    /// The amount for the pointer at `at`: metres for Move, degrees for
    /// Rotate, a factor for Scale. Snapped unless `free`.
    pub fn amount(&self, at: Point<Pixels>, grid: f32, free: bool) -> f32 {
        let along = f32::from(at.x - self.start.x) * self.dir.0
            + f32::from(at.y - self.start.y) * self.dir.1;
        let snap = |v: f32, step: f32| {
            if free || step <= 0.0 {
                v
            } else {
                (v / step).round() * step
            }
        };
        match self.kind {
            Kind::Move => snap(along / self.px_per_metre, grid),
            Kind::Rotate => snap(along * 0.75, 15.0),
            Kind::Scale => snap(1.0 + along / LENGTH, 0.1).max(0.05),
        }
    }

    /// `mesh.transform` arguments for an amount.
    pub fn args(&self, amount: f32) -> serde_json::Value {
        let mut v = [0.0f32; 3];
        match self.kind {
            Kind::Move => {
                v[self.axis] = amount;
                serde_json::json!({ "translate": v })
            }
            Kind::Rotate => {
                v[self.axis] = amount;
                serde_json::json!({ "rotate": v })
            }
            Kind::Scale => {
                let mut s = [1.0f32; 3];
                s[self.axis] = amount;
                serde_json::json!({ "scale": s })
            }
        }
    }

    /// What the drag reads as, for the label next to the pivot.
    pub fn label(&self, amount: f32) -> String {
        let axis = ["X", "Y", "Z"][self.axis];
        match self.kind {
            Kind::Move => format!("{axis} {amount:+.2} m"),
            Kind::Rotate => format!("{axis} {amount:+.0}°"),
            Kind::Scale => format!("{axis} ×{amount:.2}"),
        }
    }

    /// Nothing to apply: the identity for this kind.
    pub fn is_identity(&self, amount: f32) -> bool {
        match self.kind {
            Kind::Scale => (amount - 1.0).abs() < 1e-4,
            _ => amount.abs() < 1e-4,
        }
    }
}

fn distance_to_segment(p: Point<Pixels>, a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let (px_, py) = (f32::from(p.x), f32::from(p.y));
    let (ax, ay, bx, by) = (
        f32::from(a.x),
        f32::from(a.y),
        f32::from(b.x),
        f32::from(b.y),
    );
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-6 {
        0.0
    } else {
        (((px_ - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (ax + t * dx, ay + t * dy);
    ((px_ - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Draw the handles: a shaft per axis with an arrow (move), a ring (rotate)
/// or a square (scale) at the tip, the active axis brighter, a white centre.
pub fn paint(handles: [Handle; 3], kind: Kind, active: Option<usize>) -> impl IntoElement {
    let p = crate::theme::t();
    let colors = [p.axis_x, p.axis_y, p.axis_z];
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            for h in handles {
                let len = f32::from(h.to.x - h.from.x).abs() + f32::from(h.to.y - h.from.y).abs();
                if len < 4.0 {
                    continue;
                }
                let on = active == Some(h.axis);
                let mut color = colors[h.axis];
                if active.is_some() && !on {
                    color.a = 0.35;
                }
                let mut shaft = PathBuilder::stroke(px(if on { 3.0 } else { 2.0 }));
                shaft.move_to(h.from);
                shaft.line_to(h.to);
                if let Ok(path) = shaft.build() {
                    window.paint_path(path, color);
                }
                let (dx, dy) = (
                    f32::from(h.to.x - h.from.x) / LENGTH,
                    f32::from(h.to.y - h.from.y) / LENGTH,
                );
                let at = |along: f32, side: f32| {
                    point(
                        h.to.x + px(dx * along - dy * side),
                        h.to.y + px(dy * along + dx * side),
                    )
                };
                let mut tip = match kind {
                    Kind::Move => {
                        let mut b = PathBuilder::fill();
                        b.add_polygon(&[at(12.0, 0.0), at(0.0, 6.0), at(0.0, -6.0)], true);
                        b
                    }
                    Kind::Scale => {
                        let mut b = PathBuilder::fill();
                        b.add_polygon(
                            &[at(-5.0, -5.0), at(5.0, -5.0), at(5.0, 5.0), at(-5.0, 5.0)],
                            true,
                        );
                        b
                    }
                    Kind::Rotate => {
                        let mut b = PathBuilder::stroke(px(2.0));
                        let n = 16;
                        let pts: Vec<_> = (0..n)
                            .map(|i| {
                                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                                at(6.0 * a.cos(), 6.0 * a.sin())
                            })
                            .collect();
                        b.add_polygon(&pts, true);
                        b
                    }
                };
                if let Ok(path) = std::mem::replace(&mut tip, PathBuilder::fill()).build() {
                    window.paint_path(path, color);
                }
            }
            let c = handles[0].from;
            let mut dot = PathBuilder::fill();
            let pts: Vec<_> = (0..12)
                .map(|i| {
                    let a = i as f32 / 12.0 * std::f32::consts::TAU;
                    point(c.x + px(4.0 * a.cos()), c.y + px(4.0 * a.sin()))
                })
                .collect();
            dot.add_polygon(&pts, true);
            if let Ok(path) = dot.build() {
                window.paint_path(path, rgb(0xffffff));
            }
        },
    )
    .absolute()
    .inset_0()
}

#[cfg(test)]
mod tests {
    use super::{Drag, Handle, Kind, distance_to_segment};
    use gpui_kit::{point, px};

    fn drag(kind: Kind) -> Drag {
        let h = Handle {
            axis: 0,
            from: point(px(100.0), px(100.0)),
            to: point(px(180.0), px(100.0)),
            px_per_metre: 50.0,
        };
        Drag::new(kind, h, point(px(100.0), px(100.0)))
    }

    #[::core::prelude::v1::test]
    fn moving_snaps_to_the_grid_unless_free() {
        let d = drag(Kind::Move);
        // 60 px along X at 50 px per metre is 1.2 m: snaps to 1 m.
        assert_eq!(d.amount(point(px(160.0), px(130.0)), 1.0, false), 1.0);
        assert!((d.amount(point(px(160.0), px(130.0)), 1.0, true) - 1.2).abs() < 1e-4);
        assert_eq!(d.args(1.0)["translate"], serde_json::json!([1.0, 0.0, 0.0]));
    }

    #[::core::prelude::v1::test]
    fn rotation_snaps_to_fifteen_degrees_and_scale_to_tenths() {
        assert_eq!(
            drag(Kind::Rotate).amount(point(px(120.0), px(100.0)), 1.0, false),
            15.0
        );
        let s = drag(Kind::Scale).amount(point(px(140.0), px(100.0)), 1.0, false);
        assert!((s - 1.5).abs() < 1e-4);
    }

    #[::core::prelude::v1::test]
    fn distance_to_a_segment_clamps_at_the_ends() {
        let (a, b) = (point(px(0.0), px(0.0)), point(px(10.0), px(0.0)));
        assert_eq!(distance_to_segment(point(px(5.0), px(3.0)), a, b), 3.0);
        assert_eq!(distance_to_segment(point(px(14.0), px(3.0)), a, b), 5.0);
    }
}
