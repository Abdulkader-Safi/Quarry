//! Export (FR-EXP-01, 03, 04, 05). The writer lives in quarry-io.

use super::object;
use crate::command::{
    Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode, ExportRequest,
};
use crate::id::Id;
use crate::model::{ExportFormat, Object};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<ExportRun>();
    r.register::<ImportGltf>();
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
    /// glb (default, one self-contained file) or gltf_separate (for debugging).
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
        if format == ExportFormat::Obj {
            return Err(CommandError::invalid(
                "OBJ export is not built yet. Use glb.",
            ));
        }
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
            _ => "glb",
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
