//! The keymap: every keyboard action, its default chord, and the user's
//! overrides from settings. Chords are written `cmd+shift+z`; an empty
//! override unbinds an action.

use gpui_kit::Keystroke;
use std::collections::BTreeMap;

/// (action, label, area, default chord). The action is the settings key.
pub const ACTIONS: &[(&str, &str, &str, &str)] = &[
    ("palette", "Command search", "Global", "cmd+k"),
    ("history.undo", "Undo", "Global", "cmd+z"),
    ("history.redo", "Redo", "Global", "cmd+shift+z"),
    ("project.save", "Save", "Global", "cmd+s"),
    ("project.open", "Open", "Global", "cmd+o"),
    ("export.run", "Export", "Global", "cmd+e"),
    ("view.frame", "Frame selection", "View", "f"),
    ("cursor.forward", "Crosshair forward", "View", "w"),
    ("cursor.back", "Crosshair back", "View", "s"),
    ("cursor.left", "Crosshair left", "View", "a"),
    ("cursor.right", "Crosshair right", "View", "d"),
    ("cursor.down", "Crosshair down", "View", "q"),
    ("cursor.up", "Crosshair up", "View", "shift+e"),
    ("select.object", "Object mode", "Mesh", "1"),
    ("select.vertex", "Vertex mode", "Mesh", "2"),
    ("select.edge", "Edge mode", "Mesh", "3"),
    ("select.face", "Face mode", "Mesh", "4"),
    ("mesh.extrude", "Extrude", "Mesh", "e"),
    ("mesh.delete", "Delete", "Mesh", "backspace"),
    ("tool.move", "Move", "Mesh", "g"),
    ("tool.rotate", "Rotate", "Mesh", "r"),
    ("tool.scale", "Scale", "Mesh", "t"),
    ("rig.bone_add", "Add bone", "Rig", "b"),
    ("anim.play", "Play or pause", "Animate", "space"),
];

/// Mouse bindings, shown in the list but not rebindable.
pub const MOUSE: &[(&str, &str, &str)] = &[
    ("Place tile", "Tiles", "Click"),
    ("Erase tile", "Tiles", "Right-click"),
    ("Orbit", "View", "⌥ drag"),
    ("Pan", "View", "⇧⌥ drag"),
];

/// The chord a keystroke reads as. Control counts as cmd so Windows and
/// Linux get the same defaults; Delete and Backspace are one key.
pub fn chord(k: &Keystroke) -> String {
    let m = &k.modifiers;
    let mut s = String::new();
    if m.platform || m.control {
        s += "cmd+";
    }
    if m.alt {
        s += "alt+";
    }
    if m.shift {
        s += "shift+";
    }
    s += match k.key.as_str() {
        "delete" => "backspace",
        key => key,
    };
    s
}

/// The chord bound to `action`, overrides first.
pub fn bound<'a>(overrides: &'a BTreeMap<String, String>, action: &str) -> &'a str {
    overrides.get(action).map_or_else(
        || ACTIONS.iter().find(|a| a.0 == action).map_or("", |a| a.3),
        String::as_str,
    )
}

/// The action a chord runs, if any.
pub fn action_for(overrides: &BTreeMap<String, String>, chord: &str) -> Option<&'static str> {
    ACTIONS
        .iter()
        .map(|a| a.0)
        .find(|a| !chord.is_empty() && bound(overrides, a) == chord)
}

/// Bind `action` to `chord`. Another action holding the chord is unbound;
/// its label comes back so the caller can say so.
pub fn rebind(
    overrides: &mut BTreeMap<String, String>,
    action: &str,
    chord: &str,
) -> Option<&'static str> {
    let taken = action_for(overrides, chord).filter(|a| *a != action);
    if let Some(other) = taken {
        overrides.insert(other.into(), String::new());
    }
    let default = ACTIONS.iter().find(|a| a.0 == action).map(|a| a.3);
    if default == Some(chord) {
        overrides.remove(action);
    } else {
        overrides.insert(action.into(), chord.into());
    }
    taken.and_then(|t| ACTIONS.iter().find(|a| a.0 == t).map(|a| a.1))
}

/// A chord as the design prints it: `⇧⌘Z`, `E`, `Space`.
pub fn display(chord: &str) -> String {
    if chord.is_empty() {
        return "None".into();
    }
    let mut mods = String::new();
    let mut key = "";
    for part in chord.split('+') {
        match part {
            "shift" => mods += "⇧",
            "alt" => mods += "⌥",
            "cmd" => {
                mods += if cfg!(target_os = "macos") {
                    "⌘"
                } else {
                    "Ctrl+"
                }
            }
            k => key = k,
        }
    }
    let key = match key {
        "backspace" => "⌫".to_string(),
        "space" => "Space".into(),
        "enter" => "↩".into(),
        "tab" => "Tab".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        k => k.to_uppercase(),
    };
    mods + &key
}

#[cfg(test)]
mod tests {
    use super::{action_for, bound, rebind};
    use std::collections::BTreeMap;

    #[::core::prelude::v1::test]
    fn rebinding_takes_the_chord_from_its_old_owner() {
        let mut o = BTreeMap::new();
        assert_eq!(action_for(&o, "e"), Some("mesh.extrude"));
        // Give Extrude's E to Frame selection.
        assert_eq!(rebind(&mut o, "view.frame", "e"), Some("Extrude"));
        assert_eq!(action_for(&o, "e"), Some("view.frame"));
        assert_eq!(bound(&o, "mesh.extrude"), "");
        assert_eq!(action_for(&o, "f"), None);
        // Back to the default leaves no override for it.
        rebind(&mut o, "view.frame", "f");
        assert!(!o.contains_key("view.frame"));
    }
}
