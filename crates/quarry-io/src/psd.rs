//! Flattened PSD reading (FR-ART-03): Quarry uses the composite image the
//! editor saved, never its own layer blend.

use image::RgbaImage;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum PsdReadError {
    /// Not a PSD, or a PSD the reader cannot parse.
    Unreadable(String),
    /// The file has layers but no flattened copy of them.
    NoComposite,
}

impl fmt::Display for PsdReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "This PSD could not be read ({why})."),
            Self::NoComposite => write!(
                f,
                "This PSD has no flattened image. Re-save it with Maximize Compatibility turned on."
            ),
        }
    }
}

impl std::error::Error for PsdReadError {}

/// Read the composite image of a PSD.
pub fn read_psd(bytes: &[u8]) -> Result<RgbaImage, PsdReadError> {
    // The psd crate indexes slices without bounds checks, so a truncated file
    // panics instead of returning an error. Catch that here.
    let parsed = std::panic::catch_unwind(|| {
        let psd = psd::Psd::from_bytes(bytes).map_err(|e| e.to_string())?;
        Ok::<_, String>((psd.width(), psd.height(), psd.rgba(), psd.layers().len()))
    });
    let (width, height, rgba, layers) = match parsed {
        Ok(Ok(parts)) => parts,
        Ok(Err(why)) => return Err(PsdReadError::Unreadable(why)),
        Err(_) => return Err(PsdReadError::Unreadable("the file is damaged".into())),
    };

    // ponytail: a layered file whose composite is one flat colour is taken as
    // "saved without Maximize Compatibility". A real one-colour layered PSD
    // trips this too; compare against a layer flatten if that ever matters.
    let uniform = rgba.chunks_exact(4).all(|p| p == &rgba[..4]);
    if layers > 0 && uniform {
        return Err(PsdReadError::NoComposite);
    }
    RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| PsdReadError::Unreadable("image size does not match its data".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal RGB PSD with no layers and raw (uncompressed) composite data.
    fn tiny_psd(width: u32, height: u32, rgb: &[[u8; 3]]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"8BPS");
        b.extend_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&[0; 6]);
        b.extend_from_slice(&3u16.to_be_bytes()); // channels
        b.extend_from_slice(&height.to_be_bytes());
        b.extend_from_slice(&width.to_be_bytes());
        b.extend_from_slice(&8u16.to_be_bytes()); // depth
        b.extend_from_slice(&3u16.to_be_bytes()); // RGB
        for _ in 0..3 {
            b.extend_from_slice(&0u32.to_be_bytes()); // colour mode, resources, layers
        }
        b.extend_from_slice(&0u16.to_be_bytes()); // raw
        for channel in 0..3 {
            b.extend(rgb.iter().map(|p| p[channel]));
        }
        b
    }

    #[test]
    fn reads_the_composite() {
        let pixels = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [10, 20, 30]];
        let image = read_psd(&tiny_psd(2, 2, &pixels)).expect("tiny PSD reads");
        assert_eq!(image.dimensions(), (2, 2));
        assert_eq!(image.get_pixel(1, 0).0, [0, 255, 0, 255]);
        assert_eq!(image.get_pixel(1, 1).0, [10, 20, 30, 255]);
    }

    #[test]
    fn damaged_file_is_an_error_not_a_crash() {
        let mut bytes = tiny_psd(2, 2, &[[1, 2, 3]; 4]);
        bytes.truncate(30);
        assert!(matches!(read_psd(&bytes), Err(PsdReadError::Unreadable(_))));
        assert!(matches!(
            read_psd(b"not a psd"),
            Err(PsdReadError::Unreadable(_))
        ));
    }
}
