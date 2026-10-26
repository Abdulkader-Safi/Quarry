//! Tile commands (FR-TILE-01 to 05, 09). Tiles are quads in a mesh object
//! ("Tiles" unless named), one per 1 m cell on the placement plane, sharing
//! vertices with their neighbours.

use super::object;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{
    Axis, Brush, Corner, Document, Face, Image, ImageRole, Influences, Mesh, Object, ObjectKind,
};
use glam::{Mat4, Vec2, Vec3, Vec4};
use quarry_ops::tile::{self as ops, Orient};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const SAME: f32 = 1e-4;
const DEFAULT_OBJECT: &str = "Tiles";

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<TileImportTileset>();
    r.register::<TileBrush>();
    r.register::<TilePlane>();
    r.register::<TilePlace>();
    r.register::<TileErase>();
    r.register::<TileFillRect>();
    r.register::<TileEraseRect>();
    r.register::<TileEyedrop>();
}

pub(super) fn tileset<'a>(doc: &'a Document, reference: &str) -> Result<&'a Image, CommandError> {
    doc.images
        .find(reference)
        .filter(|i| i.role == ImageRole::Tileset)
        .ok_or_else(|| {
            CommandError::not_found(format!(
                "No tileset called '{reference}'. Call query.images, or tile.import_tileset first."
            ))
        })
}

