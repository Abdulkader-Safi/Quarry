//! The `.quarry` file (FR-PROJ-01, FR-PROJ-02): a zip holding
//!
//! ```text
//! project.json          the document minus mesh arrays and pixels
//! meshes/<object>.bin   positions, weights, faces (little-endian, versioned)
//! images/<image>.png    stored, not deflated
//! ```
//!
//! Saves are atomic (temp file + rename). Reads cap entry count, entry size and
//! total size, reject paths that escape the archive, and never panic.

use crate::autosave;
use crate::settings::Dirs;
use quarry_core::command::{CommandError, Storage};
use quarry_core::model::{Axis, Corner, Document, FORMAT_VERSION, Face, Influences, Mesh};
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_ENTRIES: usize = 100_000;
const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 16_384;
const MESH_MAGIC: &[u8; 4] = b"QMSH";
/// 2 added seams after the faces; 1 is still read.
const MESH_VERSION: u32 = 2;

/// Renders a document to PNG bytes, or None when there is nothing to show.
pub type Thumbnailer = fn(&Document) -> Option<Vec<u8>>;

/// `project.open` / `project.save` storage for the command registry. With
/// `dirs` set, saves keep backups and clear the autosave they replace.
#[derive(Default)]
pub struct QuarryFiles {
    pub dirs: Option<Dirs>,
    /// Renders `thumbnail.png` (256x256) for the recent list. Lives in
    /// `quarry-render`, passed in so this crate needs no GPU.
    pub thumbnail: Option<Thumbnailer>,
}

impl Storage for QuarryFiles {
    fn open(&self, path: &Path) -> Result<Document, CommandError> {
        read_project(path)
    }

    fn save(&self, doc: &Document, path: &Path) -> Result<(), CommandError> {
        if let Some(dirs) = &self.dirs
            && let Err(why) = autosave::backup_before_save(dirs, path)
        {
            // A missing backup must not cost the user their save.
            tracing::warn!("{why}");
        }
        let thumbnail = self.thumbnail.and_then(|render| render(doc));
        write_project_with(doc, path, thumbnail.as_deref())?;
        if let Some(dirs) = &self.dirs {
            autosave::clear_autosave(dirs, Some(path));
            autosave::clear_autosave(dirs, None);
        }
        Ok(())
    }

    fn read_image(&self, path: &Path) -> Result<quarry_core::command::LoadedImage, CommandError> {
        crate::images::load_image(path)
    }

    fn export(
        &self,
        doc: &Document,
        request: &quarry_core::command::ExportRequest,
    ) -> Result<serde_json::Value, CommandError> {
        crate::export::export(doc, request)
    }

    fn import_gltf(&self, path: &Path) -> Result<Document, CommandError> {
        crate::import::import(path)
    }

