//! Painting (FR-PAINT-01 to 04, 06). Agents paint by pixel lists; the 2D panel
//! and 3D brush send the same commands. Each call is one undo step.

use super::select::edit_mesh;
use crate::command::{Command, CommandError, CommandRegistry, CommandResult, Context, ErrorCode};
use crate::id::Id;
use crate::model::{
    Document, Filter, Image, ImageRole, MAX_PALETTE_COLORS, Mesh, Palette, SelectMode,
};
use glam::{Vec2, Vec3};
use quarry_ops::paint::{Canvas, nearest, pressure_size};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

const MAX_PIXELS: usize = 65_536;
const MAX_SIDE: u32 = 4096;

pub(super) fn register(r: &mut CommandRegistry) {
    r.register::<PaintTextureNew>();
    r.register::<PaintPixels>();
    r.register::<PaintRect>();
    r.register::<PaintFill>();
    r.register::<PaintStroke3d>();
    r.register::<PaintPaletteSet>();
    r.register::<PaintPaletteLock>();
}

fn image_id(doc: &Document, reference: &str) -> Result<Id<Image>, CommandError> {
    doc.images.find(reference).map(|i| i.id).ok_or_else(|| {
        CommandError::not_found(format!(
            "No image called '{reference}'. Call query.images to list them."
        ))
    })
}

/// The colour as it will land: snapped to the active palette when locked.
fn ink(doc: &Document, rgba: [u8; 4]) -> [u8; 4] {
    let s = &doc.settings;
    let palette = s
        .active_palette
        .as_ref()
        .and_then(|name| doc.palettes.iter().find(|p| &p.name == name))
        .or(doc.palettes.first());
    match palette {
        Some(p) if s.palette_lock => nearest(rgba, &p.colors),
        _ => rgba,
    }
}