/// Insert an image read from `path` (shared with art.import).
pub(super) fn add_image(
    cx: &mut Context,
    path: PathBuf,
    role: ImageRole,
    name: Option<String>,
    tile_px: u32,
    padding: u32,
) -> Result<Id<Image>, CommandError> {
    let loaded = cx.storage()?.read_image(&path)?;
    let (cols, rows) = ops::grid(loaded.size[0], loaded.size[1], tile_px, padding);
    if role == ImageRole::Tileset && (cols == 0 || rows == 0) {
        return Err(CommandError::new(
            ErrorCode::OutOfBounds,
            format!(
                "The image is {}x{} px, smaller than one {tile_px} px tile. Pick a smaller tile_px.",
                loaded.size[0], loaded.size[1]
            ),
        ));
    }
    let doc = cx.doc();
    let base = name.unwrap_or_else(|| {
        path.file_stem()
            .map_or("image".into(), |s| s.to_string_lossy().into_owned())
    });
    if base.trim().is_empty() {
        return Err(CommandError::invalid("An image name cannot be empty."));
    }
    let name = doc.images.unique_name(&base);
    let filter = doc.settings.filter;
    Ok(doc.images.insert(Image {
        id: Id::new(0, 0),
        name,
        role,
        size: loaded.size,
        tile_px,
        padding,
        zip_path: String::new(),
        source_path: Some(path.to_string_lossy().into_owned()),
        source_hash: loaded.hash,
        filter,
        watch: true,
        pixels: Arc::from(loaded.pixels),
    }))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TileImportTileset {
    /// PNG or PSD (the flattened image is used).
    pub path: PathBuf,
    /// Tile size in pixels. Default the project's tile_px (16).
    pub tile_px: Option<u32>,
    /// Pixels between tiles and around the sheet. Default 0.
    pub padding: Option<u32>,
    /// Default the file name, made unique.
    pub name: Option<String>,
}

impl Command for TileImportTileset {
    const NAME: &'static str = "tile.import_tileset";
    const DESCRIPTION: &'static str = "Import a PNG or PSD as a tileset. It is copied into the project and stays linked to the file for live refresh. The brush is set to its first tile.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let tile_px = self.tile_px.unwrap_or(cx.state.doc.settings.tile_px);
        if !(1..=1024).contains(&tile_px) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "tile_px must be between 1 and 1024.",
            ));
        }
        let padding = self.padding.unwrap_or(0);
        let id = add_image(
            cx,
            self.path,
            ImageRole::Tileset,
            self.name,
            tile_px,
            padding,
        )?;
        let doc = cx.doc();
        let image = doc.images.get(id).expect("inserted");
        let (cols, rows) = ops::grid(image.size[0], image.size[1], tile_px, padding);
        let name = image.name.clone();
        doc.view.tile.brush = Some(Brush {
            tileset: id,
            col: 0,
            row: 0,
            width: 1,
            height: 1,
            rotate: 0,
            mirror: false,
            flip: false,
        });
        Ok(json!({ "id": id, "name": name, "columns": cols, "rows": rows }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TileBrush {
    /// Tileset ID or name. Default the current brush's tileset.
    pub tileset: Option<String>,
    /// Tile number, counting left to right then top to bottom from 0. Or give col and row.
    pub tile: Option<u32>,
    pub col: Option<u32>,
    pub row: Option<u32>,
    /// Brush size in tiles (a rectangle of the tileset). Default 1 x 1.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Counter-clockwise: 0, 90, 180 or 270.
    pub rotate: Option<u32>,
    /// Mirror left-right.
    pub mirror: Option<bool>,
    /// Flip top-bottom.
    pub flip: Option<bool>,
}

impl Command for TileBrush {
    const NAME: &'static str = "tile.brush";
    const DESCRIPTION: &'static str = "Pick the tile brush: one tile or a rectangle of tiles from a tileset, with rotation, mirror and flip.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let current = doc.view.tile.brush;
        let image = match (&self.tileset, current) {
            (Some(reference), _) => tileset(doc, reference)?,
            (None, Some(b)) => doc.images.get(b.tileset).ok_or_else(|| {
                CommandError::not_found("The brush's tileset was removed. Name a tileset.")
            })?,
            (None, None) => return Err(CommandError::invalid("Name a tileset for the brush.")),
        };
        let (cols, rows) = ops::grid(image.size[0], image.size[1], image.tile_px, image.padding);
        let keep = current.filter(|b| b.tileset == image.id);
        let (col, row) = match (self.tile, self.col, self.row) {
            (Some(t), _, _) => (t % cols, t / cols),
            (None, Some(c), Some(r)) => (c, r),
            (None, None, None) => keep.map_or((0, 0), |b| (b.col, b.row)),
            _ => return Err(CommandError::invalid("Give both col and row, or tile.")),
        };
        let width = self.width.or(keep.map(|b| b.width)).unwrap_or(1).max(1);
        let height = self.height.or(keep.map(|b| b.height)).unwrap_or(1).max(1);
        if col + width > cols || row + height > rows {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!("That brush runs off the tileset, which is {cols} x {rows} tiles."),
            ));
        }
        let rotate = match self.rotate {
            None => keep.map_or(0, |b| b.rotate),
            Some(d @ (0 | 90 | 180 | 270)) => (d / 90) as u8,
            Some(_) => return Err(CommandError::invalid("rotate must be 0, 90, 180 or 270.")),
        };
        let brush = Brush {
            tileset: image.id,
            col,
            row,
            width,
            height,
            rotate,
            mirror: self.mirror.or(keep.map(|b| b.mirror)).unwrap_or(false),
            flip: self.flip.or(keep.map(|b| b.flip)).unwrap_or(false),
        };
        cx.doc().view.tile.brush = Some(brush);
        Ok(json!({ "brush": brush }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TilePlane {
    /// The plane's normal: y is the floor, x and z are walls.
    pub axis: Axis,
    /// Tilt in degrees about the plane's first axis, -89 to 89. Default 0.
    pub tilt: Option<f32>,
}

impl Command for TilePlane {
    const NAME: &'static str = "tile.plane";
    const DESCRIPTION: &'static str = "Lock the tile placement plane to the X, Y (floor) or Z axis through the 3D crosshair, optionally tilted.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let tilt = self.tilt.unwrap_or(0.0);
        if !(tilt.is_finite() && tilt.abs() < 90.0) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "tilt must be between -89 and 89 degrees.",
            ));
        }
        let tool = &mut cx.doc().view.tile;
        tool.axis = self.axis;
        tool.tilt = tilt;
        Ok(json!({ "axis": self.axis, "tilt": tilt }))
    }
}

pub(super) fn plane_frame(doc: &Document) -> ops::Frame {
    let tool = doc.view.tile;
    let axis = match tool.axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    ops::frame(doc.view.cursor, axis, tool.tilt)
}

fn brush(doc: &Document) -> Result<(Brush, &Image), CommandError> {
    let brush = doc.view.tile.brush.ok_or_else(|| {
        CommandError::new(
            ErrorCode::NothingSelected,
            "No tile brush yet. Call tile.brush first.",
        )
    })?;
    let image = doc.images.get(brush.tileset).ok_or_else(|| {
        CommandError::not_found(
            "The brush's tileset was removed. Call tile.brush with another tileset.",
        )
    })?;
    Ok((brush, image))
}