    fn recover(&self, autosave: &Path) -> Result<(Document, Option<PathBuf>), CommandError> {
        let doc = read_project(autosave)?;
        let sidecar: Option<autosave::Sidecar> = std::fs::read(autosave.with_extension("json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        Ok((doc, sidecar.and_then(|s| s.project)))
    }
}

/// Write `doc` to `path` atomically: a crash mid-save leaves the old file intact.
pub fn write_project(doc: &Document, path: &Path) -> Result<(), CommandError> {
    write_project_with(doc, path, None)
}

/// [`write_project`] plus an optional `thumbnail.png`.
pub fn write_project_with(
    doc: &Document,
    path: &Path,
    thumbnail: Option<&[u8]>,
) -> Result<(), CommandError> {
    let failed = |why: String| {
        CommandError::io(format!(
            "Could not save {}: {why}. Check the folder exists and you can write to it.",
            path.display()
        ))
    };
    let bytes = encode_with(doc, thumbnail).map_err(failed)?;
    crate::settings::write_atomic(path, &bytes).map_err(|e| failed(e.to_string()))
}

/// Read a `.quarry` file. Damaged or hostile files return an error naming the file.
pub fn read_project(path: &Path) -> Result<Document, CommandError> {
    let bytes = std::fs::read(path)
        .map_err(|e| CommandError::io(format!("Could not open {}: {e}.", path.display())))?;
    decode(&bytes).map_err(|why| {
        CommandError::io(format!(
            "Could not read {}: {why}. The file may be damaged or not a Quarry project.",
            path.display()
        ))
    })
}

pub fn encode(doc: &Document) -> Result<Vec<u8>, String> {
    encode_with(doc, None)
}

/// The thumbnail for a recent-list entry, if the file has one.
pub fn read_thumbnail(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    let mut total = 0;
    read_entry(&mut zip, "thumbnail.png", &mut total)
        .ok()
        .flatten()
}

pub fn encode_with(doc: &Document, thumbnail: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let err = |e: &dyn std::fmt::Display| e.to_string();

    // project.json carries no mesh arrays and no pixels; those get their own entries.
    let mut json_doc = doc.clone();
    let ids: Vec<_> = json_doc.objects.ids().collect();
    for id in ids {
        let object = json_doc.objects.get_mut(id).expect("listed");
        if let Some(mesh) = object.mesh.take() {
            zip.start_file(format!("meshes/{id}.bin"), deflated)
                .map_err(|e| err(&e))?;
            zip.write_all(&encode_mesh(&mesh)).map_err(|e| err(&e))?;
        }
    }
    let image_ids: Vec<_> = json_doc.images.ids().collect();
    for id in image_ids {
        let image = json_doc.images.get_mut(id).expect("listed");
        image.zip_path = format!("images/{id}.png");
        let [w, h] = image.size;
        let rgba = image::RgbaImage::from_raw(w, h, image.pixels.to_vec()).ok_or_else(|| {
            format!(
                "image '{}' has the wrong number of pixels for {w}x{h}",
                image.name
            )
        })?;
        let mut png = Cursor::new(Vec::new());
        rgba.write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| err(&e))?;
        zip.start_file(image.zip_path.clone(), stored)
            .map_err(|e| err(&e))?;
        zip.write_all(png.get_ref()).map_err(|e| err(&e))?;
    }
    let json = serde_json::to_vec_pretty(&json_doc).map_err(|e| err(&e))?;
    if let Some(png) = thumbnail {
        zip.start_file("thumbnail.png", stored)
            .map_err(|e| err(&e))?;
        zip.write_all(png).map_err(|e| err(&e))?;
    }
    zip.start_file("project.json", deflated)
        .map_err(|e| err(&e))?;
    zip.write_all(&json).map_err(|e| err(&e))?;
    Ok(zip.finish().map_err(|e| err(&e))?.into_inner())
}

pub fn decode(bytes: &[u8]) -> Result<Document, String> {
    let mut zip =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| "it is not a zip archive".to_string())?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!("it has more than {MAX_ENTRIES} entries"));
    }
    let mut total = 0u64;
    let mut read = |zip: &mut ZipArchive<_>, name: &str| -> Result<Option<Vec<u8>>, String> {
        read_entry(zip, name, &mut total)
    };

    let json = read(&mut zip, "project.json")?.ok_or("project.json is missing")?;
    let mut value: serde_json::Value = serde_json::from_slice(&json)
        .map_err(|e| format!("project.json is not valid JSON ({e})"))?;
    let version = value["format_version"]
        .as_u64()
        .ok_or("project.json has no format_version")?;
    if version > FORMAT_VERSION as u64 {
        return Err(format!(
            "it was saved by a newer Quarry (format {version}, this one reads up to {FORMAT_VERSION}). Update Quarry to open it"
        ));
    }
    crate::migrate::run(&mut value, version as u32)?;
    let mut doc: Document = serde_json::from_value(value)
        .map_err(|e| format!("project.json does not match the project format ({e})"))?;

    let ids: Vec<_> = doc.objects.ids().collect();
    for id in ids {
        if let Some(bin) = read(&mut zip, &format!("meshes/{id}.bin"))? {
            let mesh = decode_mesh(&bin).map_err(|why| format!("meshes/{id}.bin {why}"))?;
            doc.objects.get_mut(id).expect("listed").mesh = Some(Arc::new(mesh));
        }
    }
    let image_ids: Vec<_> = doc.images.ids().collect();
    for id in image_ids {
        let name = format!("images/{id}.png");
        let png = read(&mut zip, &name)?.ok_or_else(|| format!("{name} is missing"))?;
        let mut reader = image::ImageReader::with_format(Cursor::new(png), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_IMAGE_SIDE);
        limits.max_image_height = Some(MAX_IMAGE_SIDE);
        reader.limits(limits);
        let rgba = reader
            .decode()
            .map_err(|e| format!("{name} is not a readable PNG ({e})"))?
            .into_rgba8();
        let image = doc.images.get_mut(id).expect("listed");
        image.size = [rgba.width(), rgba.height()];
        image.zip_path = name;
        image.pixels = Arc::from(rgba.into_raw());
    }
    Ok(doc)
}

/// Read one named entry, enforcing the size caps on the bytes actually inflated.
fn read_entry<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
    total: &mut u64,
) -> Result<Option<Vec<u8>>, String> {
    let entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(format!("{name} cannot be read ({e})")),
    };
    if entry.enclosed_name().is_none() {
        return Err(format!("{name} points outside the archive"));
    }
    if entry.size() > MAX_ENTRY_BYTES {
        return Err(format!("{name} is larger than 512 MB"));
    }
    let mut bytes = Vec::new();
    entry
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{name} cannot be read ({e})"))?;
    *total += bytes.len() as u64;
    if bytes.len() as u64 > MAX_ENTRY_BYTES || *total > MAX_TOTAL_BYTES {
        return Err(format!("{name} unpacks to more than the size limit"));
    }
    Ok(Some(bytes))
}

