//! Export (FR-EXP-01, 03, 04, 05, 06, 08). The writers live in quarry-io.

use super::object;
use crate::command::{
    Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode, ExportRequest,
};
use crate::id::Id;
use crate::model::{ExportFormat, ExportPreset, ExportSelection, Object};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<ExportRun>();
    r.register::<ImportGltf>();
    r.register::<ExportPresetSave>();
    r.register::<ExportPresetRun>();
    r.register::<ExportPresetDelete>();
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportGltf {
    /// A .gltf or .glb file.
    pub path: PathBuf,
}

impl Command for ImportGltf {
    const NAME: &'static str = "import.gltf";
    const DESCRIPTION: &'static str = "Import a .gltf or .glb: each node stays its own object with its name, textures are copied in, skins become editable skeletons and animations become clips. One undo step.";
    const LONG: bool = true;

    fn run(self, cx: &mut Context) -> CommandResult {
        cx.step(0.0)?;
        let fragment = cx.storage()?.import_gltf(&self.path)?;
        cx.step(0.9)?;
        let counts = (
            fragment.objects.len(),
            fragment.images.len(),
            fragment.skeletons.len(),
            fragment.clips.len(),
        );
        let roots = cx.doc().merge(fragment);
        Ok(serde_json::json!({
            "objects": counts.0,
            "images": counts.1,
            "skeletons": counts.2,
            "clips": counts.3,
            "roots": roots,
        }))
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Which {
    /// Every top-level object and everything under it.
    #[default]
    All,
    /// The selected objects and their children.
    Selected,
    /// The objects named in `objects`, with their children.
    Objects,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportRun {
    /// Where to write. `.glb` is added if missing; gltf_separate writes a
    /// `.gltf` with its `.bin` and PNGs beside it.
    pub path: PathBuf,
    /// glb (default, one self-contained file), gltf_separate (for debugging)
    /// or obj (static meshes, with a .mtl and PNGs beside it).
    pub format: Option<ExportFormat>,
    #[serde(default)]
    pub selection: Which,
    /// Object IDs or names when selection is objects.
    #[serde(default)]
    pub objects: Vec<String>,
    /// Multiply the whole export by this. Default 1.
    pub scale: Option<f32>,
}

impl Command for ExportRun {
    const NAME: &'static str = "export.run";
    const DESCRIPTION: &'static str = "Export the scene or some objects as glTF: one self-contained .glb with meshes, textures (nearest filtering for pixel art), node names and extras for markers, collision and properties. Returns the files written and a check report.";
    const LONG: bool = true;
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let format = self.format.unwrap_or(ExportFormat::Glb);
        let scale = self.scale.unwrap_or(1.0);
        if !(scale.is_finite() && scale > 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "scale must be a positive number.",
            ));
        }
        let objects: Vec<Id<Object>> = match self.selection {
            Which::All => doc
                .objects
                .iter()
                .filter(|o| o.parent.is_none())
                .map(|o| o.id)
                .collect(),
            Which::Selected => doc.selection.objects.iter().copied().collect(),
            Which::Objects => self
                .objects
                .iter()
                .map(|r| object(doc, r))
                .collect::<Result<_, _>>()?,
        };
        if objects.is_empty() {
            return Err(CommandError::new(
                ErrorCode::NothingSelected,
                "There is nothing to export. Add objects or pick some to export.",
            ));
        }
        let mut path = self.path;
        let ext = match format {
            ExportFormat::GltfSeparate => "gltf",
            ExportFormat::Obj => "obj",
            ExportFormat::Glb => "glb",
        };
        if path.extension().is_none_or(|e| e != ext) {
            path.as_mut_os_string().push(format!(".{ext}"));
        }
        cx.step(0.0)?;
        let request = ExportRequest {
            path,
            format,
            objects,
            scale,
        };
        let report = cx.storage()?.export(&cx.state.doc, &request)?;
        cx.step(1.0)?;
        Ok(report)
    }
}