fn orient(b: &Brush) -> Orient {
    Orient {
        rotate: b.rotate,
        mirror: b.mirror,
        flip: b.flip,
    }
}

/// The tiles object to edit (created on first use) and its world inverse.
fn target(doc: &mut Document, name: Option<&str>) -> Result<(Id<Object>, Mat4), CommandError> {
    let name = name.unwrap_or(DEFAULT_OBJECT);
    let id = match doc.objects.find(name) {
        Some(o) if o.kind != ObjectKind::Mesh || o.instance_of.is_some() => {
            return Err(CommandError::new(
                ErrorCode::Conflict,
                format!("'{name}' is not a mesh object, so tiles cannot go in it. Name another."),
            ));
        }
        Some(o) => o.id,
        None => {
            let mut object = Object::new(name, ObjectKind::Mesh);
            object.mesh = Some(Arc::new(Mesh::default()));
            doc.objects.insert(object)
        }
    };
    let object = doc.objects.get(id).expect("found or made");
    Ok((id, doc.world_matrix(object).inverse()))
}

fn mesh_of(doc: &mut Document, id: Id<Object>) -> &mut Mesh {
    let object = doc.objects.get_mut(id).expect("target exists");
    Arc::make_mut(object.mesh.get_or_insert_with(Default::default))
}

/// The face sitting exactly on these four corners, if any.
fn face_at(mesh: &Mesh, corners: &[Vec3; 4]) -> Option<usize> {
    mesh.faces.iter().position(|f| {
        f.corners.len() == 4
            && corners.iter().all(|c| {
                f.corners
                    .iter()
                    .any(|fc| mesh.positions[fc.vertex as usize].distance(*c) < SAME)
            })
    })
}

fn vertex(mesh: &mut Mesh, p: Vec3) -> u32 {
    // ponytail: linear search for a shared vertex; a spatial hash when tile
    // objects reach tens of thousands of vertices.
    if let Some(i) = mesh.positions.iter().position(|q| q.distance(p) < SAME) {
        return i as u32;
    }
    mesh.positions.push(p);
    if !mesh.weights.is_empty() {
        mesh.weights.push(Influences::default());
    }
    (mesh.positions.len() - 1) as u32
}

/// Place one tile: replace the face already on that cell, or add a new quad.
fn put(mesh: &mut Mesh, corners: [Vec3; 4], uvs: [Vec2; 4], image: Id<Image>, tile: u32) {
    let face_corners: Vec<Corner> = (0..4)
        .map(|k| Corner {
            vertex: vertex(mesh, corners[k]),
            uv: uvs[k],
            color: Vec4::ONE,
        })
        .collect();
    let face = Face {
        corners: face_corners,
        image: Some(image),
        smooth_group: 0,
        tile: Some(tile),
        hidden: false,
    };
    match face_at(mesh, &corners) {
        Some(i) => mesh.faces[i] = face,
        None => mesh.faces.push(face),
    }
}

/// Drop vertices no face uses any more, keeping indices consistent.
pub(super) fn compact(mesh: &mut Mesh) {
    let mut used = vec![false; mesh.positions.len()];
    for c in mesh.faces.iter().flat_map(|f| &f.corners) {
        used[c.vertex as usize] = true;
    }
    let mut remap = vec![0u32; used.len()];
    let mut next = 0u32;
    for (i, keep) in used.iter().enumerate() {
        remap[i] = next;
        next += *keep as u32;
    }
    let mut i = 0;
    mesh.positions.retain(|_| {
        i += 1;
        used[i - 1]
    });
    if !mesh.weights.is_empty() {
        let mut i = 0;
        mesh.weights.retain(|_| {
            i += 1;
            used[i - 1]
        });
    }
    for c in mesh.faces.iter_mut().flat_map(|f| &mut f.corners) {
        c.vertex = remap[c.vertex as usize];
    }
}

