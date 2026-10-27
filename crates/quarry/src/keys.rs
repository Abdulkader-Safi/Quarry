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
    ("export.again", "Export again", "Global", "cmd+shift+e"),
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
    ("scene.duplicate", "Duplicate", "Scene", "shift+d"),
    ("tool.move", "Move", "Mesh", "g"),
    ("tool.rotate", "Rotate", "Mesh", "r"),
    ("tool.scale", "Scale", "Mesh", "t"),
    ("rig.bone_add", "Add bone", "Rig", "b"),
    ("anim.play", "Play or pause", "Animate", "space"),
];

/// The Blender-style preset: what it changes from the defaults. Its S scales,
/// so the crosshair keys (WASD) are left unbound.
pub const BLENDER: &[(&str, &str)] = &[
    ("palette", "f3"),
    ("mesh.delete", "x"),
    ("tool.scale", "s"),
    ("view.frame", "."),
    ("rig.bone_add", "shift+a"),
    ("cursor.forward", ""),
    ("cursor.back", ""),
    ("cursor.left", ""),
    ("cursor.right", ""),
    ("cursor.down", ""),
    ("cursor.up", ""),
];

/// The preset's changes as overrides (empty for Quarry's own).
pub fn preset(blender: bool) -> BTreeMap<String, String> {
    if !blender {
        return BTreeMap::new();
    }
    BLENDER
        .iter()
        .map(|(a, c)| (a.to_string(), c.to_string()))
        .collect()
}

/// The keymap in force: the preset, then the user's own changes on top.
pub fn layered(blender: bool, user: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut map = preset(blender);
    map.extend(user.iter().map(|(a, c)| (a.clone(), c.clone())));
    map
}

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

/// Bind `action` to `chord` in the user's overrides, on top of `preset`.
/// Another action holding the chord is unbound; its label comes back so the
/// caller can say so.
pub fn rebind(
    user: &mut BTreeMap<String, String>,
    preset: &BTreeMap<String, String>,
    action: &str,
    chord: &str,
) -> Option<&'static str> {
    let merged = {
        let mut m = preset.clone();
        m.extend(user.iter().map(|(a, c)| (a.clone(), c.clone())));
        m
    };
    let taken = action_for(&merged, chord).filter(|a| *a != action);
    if let Some(other) = taken {
        user.insert(other.into(), String::new());
    }
    if bound(preset, action) == chord {
        user.remove(action);
    } else {
        user.insert(action.into(), chord.into());
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
    use super::{action_for, bound, layered, preset, rebind};
    use std::collections::BTreeMap;

    #[::core::prelude::v1::test]
    fn rebinding_takes_the_chord_from_its_old_owner() {
        let (mut o, none) = (BTreeMap::new(), BTreeMap::new());
        assert_eq!(action_for(&o, "e"), Some("mesh.extrude"));
        // Give Extrude's E to Frame selection.
        assert_eq!(rebind(&mut o, &none, "view.frame", "e"), Some("Extrude"));
        assert_eq!(action_for(&o, "e"), Some("view.frame"));
        assert_eq!(bound(&o, "mesh.extrude"), "");
        assert_eq!(action_for(&o, "f"), None);
        // Back to the default leaves no override for it.
        rebind(&mut o, &none, "view.frame", "f");
        assert!(!o.contains_key("view.frame"));
    }

    #[::core::prelude::v1::test]
    fn the_blender_preset_layers_under_the_users_changes() {
        let blender = preset(true);
        let mut user = BTreeMap::new();
        let map = layered(true, &user);
        assert_eq!(
            action_for(&map, "s"),
            Some("tool.scale"),
            "S scales, not the crosshair"
        );
        assert_eq!(action_for(&map, "x"), Some("mesh.delete"));
        assert_eq!(bound(&map, "cursor.back"), "");
        // Setting a key back to the preset's own leaves no user override.
        rebind(&mut user, &blender, "tool.scale", "t");
        assert_eq!(bound(&layered(true, &user), "tool.scale"), "t");
        rebind(&mut user, &blender, "tool.scale", "s");
        assert!(!user.contains_key("tool.scale"));
    }
}