// ---- presets (FR-EXP-06) ---------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportPresetSave {
    /// The preset's name; saving under an existing name replaces it.
    pub name: String,
    /// Where it exports. Inside the project folder it is kept relative, so the
    /// preset still works when the folder moves.
    pub path: PathBuf,
    pub format: Option<ExportFormat>,
    #[serde(default)]
    pub selection: Which,
    /// Object IDs or names when selection is objects.
    #[serde(default)]
    pub objects: Vec<String>,
    pub scale: Option<f32>,
}

impl Command for ExportPresetSave {
    const NAME: &'static str = "export.preset_save";
    const DESCRIPTION: &'static str = "Save export settings (path, format, selection, scale) as a named preset in the project. The newest preset is the one export.preset_run uses by default.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if self.name.trim().is_empty() {
            return Err(CommandError::invalid("A preset needs a name."));
        }
        let scale = self.scale.unwrap_or(1.0);
        if !(scale.is_finite() && scale > 0.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "scale must be a positive number.",
            ));
        }
        let folder = cx
            .state
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from);
        let path = match &folder {
            Some(dir) => self
                .path
                .strip_prefix(dir)
                .unwrap_or(&self.path)
                .to_path_buf(),
            None => self.path.clone(),
        };
        let preset = ExportPreset {
            name: self.name.clone(),
            path: path.to_string_lossy().into_owned(),
            format: self.format.unwrap_or(ExportFormat::Glb),
            selection: match self.selection {
                Which::All => ExportSelection::All,
                Which::Selected => ExportSelection::Selected,
                Which::Objects => ExportSelection::Objects(self.objects),
            },
            scale,
        };
        let presets = &mut cx.doc().export_presets;
        presets.retain(|p| p.name != self.name);
        presets.insert(0, preset);
        Ok(serde_json::json!({ "preset": self.name, "path": path }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportPresetRun {
    /// Preset name. Default the most recent one.
    pub name: Option<String>,
}

impl Command for ExportPresetRun {
    const NAME: &'static str = "export.preset_run";
    const DESCRIPTION: &'static str =
        "Export again with a saved preset (default the most recent). Same report as export.run.";
    const LONG: bool = true;
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let presets = &cx.state.doc.export_presets;
        let preset = match &self.name {
            Some(n) => presets.iter().find(|p| &p.name == n),
            None => presets.first(),
        }
        .cloned()
        .ok_or_else(|| {
            CommandError::not_found(
                "No export preset yet. Export once and save it with export.preset_save.",
            )
        })?;
        let mut path = PathBuf::from(&preset.path);
        if path.is_relative() {
            let Some(dir) = cx.state.path.as_ref().and_then(|p| p.parent()) else {
                return Err(CommandError::invalid(
                    "This preset's path is relative to the project folder. Save the project first.",
                ));
            };
            path = dir.join(path);
        }
        let (selection, objects) = match preset.selection {
            ExportSelection::All => (Which::All, Vec::new()),
            ExportSelection::Selected => (Which::Selected, Vec::new()),
            ExportSelection::Objects(list) => (Which::Objects, list),
        };
        let mut report = ExportRun {
            path,
            format: Some(preset.format),
            selection,
            objects,
            scale: Some(preset.scale),
        }
        .run(cx)?;
        report["preset"] = serde_json::json!(preset.name);
        Ok(report)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportPresetDelete {
    pub name: String,
}

impl Command for ExportPresetDelete {
    const NAME: &'static str = "export.preset_delete";
    const DESCRIPTION: &'static str = "Remove a saved export preset.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let presets = &mut cx.doc().export_presets;
        let before = presets.len();
        presets.retain(|p| p.name != self.name);
        if presets.len() == before {
            return Err(CommandError::not_found(format!(
                "No preset called '{}'.",
                self.name
            )));
        }
        Ok(serde_json::json!({ "deleted": self.name }))
    }
}