/// Stamp the brush with its first cell at `at`. Returns tiles placed.
fn stamp_brush(
    doc: &mut Document,
    id: Id<Object>,
    inverse: Mat4,
    cells: &[(i32, i32)],
) -> Result<usize, CommandError> {
    let (b, image) = brush(doc)?;
    let (tile_px, padding, size) = (image.tile_px, image.padding, image.size);
    let (cols, _) = ops::grid(size[0], size[1], tile_px, padding);
    let plane = plane_frame(doc);
    let layout = ops::stamp(b.width, b.height, orient(&b));
    let mesh = mesh_of(doc, id);
    let mut placed = 0;
    for &(i, j) in cells {
        for &((di, dj), (tx, ty)) in &layout {
            let (col, row) = (b.col + tx, b.row + ty);
            let corners = plane
                .cell(i + di, j + dj)
                .map(|p| inverse.transform_point3(p));
            let uvs = ops::tile_uvs(ops::tile_rect(col, row, tile_px, padding), orient(&b));
            put(mesh, corners, uvs, b.tileset, row * cols + col);
            placed += 1;
        }
    }
    Ok(placed)
}

fn erase_cells(doc: &mut Document, id: Id<Object>, inverse: Mat4, cells: &[(i32, i32)]) -> usize {
    let plane = plane_frame(doc);
    let targets: Vec<[Vec3; 4]> = cells
        .iter()
        .map(|&(i, j)| plane.cell(i, j).map(|p| inverse.transform_point3(p)))
        .collect();
    let mesh = doc.objects.get(id).and_then(|o| o.mesh.as_deref());
    let hits: Vec<usize> = mesh.map_or(Vec::new(), |m| {
        targets.iter().filter_map(|c| face_at(m, c)).collect()
    });
    if hits.is_empty() {
        return 0; // nothing to change, so nothing enters history
    }
    let mesh = mesh_of(doc, id);
    let mut index = 0;
    mesh.faces.retain(|_| {
        index += 1;
        !hits.contains(&(index - 1))
    });
    compact(mesh);
    hits.len()
}

fn cells(list: Option<Vec<[i32; 2]>>) -> Vec<(i32, i32)> {
    list.unwrap_or_else(|| vec![[0, 0]])
        .into_iter()
        .map(|[i, j]| (i, j))
        .collect()
}

