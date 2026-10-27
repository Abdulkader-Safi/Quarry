//! Embedded icons and the logo glyph, exported from the Quarry Figma file.
//! `quarry/<name>.svg` is ours; anything else falls through to the kit's icons.

use gpui_kit::*;
use std::borrow::Cow;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        fn icon_bytes(name: &str) -> Option<&'static [u8]> {
            match name {
                $($name => Some(include_bytes!(concat!("../../../assets/icons/", $name, ".svg"))),)*
                "logo-glyph" => Some(include_bytes!("../../../assets/logo-glyph.svg")),
                _ => None,
            }
        }
    };
}

icons!(
    "bone",
    "brush",
    "camera",
    "check",
    "chevron-down",
    "chevron-right",
    "close",
    "command",
    "crosshair",
    "cube",
    "eraser",
    "export",
    "eye-off",
    "eye",
    "eyedropper",
    "file",
    "fill",
    "folder",
    "grid",
    "history",
    "image",
    "info",
    "keyboard",
    "keyframe",
    "knife",
    "layers",
    "link",
    "lock",
    "magnet",
    "moon",
    "more",
    "mouse",
    "move",
    "pause",
    "play",
    "plug",
    "plus",
    "pointer",
    "redo",
    "refresh",
    "rotate",
    "save",
    "scale",
    "search",
    "select-box",
    "settings",
    "shapes",
    "sidebar-bottom",
    "sidebar-left",
    "sledgehammer",
    "sliders",
    "step",
    "stop",
    "sun",
    "target",
    "terminal",
    "diamond",
    "dot",
    "tiles",
    "trash",
    "undo",
    "wand",
    "warning",
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path
            .strip_prefix("quarry/")
            .and_then(|p| p.strip_suffix(".svg"))
        {
            return Ok(icon_bytes(name).map(Cow::Borrowed));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}

/// Asset path of a Reicon icon by its Figma name ("plus" for Icon/plus).
pub fn icon_path(name: &str) -> SharedString {
    format!("quarry/{name}.svg").into()
}