fn encode_mesh(mesh: &Mesh) -> Vec<u8> {
    let mut b = Vec::with_capacity(mesh.bytes() + 32);
    b.extend_from_slice(MESH_MAGIC);
    let u32s = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
    let f32s =
        |b: &mut Vec<u8>, v: &[f32]| v.iter().for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
    u32s(&mut b, MESH_VERSION);
    u32s(&mut b, mesh.positions.len() as u32);
    u32s(&mut b, mesh.weights.len() as u32);
    u32s(&mut b, mesh.faces.len() as u32);
    b.push(match mesh.mirror {
        None => 0,
        Some(Axis::X) => 1,
        Some(Axis::Y) => 2,
        Some(Axis::Z) => 3,
    });
    for p in &mesh.positions {
        f32s(&mut b, &p.to_array());
    }
    for w in &mesh.weights {
        w.bones
            .iter()
            .for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
        f32s(&mut b, &w.weights);
    }
    for f in &mesh.faces {
        b.extend_from_slice(&[f.corners.len() as u8, f.smooth_group, f.hidden as u8]);
        match f.image {
            Some(id) => {
                b.push(1);
                u32s(&mut b, id.slot);
                u32s(&mut b, id.generation);
            }
            None => b.push(0),
        }
        match f.tile {
            Some(t) => {
                b.push(1);
                u32s(&mut b, t);
            }
            None => b.push(0),
        }
        for c in &f.corners {
            u32s(&mut b, c.vertex);
            f32s(&mut b, &c.uv.to_array());
            f32s(&mut b, &c.color.to_array());
        }
    }
    u32s(&mut b, mesh.seams.len() as u32);
    for (a, c) in &mesh.seams {
        u32s(&mut b, *a);
        u32s(&mut b, *c);
    }
    b
}

/// Bounds-checked reader over a mesh `.bin`.
struct Bin<'a>(&'a [u8]);

impl Bin<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        if self.0.len() < N {
            return Err("ends early".into());
        }
        let (head, rest) = self.0.split_at(N);
        self.0 = rest;
        Ok(head.try_into().expect("split at N"))
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take()?))
    }
    fn f32s<const N: usize>(&mut self) -> Result<[f32; N], String> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }
    /// A count that cannot claim more items than bytes remain (allocation guard).
    fn count(&mut self, min_item_bytes: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_item_bytes) > self.0.len() {
            return Err("claims more data than it holds".into());
        }
        Ok(n)
    }
}

fn decode_mesh(bytes: &[u8]) -> Result<Mesh, String> {
    let mut r = Bin(bytes);
    if &r.take::<4>()? != MESH_MAGIC {
        return Err("is not a Quarry mesh".into());
    }
    let version = r.u32()?;
    if !(1..=MESH_VERSION).contains(&version) {
        return Err(format!(
            "has mesh format {version}, this Quarry reads {MESH_VERSION}"
        ));
    }
    let positions = r.count(12)?;
    let weights = r.count(0)?;
    let faces = r.count(0)?;
    let mirror = match r.u8()? {
        0 => None,
        1 => Some(Axis::X),
        2 => Some(Axis::Y),
        3 => Some(Axis::Z),
        _ => return Err("has an unknown mirror axis".into()),
    };
    let mut mesh = Mesh {
        mirror,
        ..Mesh::default()
    };
    for _ in 0..positions {
        mesh.positions.push(r.f32s::<3>()?.into());
    }
    if weights.saturating_mul(24) > r.0.len() || faces.saturating_mul(5) > r.0.len() {
        return Err("claims more data than it holds".into());
    }
    for _ in 0..weights {
        let bones = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
        mesh.weights.push(Influences {
            bones,
            weights: r.f32s()?,
        });
    }
    for _ in 0..faces {
        let [count, smooth_group, hidden] = r.take::<3>()?;
        if !(3..=4).contains(&count) {
            return Err(format!("has a face with {count} corners"));
        }
        let image = match r.u8()? {
            0 => None,
            _ => Some(quarry_core::id::Id::new(r.u32()?, r.u32()?)),
        };
        let tile = match r.u8()? {
            0 => None,
            _ => Some(r.u32()?),
        };
        let mut corners = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let vertex = r.u32()?;
            if vertex as usize >= positions {
                return Err("has a face pointing at a missing vertex".into());
            }
            corners.push(Corner {
                vertex,
                uv: r.f32s::<2>()?.into(),
                color: r.f32s::<4>()?.into(),
            });
        }
        mesh.faces.push(Face {
            corners,
            image,
            smooth_group,
            tile,
            hidden: hidden != 0,
        });
    }
    if version >= 2 {
        let seams = r.count(8)?;
        for _ in 0..seams {
            let (a, b) = (r.u32()?, r.u32()?);
            if a as usize >= positions || b as usize >= positions {
                return Err("has a seam on a missing vertex".into());
            }
            mesh.seams.insert((a.min(b), a.max(b)));
        }
    }
    if !r.0.is_empty() {
        return Err("has trailing data".into());
    }
    Ok(mesh)
}
