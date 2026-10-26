//! App settings and recent projects (FR-UI-05, FR-PROJ-06), stored as JSON in
//! the OS config folder. A damaged file never stops Quarry from starting.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const RECENT_LIMIT: usize = 10;

/// Where Quarry keeps its own files. `QUARRY_DATA_DIR` puts everything in one
/// folder (tests, portable use).
#[derive(Clone, Debug)]
pub struct Dirs {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Dirs {
    pub fn from_env() -> Result<Self, String> {
        if let Some(dir) = std::env::var_os("QUARRY_DATA_DIR") {
            return Ok(Self::in_one(PathBuf::from(dir)));
        }
        let dirs = directories::ProjectDirs::from("", "", "Quarry")
            .ok_or("this system has no home folder to keep settings in")?;
        Ok(Self {
            config: dirs.config_dir().to_path_buf(),
            data: dirs.data_dir().to_path_buf(),
        })
    }

    pub fn in_one(dir: PathBuf) -> Self {
        Self {
            config: dir.clone(),
            data: dir,
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }
    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }
    pub fn autosaves(&self) -> PathBuf {
        self.data.join("autosave")
    }
    pub fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 1.0 to 2.0.
    pub ui_scale: f32,
    pub theme: Theme,
    /// Action name (`mesh.extrude`) to key chord (`cmd+e`). Only overrides of the preset.
    pub shortcuts: BTreeMap<String, String>,
    pub shortcut_preset: ShortcutPreset,
    /// App used by "Open in external editor"; None uses the system default.
    pub external_editor: Option<String>,
    pub autosave_minutes: u32,
    pub weight_ramp: WeightRamp,
    /// Most recent first, at most 10.
    pub recent_files: Vec<RecentFile>,
    pub window_state: Option<WindowState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            theme: Theme::Dark,
            shortcuts: BTreeMap::new(),
            shortcut_preset: ShortcutPreset::Default,
            external_editor: None,
            autosave_minutes: 2,
            weight_ramp: WeightRamp::Classic,
            recent_files: Vec::new(),
            window_state: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutPreset {
    Default,
    Blender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightRamp {
    Classic,
    Colorblind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    /// Seconds since the Unix epoch.
    pub opened_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// x, y, width, height in logical pixels.
    pub bounds: [f32; 4],
    pub maximized: bool,
    /// Panel name to its width or height in logical pixels.
    pub panels: BTreeMap<String, f32>,
}

impl Settings {
    /// Load settings. A missing file gives defaults; a damaged one is moved aside
    /// to `settings.json.bad` and also gives defaults, with the reason returned.
    pub fn load(dirs: &Dirs) -> (Self, Option<String>) {
        let path = dirs.settings_file();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => {
                return (
                    Self::default(),
                    Some(format!(
                        "Could not read {}: {e}. Using default settings.",
                        path.display()
                    )),
                );
            }
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(settings) => (settings.clamped(), None),
            Err(e) => {
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                (
                    Self::default(),
                    Some(format!(
                        "Settings file was damaged ({e}). Using defaults; the old file is kept as settings.json.bad."
                    )),
                )
            }
        }
    }

    pub fn save(&self, dirs: &Dirs) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(&dirs.settings_file(), &json)
            .map_err(|e| format!("Could not save settings: {e}."))
    }

    /// Values outside their range come back to the nearest valid one.
    fn clamped(mut self) -> Self {
        self.ui_scale = if self.ui_scale.is_finite() {
            self.ui_scale.clamp(1.0, 2.0)
        } else {
            1.0
        };
        self.autosave_minutes = self.autosave_minutes.clamp(1, 60);
        self.recent_files.truncate(RECENT_LIMIT);
        self
    }

    /// Put `path` at the top of the recent list.
    pub fn add_recent(&mut self, path: &Path) {
        self.recent_files.retain(|r| r.path != path);
        let opened_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.recent_files.insert(
            0,
            RecentFile {
                path: path.to_path_buf(),
                opened_at,
            },
        );
        self.recent_files.truncate(RECENT_LIMIT);
    }

    /// The recent list with each entry marked missing when the file has moved.
    pub fn recent_with_status(&self) -> Vec<(&RecentFile, bool)> {
        self.recent_files
            .iter()
            .map(|r| (r, r.path.is_file()))
            .collect()
    }
}

/// Write via a temp file and rename, creating the folder if needed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    let temp = path.with_file_name(name);
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(test: &str) -> Dirs {
        let dir =
            std::env::temp_dir().join(format!("quarry-settings-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Dirs::in_one(dir)
    }

    #[test]
    fn settings_survive_a_restart() {
        let dirs = dirs("restart");
        let mut s = Settings {
            ui_scale: 1.5,
            ..Settings::default()
        };
        s.shortcuts.insert("mesh.extrude".into(), "e".into());
        s.save(&dirs).unwrap();
        let (back, warning) = Settings::load(&dirs);
        assert_eq!(back, s);
        assert!(warning.is_none());
    }

    #[test]
    fn damaged_file_falls_back_to_defaults() {
        let dirs = dirs("damaged");
        std::fs::create_dir_all(&dirs.config).unwrap();
        std::fs::write(dirs.settings_file(), "{ not json").unwrap();
        let (s, warning) = Settings::load(&dirs);
        assert_eq!(s, Settings::default());
        assert!(warning.unwrap().contains("settings.json.bad"));
        assert!(dirs.config.join("settings.json.bad").exists());

        std::fs::write(
            dirs.settings_file(),
            r#"{"ui_scale": 9, "autosave_minutes": 0}"#,
        )
        .unwrap();
        let (s, _) = Settings::load(&dirs);
        assert_eq!(
            (s.ui_scale, s.autosave_minutes),
            (2.0, 1),
            "out-of-range values are clamped"
        );
    }

    #[test]
    fn recent_list_is_newest_first_and_marks_missing_files() {
        let dirs = dirs("recent");
        std::fs::create_dir_all(&dirs.data).unwrap();
        let files: Vec<PathBuf> = ["a", "b", "c"]
            .iter()
            .map(|n| dirs.data.join(format!("{n}.quarry")))
            .collect();
        let mut s = Settings::default();
        for f in &files {
            std::fs::write(f, b"x").unwrap();
            s.add_recent(f);
        }
        s.add_recent(&files[0]); // reopening moves it to the top, no duplicate
        std::fs::remove_file(&files[1]).unwrap();

        let listed: Vec<_> = s
            .recent_with_status()
            .into_iter()
            .map(|(r, exists)| {
                (
                    r.path.file_name().unwrap().to_str().unwrap().to_string(),
                    exists,
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                ("a.quarry".into(), true),
                ("c.quarry".into(), true),
                ("b.quarry".into(), false)
            ]
        );
        for i in 0..15 {
            s.add_recent(&dirs.data.join(format!("{i}.quarry")));
        }
        assert_eq!(s.recent_files.len(), RECENT_LIMIT);
    }
}
