//! Reading linked art: PNG (and anything `image` decodes) or flattened PSD.

use quarry_core::CommandError;
use quarry_core::command::LoadedImage;
use std::path::Path;

pub const MAX_SIDE: u32 = 16_384;

/// FNV-1a over the file bytes: cheap, stable across runs, enough to tell
/// "this file changed" from "this is the file we already loaded".
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub fn load_image(path: &Path) -> Result<LoadedImage, CommandError> {
    let bytes = std::fs::read(path)
        .map_err(|e| CommandError::io(format!("Could not read {}: {e}.", path.display())))?;
    decode(&bytes, path)
}

/// Decode already-read bytes; `path` only names the file in errors and picks PSD.
pub fn decode(bytes: &[u8], path: &Path) -> Result<LoadedImage, CommandError> {
    let unreadable =
        |why: String| CommandError::io(format!("Could not read {}: {why}", path.display()));
    let is_psd = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("psd"));
    let rgba = if is_psd {
        crate::psd::read_psd(bytes).map_err(|e| unreadable(e.to_string()))?
    } else {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| unreadable(format!("{e}.")))?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        reader.limits(limits);
        reader
            .decode()
            .map_err(|e| unreadable(format!("{e}. Save it as PNG or PSD.")))?
            .into_rgba8()
    };
    if rgba.width() > MAX_SIDE || rgba.height() > MAX_SIDE {
        return Err(unreadable(format!(
            "it is larger than {MAX_SIDE} px on a side."
        )));
    }
    Ok(LoadedImage {
        size: [rgba.width(), rgba.height()],
        pixels: rgba.into_raw(),
        hash: hash(bytes),
    })
}