fn rect(a: [i32; 2], b: [i32; 2]) -> Result<Vec<(i32, i32)>, CommandError> {
    let (i0, i1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (j0, j1) = (a[1].min(b[1]), a[1].max(b[1]));
    if (i1 - i0 + 1) as i64 * (j1 - j0 + 1) as i64 > 65_536 {
        return Err(CommandError::new(
            ErrorCode::OutOfBounds,
            "That rectangle is over 65,536 cells. Fill it in parts.",
        ));
    }
    Ok((j0..=j1)
        .flat_map(|j| (i0..=i1).map(move |i| (i, j)))
        .collect())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TilePlace {
    /// Cells [i, j] on the placement plane; [0, 0] is at the 3D crosshair.
    /// Default one stamp at the crosshair. A drag is one call with many cells.
    pub cells: Option<Vec<[i32; 2]>>,
    /// Mesh object to add tiles to. Default "Tiles", created if missing.
    pub object: Option<String>,
}

impl Command for TilePlace {
    const NAME: &'static str = "tile.place";
    const DESCRIPTION: &'static str = "Place the brush on cells of the placement plane (1 m each, [0, 0] at the crosshair). A tile already on a cell is replaced. One call is one undo step.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let cells = cells(self.cells);
        if cells.len() > 65_536 {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "Place at most 65,536 cells per call.",
            ));
        }
        let doc = cx.doc();
        brush(doc)?;
        let (id, inverse) = target(doc, self.object.as_deref())?;
        let placed = stamp_brush(doc, id, inverse, &cells)?;
        Ok(json!({ "object": id, "placed": placed }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TileErase {
    /// Cells [i, j] to clear. Default the crosshair's cell.
    pub cells: Option<Vec<[i32; 2]>>,
    pub object: Option<String>,
}

impl Command for TileErase {
    const NAME: &'static str = "tile.erase";
    const DESCRIPTION: &'static str = "Remove the tiles on cells of the placement plane.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = object(doc, self.object.as_deref().unwrap_or(DEFAULT_OBJECT))?;
        let inverse = doc
            .world_matrix(doc.objects.get(id).expect("resolved"))
            .inverse();
        let erased = erase_cells(doc, id, inverse, &cells(self.cells));
        Ok(json!({ "erased": erased }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TileFillRect {
    /// One corner cell [i, j].
    pub a: [i32; 2],
    /// The opposite corner cell [i, j].
    pub b: [i32; 2],
    pub object: Option<String>,
}

impl Command for TileFillRect {
    const NAME: &'static str = "tile.fill_rect";
    const DESCRIPTION: &'static str =
        "Fill a rectangle of cells with the brush, repeating a multi-tile brush as a pattern.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let all = rect(self.a, self.b)?;
        let doc = cx.doc();
        let (b, _) = brush(doc)?;
        let layout = ops::stamp(b.width, b.height, orient(&b));
        let w = layout.iter().map(|((x, _), _)| x + 1).max().unwrap_or(1);
        let h = layout.iter().map(|((_, y), _)| y + 1).max().unwrap_or(1);
        let (i0, j0) = (self.a[0].min(self.b[0]), self.a[1].min(self.b[1]));
        let (id, inverse) = target(doc, self.object.as_deref())?;
        // Stamp whole brushes on a grid from the rectangle's corner, then
        // clear what spilled past its far edges.
        let starts: Vec<(i32, i32)> = all
            .iter()
            .filter(|(i, j)| (i - i0) % w == 0 && (j - j0) % h == 0)
            .copied()
            .collect();
        stamp_brush(doc, id, inverse, &starts)?;
        let (i1, j1) = (self.a[0].max(self.b[0]), self.a[1].max(self.b[1]));
        let spill: Vec<(i32, i32)> = starts
            .iter()
            .flat_map(|&(i, j)| layout.iter().map(move |((di, dj), _)| (i + di, j + dj)))
            .filter(|&(i, j)| i > i1 || j > j1)
            .collect();
        erase_cells(doc, id, inverse, &spill);
        Ok(json!({ "object": id, "filled": all.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TileEraseRect {
    pub a: [i32; 2],
    pub b: [i32; 2],
    pub object: Option<String>,
}

impl Command for TileEraseRect {
    const NAME: &'static str = "tile.erase_rect";
    const DESCRIPTION: &'static str =
        "Remove every tile in a rectangle of cells on the placement plane.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let all = rect(self.a, self.b)?;
        let doc = cx.doc();
        let id = object(doc, self.object.as_deref().unwrap_or(DEFAULT_OBJECT))?;
        let inverse = doc
            .world_matrix(doc.objects.get(id).expect("resolved"))
            .inverse();
        let erased = erase_cells(doc, id, inverse, &all);
        Ok(json!({ "erased": erased }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TileEyedrop {
    /// Object ID or name.
    pub object: String,
    /// Face index in that object's mesh.
    pub face: usize,
}

impl Command for TileEyedrop {
    const NAME: &'static str = "tile.eyedrop";
    const DESCRIPTION: &'static str =
        "Copy a placed tile, with its rotation, mirror and flip, into the brush.";
    const UNDOABLE: bool = false;

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = &cx.state.doc;
        let id = object(doc, &self.object)?;
        let o = doc.objects.get(id).expect("resolved");
        let face = doc
            .drawn_mesh(o)
            .and_then(|m| m.faces.get(self.face))
            .ok_or_else(|| {
                CommandError::new(
                    ErrorCode::OutOfBounds,
                    format!("'{}' has no face {}.", o.name, self.face),
                )
            })?;
        let not_tile =
            || CommandError::invalid("That face is not a tile. Pick a face placed from a tileset.");
        let image = face
            .image
            .and_then(|i| doc.images.get(i))
            .filter(|i| i.role == ImageRole::Tileset)
            .ok_or_else(not_tile)?;
        if face.corners.len() != 4 {
            return Err(not_tile());
        }
        let uvs = [0, 1, 2, 3].map(|k| face.corners[k].uv);
        let min = uvs.iter().fold(Vec2::splat(f32::MAX), |a, b| a.min(*b));
        let step = (image.tile_px + image.padding) as f32;
        let col = ((min.x - image.padding as f32) / step).round().max(0.0) as u32;
        let row = ((min.y - image.padding as f32) / step).round().max(0.0) as u32;
        let rect = ops::tile_rect(col, row, image.tile_px, image.padding);
        let o = ops::orient_of(rect, uvs).ok_or_else(not_tile)?;
        let brush = Brush {
            tileset: image.id,
            col,
            row,
            width: 1,
            height: 1,
            rotate: o.rotate,
            mirror: o.mirror,
            flip: o.flip,
        };
        cx.doc().view.tile.brush = Some(brush);
        Ok(json!({ "brush": brush }))
    }
}
