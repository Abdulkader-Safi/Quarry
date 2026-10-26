//! Linked art (FR-ART-01 to 04, 06). The project always keeps its own copy of
//! every image; a link adds the source path so changes on disk flow in.

use super::tile::add_image;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{Document, Image, ImageRole};
use quarry_ops::tile as ops;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<ArtImport>();
    r.register::<ArtLink>();
    r.register::<ArtUnlink>();
    r.register::<ArtReload>();
    r.register::<ArtWatch>();
    r.register::<QueryImages>();
}

fn image(doc: &Document, reference: &str) -> Result<Id<Image>, CommandError> {
    doc.images.find(reference).map(|i| i.id).ok_or_else(|| {
        CommandError::not_found(format!(
            "No image called '{reference}'. Call query.images to list them."
        ))
    })
}

/// Swap in new pixels, keeping UVs in pixel positions (FR-ART-04).
fn replace_pixels(cx: &mut Context, id: Id<Image>, path: &Path) -> Result<Value, CommandError> {
    let loaded = cx.storage()?.read_image(path)?;
    let image = cx.doc().images.get_mut(id).expect("resolved");
    let old = image.size;
    image.size = loaded.size;
    image.pixels = Arc::from(loaded.pixels);
    image.source_hash = loaded.hash;
    image.source_path = Some(path.to_string_lossy().into_owned());
    let mut reply = json!({ "id": id, "size": loaded.size });
    if old != loaded.size {
        reply["warning"] = json!(format!(
            "The image changed size from {}x{} to {}x{}. Faces keep their pixel positions, so they show the same pixels; anything that moved on the canvas needs its UVs adjusting.",
            old[0], old[1], loaded.size[0], loaded.size[1]
        ));
    }
    Ok(reply)
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportAs {
    Tileset,
    Texture,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtImport {
    /// PNG or PSD file.
    pub path: PathBuf,
    /// Use it as a tileset or as a painted texture.
    #[serde(rename = "as")]
    pub as_: ImportAs,
    /// Tilesets only. Default the project's tile_px.
    pub tile_px: Option<u32>,
    /// Tilesets only. Default 0.
    pub padding: Option<u32>,
    pub name: Option<String>,
}

impl Command for ArtImport {
    const NAME: &'static str = "art.import";
    const DESCRIPTION: &'static str = "Import a PNG or PSD as a tileset or texture (what a file drop does). The project keeps a copy and stays linked to the file.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let tile_px = self.tile_px.unwrap_or(cx.state.doc.settings.tile_px);
        let role = match self.as_ {
            ImportAs::Tileset => ImageRole::Tileset,
            ImportAs::Texture => ImageRole::Texture,
        };
        let id = add_image(
            cx,
            self.path,
            role,
            self.name,
            tile_px,
            self.padding.unwrap_or(0),
        )?;
        let image = cx.state.doc.images.get(id).expect("inserted");
        Ok(json!({ "id": id, "name": image.name, "size": image.size }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtLink {
    /// Image ID or name.
    pub image: String,
    /// The PNG or PSD to follow.
    pub path: PathBuf,
}

impl Command for ArtLink {
    const NAME: &'static str = "art.link";
    const DESCRIPTION: &'static str = "Link an image to a PNG or PSD on disk and load it now. Later saves of that file refresh the image.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let id = image(&cx.state.doc, &self.image)?;
        replace_pixels(cx, id, &self.path)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtUnlink {
    pub image: String,
}

impl Command for ArtUnlink {
    const NAME: &'static str = "art.unlink";
    const DESCRIPTION: &'static str =
        "Stop following an image's file on disk. The project keeps its copy.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let id = image(&cx.state.doc, &self.image)?;
        cx.doc().images.get_mut(id).expect("resolved").source_path = None;
        Ok(json!({ "id": id, "linked": false }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtReload {
    pub image: String,
}

impl Command for ArtReload {
    const NAME: &'static str = "art.reload";
    const DESCRIPTION: &'static str = "Reload a linked image from its file. Nothing changes if the file is the same as last time.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let id = image(doc, &self.image)?;
        let img = doc.images.get(id).expect("resolved");
        let path = img.source_path.clone().map(PathBuf::from).ok_or_else(|| {
            CommandError::invalid(format!(
                "'{}' is not linked to a file. Call art.link first.",
                img.name
            ))
        })?;
        if !path.is_file() {
            return Err(CommandError::new(
                ErrorCode::Io,
                format!(
                    "The linked file {} is missing. The project still has its copy; relink with art.link.",
                    path.display()
                ),
            ));
        }
        let hash = img.source_hash;
        let loaded = cx.storage()?.read_image(&path)?;
        if loaded.hash == hash {
            return Ok(json!({ "id": id, "changed": false }));
        }
        let mut reply = replace_pixels(cx, id, &path)?;
        reply["changed"] = json!(true);
        Ok(reply)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtWatch {
    pub image: String,
    /// Reload automatically when the linked file changes.
    pub watch: bool,
}

impl Command for ArtWatch {
    const NAME: &'static str = "art.watch";
    const DESCRIPTION: &'static str = "Turn live refresh on or off for one linked image.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let id = image(&cx.state.doc, &self.image)?;
        cx.doc().images.get_mut(id).expect("resolved").watch = self.watch;
        Ok(json!({ "id": id, "watch": self.watch }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryImages {}

impl Command for QueryImages {
    const NAME: &'static str = "query.images";
    const DESCRIPTION: &'static str = "Tilesets and textures: id, name, size, tile grid, linked file and whether that file is missing.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let list: Vec<Value> = cx
            .state
            .doc
            .images
            .iter()
            .map(|i| {
                let (columns, rows) = ops::grid(i.size[0], i.size[1], i.tile_px, i.padding);
                json!({
                    "id": i.id,
                    "name": i.name,
                    "role": i.role,
                    "size": i.size,
                    "tile_px": i.tile_px,
                    "padding": i.padding,
                    "columns": columns,
                    "rows": rows,
                    "source_path": i.source_path,
                    "missing": i.source_path.as_ref().is_some_and(|p| !Path::new(p).is_file()),
                    "watch": i.watch,
                })
            })
            .collect();
        Ok(json!({ "images": list }))
    }
}