/// Paint into an image's pixels; copies them first if a snapshot shares them.
fn with_canvas<R>(doc: &mut Document, id: Id<Image>, paint: impl FnOnce(&mut Canvas) -> R) -> R {
    let image = doc.images.get_mut(id).expect("resolved");
    let (width, height) = (image.size[0], image.size[1]);
    let mut canvas = Canvas {
        pixels: Arc::make_mut(&mut image.pixels),
        width,
        height,
    };
    paint(&mut canvas)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintTextureNew {
    pub width: u32,
    pub height: u32,
    /// Fill colour [r, g, b, a]. Default opaque white.
    pub color: Option<[u8; 4]>,
    pub name: Option<String>,
}

impl Command for PaintTextureNew {
    const NAME: &'static str = "paint.texture_new";
    const DESCRIPTION: &'static str = "Create a blank texture and put it on the selected faces of the edited mesh (all its faces when none are selected). Their UVs are rescaled so the whole texture shows.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if !(1..=MAX_SIDE).contains(&self.width) || !(1..=MAX_SIDE).contains(&self.height) {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!("width and height must be 1 to {MAX_SIDE} pixels."),
            ));
        }
        let doc = cx.doc();
        let (o, mesh) = edit_mesh(doc)?;
        let object = o.id;
        let faces: Vec<usize> =
            if doc.selection.mode == SelectMode::Face && !doc.selection.faces.is_empty() {
                doc.selection.faces.iter().map(|f| *f as usize).collect()
            } else {
                (0..mesh.faces.len()).collect()
            };
        let old_sizes: Vec<Vec2> = faces
            .iter()
            .map(|f| {
                mesh.faces[*f]
                    .image
                    .and_then(|i| doc.images.get(i))
                    .map_or(Vec2::splat(doc.settings.tile_px as f32), |i| {
                        Vec2::new(i.size[0] as f32, i.size[1] as f32)
                    })
            })
            .collect();
        let color = self.color.unwrap_or([255; 4]);
        let pixels: Vec<u8> = color.repeat((self.width * self.height) as usize);
        let base = self.name.unwrap_or_else(|| "texture".into());
        let id = doc.images.insert(Image {
            id: Id::new(0, 0),
            name: doc.images.unique_name(&base),
            role: ImageRole::Texture,
            size: [self.width, self.height],
            tile_px: doc.settings.tile_px,
            padding: 0,
            zip_path: String::new(),
            source_path: None,
            source_hash: 0,
            filter: Filter::Nearest,
            watch: true,
            pixels: Arc::from(pixels),
        });
        let new_size = Vec2::new(self.width as f32, self.height as f32);
        let mesh = Arc::make_mut(
            doc.objects
                .get_mut(object)
                .expect("edited")
                .mesh
                .as_mut()
                .expect("mesh"),
        );
        for (f, old) in faces.iter().zip(old_sizes) {
            let face = &mut mesh.faces[*f];
            face.image = Some(id);
            for c in &mut face.corners {
                c.uv = c.uv / old * new_size;
            }
        }
        let name = doc.images.get(id).expect("inserted").name.clone();
        Ok(json!({ "id": id, "name": name, "faces": faces.len() }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Pixel {
    pub x: u32,
    pub y: u32,
    /// [r, g, b, a]; a = 0 erases.
    pub rgba: [u8; 4],
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintPixels {
    /// Image ID or name.
    pub image: String,
    /// Up to 65,536 pixels, x right and y down from the top-left.
    pub pixels: Vec<Pixel>,
}

impl Command for PaintPixels {
    const NAME: &'static str = "paint.pixels";
    const DESCRIPTION: &'static str = "Set individual pixels of an image (pencil and eraser). Colours snap to the palette when palette lock is on.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if self.pixels.len() > MAX_PIXELS {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "Send at most 65,536 pixels per call.",
            ));
        }
        let doc = cx.doc();
        let id = image_id(doc, &self.image)?;
        let size = doc.images.get(id).expect("resolved").size;
        if let Some(p) = self
            .pixels
            .iter()
            .find(|p| p.x >= size[0] || p.y >= size[1])
        {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!(
                    "Pixel ({}, {}) is outside the {}x{} image.",
                    p.x, p.y, size[0], size[1]
                ),
            ));
        }
        let inks: Vec<[u8; 4]> = self.pixels.iter().map(|p| ink(doc, p.rgba)).collect();
        let changed = with_canvas(doc, id, |c| {
            self.pixels
                .iter()
                .zip(inks)
                .filter(|(p, ink)| c.set(p.x as i64, p.y as i64, *ink))
                .count()
        });
        Ok(json!({ "changed": changed }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintRect {
    pub image: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: [u8; 4],
}

impl Command for PaintRect {
    const NAME: &'static str = "paint.rect";
    const DESCRIPTION: &'static str =
        "Fill a rectangle of an image with one colour (clipped to the image).";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = image_id(doc, &self.image)?;
        let c = ink(doc, self.rgba);
        let changed = with_canvas(doc, id, |canvas| {
            canvas.rect(
                self.x as i64,
                self.y as i64,
                self.width.min(MAX_SIDE),
                self.height.min(MAX_SIDE),
                c,
            )
        });
        Ok(json!({ "changed": changed }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintFill {
    pub image: String,
    pub x: u32,
    pub y: u32,
    pub rgba: [u8; 4],
}

impl Command for PaintFill {
    const NAME: &'static str = "paint.fill";
    const DESCRIPTION: &'static str =
        "Flood-fill the area of exactly the same colour around a pixel.";

    fn run(self, cx: &mut Context) -> CommandResult {
        let doc = cx.doc();
        let id = image_id(doc, &self.image)?;
        let size = doc.images.get(id).expect("resolved").size;
        if self.x >= size[0] || self.y >= size[1] {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!(
                    "({}, {}) is outside the {}x{} image.",
                    self.x, self.y, size[0], size[1]
                ),
            ));
        }
        let c = ink(doc, self.rgba);
        let changed = with_canvas(doc, id, |canvas| {
            canvas.fill(self.x as i64, self.y as i64, c)
        });
        Ok(json!({ "changed": changed }))
    }
}

/// The face point nearest `p` (local space): (face, texel uv in pixels).
fn hit(mesh: &Mesh, p: Vec3) -> Option<(usize, Vec2, Id<Image>)> {
    let mut best: Option<(f32, usize, Vec2, Id<Image>)> = None;
    for (f, face) in mesh.faces.iter().enumerate() {
        let Some(image) = face.image else { continue };
        let pos: Vec<Vec3> = face
            .corners
            .iter()
            .map(|c| mesh.positions[c.vertex as usize])
            .collect();
        for k in 1..pos.len() - 1 {
            let (a, b, c) = (pos[0], pos[k], pos[k + 1]);
            let (q, w) = closest_on_triangle(p, a, b, c);
            let d = q.distance_squared(p);
            if best.as_ref().is_none_or(|(bd, ..)| d < *bd) {
                let uv = face.corners[0].uv * w.x
                    + face.corners[k].uv * w.y
                    + face.corners[k + 1].uv * w.z;
                best = Some((d, f, uv, image));
            }
        }
    }
    best.map(|(_, f, uv, image)| (f, uv, image))
}

/// Closest point on triangle abc to p, with its barycentric weights.
fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (Vec3, Vec3) {
    // Ericson, Real-Time Collision Detection 5.1.5.
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, Vec3::X);
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, Vec3::Y);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (a + ab * v, Vec3::new(1.0 - v, v, 0.0));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, Vec3::Z);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (a + ac * w, Vec3::new(1.0 - w, 0.0, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (b + (c - b) * w, Vec3::new(0.0, 1.0 - w, w));
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    (a + ab * v + ac * w, Vec3::new(1.0 - v - w, v, w))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintStroke3d {
    /// World points along the stroke [[x, y, z], ...], on or near the edited mesh.
    pub points: Vec<[f32; 3]>,
    pub rgba: [u8; 4],
    /// Brush size in whole pixels. Default 1.
    pub size: Option<u32>,
    /// Pen pressure 0..1 per point; the brush grows from 1 px to `size`.
    pub pressures: Option<Vec<f32>>,
}

impl Command for PaintStroke3d {
    const NAME: &'static str = "paint.stroke_3d";
    const DESCRIPTION: &'static str = "Paint a stroke on the edited mesh: each point lands on the texel under it, and the line stays continuous across UV seams. One undo step.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if self.points.is_empty() || self.points.len() > 10_000 {
            return Err(CommandError::invalid("Send 1 to 10,000 points."));
        }
        let max = self.size.unwrap_or(1).clamp(1, 64);
        let doc = cx.doc();
        let c = ink(doc, self.rgba);
        let (o, mesh) = edit_mesh(doc)?;
        let to_local = doc.world_matrix(o).inverse();
        let mesh = mesh.clone();
        // Walk the stroke in steps of a quarter texel so seams never leave gaps.
        let step = 0.25 / doc.settings.tile_px as f32;
        let points: Vec<Vec3> = self
            .points
            .iter()
            .map(|p| to_local.transform_point3(Vec3::from(*p)))
            .collect();
        let pressures = self.pressures.unwrap_or_default();
        let mut samples: Vec<(Vec3, u32)> = Vec::new();
        for (i, p) in points.iter().enumerate() {
            let size = pressure_size(pressures.get(i).copied(), max);
            match points.get(i + 1) {
                Some(next) => {
                    let n = ((next.distance(*p) / step).ceil() as usize).clamp(1, 4096);
                    samples.extend((0..n).map(|k| (p.lerp(*next, k as f32 / n as f32), size)));
                }
                None => samples.push((*p, size)),
            }
        }
        let mut changed = 0;
        let mut last: Option<(Id<Image>, i64, i64)> = None;
        for (p, size) in samples {
            let Some((_, uv, image)) = hit(&mesh, p) else {
                continue;
            };
            let (x, y) = (uv.x.floor() as i64, uv.y.floor() as i64);
            if last == Some((image, x, y)) {
                continue;
            }
            last = Some((image, x, y));
            changed += with_canvas(doc, image, |canvas| canvas.stamp(x, y, size, c));
        }
        if changed == 0 && mesh.faces.iter().all(|f| f.image.is_none()) {
            return Err(CommandError::invalid(
                "The mesh has no texture to paint. Call paint.texture_new first.",
            ));
        }
        Ok(json!({ "changed": changed }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintPaletteSet {
    pub name: String,
    /// Up to 256 colours [r, g, b, a].
    pub colors: Vec<[u8; 4]>,
}

impl Command for PaintPaletteSet {
    const NAME: &'static str = "paint.palette_set";
    const DESCRIPTION: &'static str =
        "Create or replace a named palette and make it the one painting uses.";

    fn run(self, cx: &mut Context) -> CommandResult {
        if self.colors.is_empty() || self.colors.len() > MAX_PALETTE_COLORS {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                "A palette has 1 to 256 colours.",
            ));
        }
        let doc = cx.doc();
        let palette = Palette {
            name: self.name.clone(),
            colors: self.colors,
            weights: None,
        };
        match doc.palettes.iter_mut().find(|p| p.name == self.name) {
            Some(p) => *p = palette,
            None => doc.palettes.push(palette),
        }
        doc.settings.active_palette = Some(self.name.clone());
        Ok(json!({ "palette": self.name }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaintPaletteLock {
    pub lock: bool,
}

impl Command for PaintPaletteLock {
    const NAME: &'static str = "paint.palette_lock";
    const DESCRIPTION: &'static str = "Turn palette lock on or off: when on, every painted colour snaps to the nearest colour in the active palette.";

    fn run(self, cx: &mut Context) -> CommandResult {
        cx.doc().settings.palette_lock = self.lock;
        Ok(json!({ "palette_lock": self.lock }))
    }
}
