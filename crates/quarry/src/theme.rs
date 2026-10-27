//! Design tokens from the Quarry Figma file (Theme, Layout and type styles).
//! Dark and Light are the two Theme modes; everything reads the active one.

use gpui_kit::*;
use std::sync::atomic::{AtomicBool, Ordering};

// Every Theme variable from the design file, used or not yet.
#[allow(dead_code)]
pub struct Palette {
    pub bg_app: Rgba,
    pub bg_panel: Rgba,
    pub bg_raised: Rgba,
    pub bg_hover: Rgba,
    pub bg_active: Rgba,
    pub bg_input: Rgba,
    pub border_subtle: Rgba,
    pub border_default: Rgba,
    pub border_strong: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_on_accent: Rgba,
    pub text_on_axis: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_pressed: Rgba,
    pub accent_subtle: Rgba,
    pub accent_strong_subtle: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub success_subtle: Rgba,
    pub warning_subtle: Rgba,
    pub danger_subtle: Rgba,
    pub info_subtle: Rgba,
    pub play: Rgba,
    pub axis_x: Rgba,
    pub axis_y: Rgba,
    pub axis_z: Rgba,
    pub agent: Rgba,
}

const fn hex(v: u32) -> Rgba {
    Rgba {
        r: ((v >> 16) & 0xff) as f32 / 255.0,
        g: ((v >> 8) & 0xff) as f32 / 255.0,
        b: (v & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

pub const DARK: Palette = Palette {
    bg_app: hex(0x0b0c0e),
    bg_panel: hex(0x16181c),
    bg_raised: hex(0x22262c),
    bg_hover: hex(0x2a2f36),
    bg_active: hex(0x343a42),
    bg_input: hex(0x111316),
    border_subtle: hex(0x1b1e23),
    border_default: hex(0x2a2f36),
    border_strong: hex(0x4a515b),
    text_primary: hex(0xeceef1),
    text_secondary: hex(0xb4bac3),
    text_muted: hex(0x6b737e),
    text_on_accent: hex(0xffffff),
    text_on_axis: hex(0x0b0c0e),
    accent: hex(0x3873d3),
    accent_hover: hex(0x5c91e3),
    accent_pressed: hex(0x2c5fb3),
    accent_subtle: hex(0x1c283d),
    accent_strong_subtle: hex(0x22375a),
    success: hex(0x4ade80),
    warning: hex(0xfacc15),
    danger: hex(0xff6b6b),
    success_subtle: hex(0x183427),
    warning_subtle: hex(0x383119),
    danger_subtle: hex(0x391f22),
    info_subtle: hex(0x172a40),
    play: hex(0x22c55e),
    axis_x: hex(0xff4d5e),
    axis_y: hex(0x7bdc4b),
    axis_z: hex(0x4d8bff),
    agent: hex(0xa78bfa),
};

pub const LIGHT: Palette = Palette {
    bg_app: hex(0xe4e7eb),
    bg_panel: hex(0xffffff),
    bg_raised: hex(0xf6f7f9),
    bg_hover: hex(0xeceef1),
    bg_active: hex(0xd5d9df),
    bg_input: hex(0xffffff),
    border_subtle: hex(0xeceef1),
    border_default: hex(0xd5d9df),
    border_strong: hex(0x8e96a1),
    text_primary: hex(0x111316),
    text_secondary: hex(0x343a42),
    text_muted: hex(0x6b737e),
    text_on_accent: hex(0xffffff),
    text_on_axis: hex(0x0b0c0e),
    accent: hex(0x2c5fb3),
    accent_hover: hex(0x3873d3),
    accent_pressed: hex(0x234b8d),
    accent_subtle: hex(0xdbe6f7),
    accent_strong_subtle: hex(0xbbcff0),
    success: hex(0x22c55e),
    warning: hex(0xeab308),
    danger: hex(0xef4444),
    success_subtle: hex(0xdcf6e5),
    warning_subtle: hex(0xfcf3d8),
    danger_subtle: hex(0xfce1e1),
    info_subtle: hex(0xddebff),
    play: hex(0x22c55e),
    axis_x: hex(0xff4d5e),
    axis_y: hex(0x7bdc4b),
    axis_z: hex(0x4d8bff),
    agent: hex(0xa78bfa),
};

static LIGHT_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_light(on: bool) {
    LIGHT_MODE.store(on, Ordering::Relaxed);
}

pub fn is_light() -> bool {
    LIGHT_MODE.load(Ordering::Relaxed)
}

/// The active palette.
pub fn t() -> &'static Palette {
    if is_light() { &LIGHT } else { &DARK }
}

pub const SANS: &str = "Geist";
pub const MONO: &str = "Geist Mono";
pub const DISPLAY: &str = "Bricolage Grotesque";

/// The type styles in the design file.
#[derive(Clone, Copy)]
pub enum Type {
    /// Bricolage Bold 44/48.
    DisplayL,
    /// Geist SemiBold 22/28.
    H1,
    /// Geist SemiBold 13/18.
    H3,
    /// Geist Medium 12/16.
    LabelM,
    /// Geist Medium 11/14.
    LabelS,
    /// Geist Regular 13/18.
    BodyM,
    /// Geist Regular 12/16.
    BodyS,
    /// Geist SemiBold 10/12, upper case.
    Caption,
    /// Geist Mono 12/16.
    MonoM,
    /// Geist Mono 11/14.
    MonoS,
}

impl Type {
    fn spec(self) -> (&'static str, FontWeight, f32, f32) {
        match self {
            Type::DisplayL => (DISPLAY, FontWeight::BOLD, 44.0, 48.0),
            Type::H1 => (SANS, FontWeight::SEMIBOLD, 22.0, 28.0),
            Type::H3 => (SANS, FontWeight::SEMIBOLD, 13.0, 18.0),
            Type::LabelM => (SANS, FontWeight::MEDIUM, 12.0, 16.0),
            Type::LabelS => (SANS, FontWeight::MEDIUM, 11.0, 14.0),
            Type::BodyM => (SANS, FontWeight::NORMAL, 13.0, 18.0),
            Type::BodyS => (SANS, FontWeight::NORMAL, 12.0, 16.0),
            Type::Caption => (SANS, FontWeight::SEMIBOLD, 10.0, 12.0),
            Type::MonoM => (MONO, FontWeight::NORMAL, 12.0, 16.0),
            Type::MonoS => (MONO, FontWeight::NORMAL, 11.0, 14.0),
        }
    }
}

/// One line of text in a type style and colour.
pub fn text(s: impl Into<SharedString>, ty: Type, color: Rgba) -> Div {
    let (family, weight, size, line) = ty.spec();
    let s: SharedString = s.into();
    let s = if matches!(ty, Type::Caption) {
        s.to_uppercase().into()
    } else {
        s
    };
    div()
        .font_family(family)
        .font_weight(weight)
        .text_size(px(size))
        .line_height(px(line))
        .text_color(color)
        .whitespace_nowrap()
        .child(s)
}

/// Elevation/Popover: 0 8 24 #00000073.
pub fn popover_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0.0, 0.0, 0.0, 0.45),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(0.0),
        inset: false,
    }]
}

