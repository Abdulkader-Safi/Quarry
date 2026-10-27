//! Static OBJ export (FR-EXP-08): world-space meshes with one material per
//! image, a `.mtl` and the PNGs beside the `.obj`. No skins or animation.

use quarry_core::command::ExportRequest;
use quarry_core::id::Id;
use quarry_core::model::{Document, Object};
use quarry_core::{CommandError, ErrorCode};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fmt::Write;

pub fn export(doc: &Document, request: &ExportRequest) -> Result<Value, CommandError> {
    let fail = |why: String| {
        CommandError::new(
            ErrorCode::Io,
            format!("Could not export to {}: {why}.", request.path.display()),
        )
    };
    let dir = request.path.parent().unwrap_or(std::path::Path::new("."));
    let stem = request
        .path
        .file_stem()
        .map_or("export".into(), |s| s.to_string_lossy().into_owned());
    // The roots and everything under them.
    let mut wanted: BTreeSet<Id<Object>> = request.objects.iter().copied().collect();
    loop {
        let more: Vec<Id<Object>> = doc
            .objects
            .iter()
            .filter(|o| o.parent.is_some_and(|p| wanted.contains(&p)) && !wanted.contains(&o.id))
            .map(|o| o.id)
            .collect();
        if more.is_empty() {
            break;
        }
        wanted.extend(more);
    }
    let mut obj = format!("# Quarry\nmtllib {stem}.mtl\n");
    let mut images = BTreeSet::new();
    let (mut v_base, mut t_base, mut n_base, mut faces) = (1usize, 1usize, 1usize, 0usize);
    for o in doc
        .objects
        .iter()
        .filter(|o| wanted.contains(&o.id) && o.visible)
    {
        let Some(mesh) = doc.drawn_mesh(o) else {
            continue;
        };
        let world = glam::Mat4::from_scale(glam::Vec3::splat(request.scale)) * doc.world_matrix(o);
        let points: Vec<glam::Vec3> = mesh
            .positions
            .iter()
            .map(|p| world.transform_point3(*p))
            .collect();
        let mirrored = world.determinant() < 0.0;
        let normals = mesh.corner_normals(&points);
        let _ = writeln!(obj, "o {}", o.name.replace(char::is_whitespace, "_"));
        for p in &points {
            let _ = writeln!(obj, "v {} {} {}", p.x, p.y, p.z);
        }
        let mut current = None;
        for (f, n) in mesh.faces.iter().zip(&normals).filter(|(f, _)| !f.hidden) {
            let image = f.image.and_then(|i| doc.images.get(i));
            let size = image.map_or(glam::Vec2::ONE, |i| {
                glam::Vec2::new(i.size[0].max(1) as f32, i.size[1].max(1) as f32)
            });
            let material = image.map_or("plain".to_string(), |i| material(&i.name));
            if current.as_ref() != Some(&material) {
                let _ = writeln!(obj, "usemtl {material}");
                current = Some(material);
            }
            if let Some(i) = image {
                images.insert(i.id);
            }
            let mut corners: Vec<String> = Vec::new();
            for (c, n) in f.corners.iter().zip(n) {
                // OBJ's v runs up the image; Quarry's pixel rows run down.
                let uv = c.uv / size;
                let n = if mirrored { -*n } else { *n };
                let _ = writeln!(obj, "vt {} {}", uv.x, 1.0 - uv.y);
                let _ = writeln!(obj, "vn {} {} {}", n.x, n.y, n.z);
                corners.push(format!(
                    "{}/{}/{}",
                    v_base + c.vertex as usize,
                    t_base,
                    n_base
                ));
                t_base += 1;
                n_base += 1;
            }
            if mirrored {
                corners[1..].reverse();
            }
            let _ = writeln!(obj, "f {}", corners.join(" "));
            faces += 1;
        }
        v_base += points.len();
    }
    let mut mtl = String::from("# Quarry\nnewmtl plain\nKd 1 1 1\n");
    let mut files = Vec::new();
    for id in &images {
        let image = doc.images.get(*id).expect("used");
        let name = material(&image.name);
        let png_name = format!("{stem}_{name}.png");
        let png = image::RgbaImage::from_raw(image.size[0], image.size[1], image.pixels.to_vec())
            .ok_or_else(|| {
            fail(format!(
                "image {} has the wrong number of pixels",
                image.name
            ))
        })?;
        let mut bytes = Vec::new();
        png.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| fail(e.to_string()))?;
        let path = dir.join(&png_name);
        crate::settings::write_atomic(&path, &bytes).map_err(|e| fail(e.to_string()))?;
        files.push(path);
        let _ = write!(mtl, "\nnewmtl {name}\nKd 1 1 1\nmap_Kd {png_name}\n");
    }
    let mtl_path = dir.join(format!("{stem}.mtl"));
    crate::settings::write_atomic(&mtl_path, mtl.as_bytes()).map_err(|e| fail(e.to_string()))?;
    files.push(mtl_path);
    crate::settings::write_atomic(&request.path, obj.as_bytes())
        .map_err(|e| fail(e.to_string()))?;
    files.push(request.path.clone());
    Ok(
        json!({ "files": files, "vertices": v_base - 1, "faces": faces, "materials": images.len() + 1 }),
    )
}

/// A material name OBJ readers accept: no spaces.
fn material(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