/// Elevation/Modal: 0 24 64 #00000099.
pub fn modal_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0.0, 0.0, 0.0, 0.6),
        offset: point(px(0.0), px(24.0)),
        blur_radius: px(64.0),
        spread_radius: px(0.0),
        inset: false,
    }]
}

/// Focus/Ring: a 3px cobalt halo.
pub fn focus_ring() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: Rgba {
            a: 0.45,
            ..hex(0x3873d4)
        }
        .into(),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(3.0),
        inset: false,
    }]
}

/// Load the bundled fonts (OFL, see assets/fonts/LICENSE.md).
pub fn load_fonts(cx: &mut App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = vec![
        include_bytes!("../../../assets/fonts/geist-400.ttf")
            .as_slice()
            .into(),
        include_bytes!("../../../assets/fonts/geist-500.ttf")
            .as_slice()
            .into(),
        include_bytes!("../../../assets/fonts/geist-600.ttf")
            .as_slice()
            .into(),
        include_bytes!("../../../assets/fonts/geist-mono-400.ttf")
            .as_slice()
            .into(),
        include_bytes!("../../../assets/fonts/bricolage-grotesque-700.ttf")
            .as_slice()
            .into(),
    ];
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        tracing::warn!("could not load the bundled fonts: {e}");
    }
}
