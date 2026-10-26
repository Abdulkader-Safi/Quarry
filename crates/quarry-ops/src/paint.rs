//! Pixel painting on RGBA8 images (FR-PAINT-01, 03, 04): palette snapping,
//! brush stamps, lines and flood fill. Pixels are sRGB, row-major, y down.

/// The palette colour closest to `c` ("redmean" distance, cheap and close to
/// what the eye sees). Fully transparent colours (the eraser) never snap.
pub fn nearest(c: [u8; 4], palette: &[[u8; 4]]) -> [u8; 4] {
    if c[3] == 0 || palette.is_empty() {
        return c;
    }
    let distance = |p: &[u8; 4]| {
        let rmean = (c[0] as i32 + p[0] as i32) / 2;
        let (dr, dg, db) = (
            c[0] as i32 - p[0] as i32,
            c[1] as i32 - p[1] as i32,
            c[2] as i32 - p[2] as i32,
        );
        let da = c[3] as i32 - p[3] as i32;
        (((512 + rmean) * dr * dr) >> 8)
            + 4 * dg * dg
            + (((767 - rmean) * db * db) >> 8)
            + 2 * da * da
    };
    *palette
        .iter()
        .min_by_key(|p| distance(p))
        .expect("not empty")
}

/// Brush size in whole pixels from pen pressure 0..1: 1 at a light touch,
/// `max` at full pressure, never 0. No pressure (mouse) means `max`.
pub fn pressure_size(pressure: Option<f32>, max: u32) -> u32 {
    let max = max.max(1);
    match pressure {
        Some(p) if p.is_finite() => ((p.clamp(0.0, 1.0) * max as f32).ceil() as u32).clamp(1, max),
        _ => max,
    }
}

pub struct Canvas<'a> {
    pub pixels: &'a mut [u8],
    pub width: u32,
    pub height: u32,
}

impl Canvas<'_> {
    pub fn get(&self, x: i64, y: i64) -> Option<[u8; 4]> {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return None;
        }
        let i = ((y as u64 * self.width as u64 + x as u64) * 4) as usize;
        Some(self.pixels[i..i + 4].try_into().expect("4 bytes"))
    }

    /// Set one pixel; off-canvas writes are ignored. Returns true if it changed.
    pub fn set(&mut self, x: i64, y: i64, c: [u8; 4]) -> bool {
        if self.get(x, y).is_none_or(|old| old == c) {
            return false;
        }
        let i = ((y as u64 * self.width as u64 + x as u64) * 4) as usize;
        self.pixels[i..i + 4].copy_from_slice(&c);
        true
    }

    /// A square brush `size` pixels wide centred on (x, y).
    pub fn stamp(&mut self, x: i64, y: i64, size: u32, c: [u8; 4]) -> usize {
        let size = size.max(1) as i64;
        let start = -(size - 1) / 2;
        let mut changed = 0;
        for dy in start..start + size {
            for dx in start..start + size {
                changed += self.set(x + dx, y + dy, c) as usize;
            }
        }
        changed
    }

    /// Bresenham line of brush stamps, both ends included.
    pub fn line(&mut self, from: (i64, i64), to: (i64, i64), size: u32, c: [u8; 4]) -> usize {
        let (mut x, mut y) = from;
        let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
        let (sx, sy) = (if x < to.0 { 1 } else { -1 }, if y < to.1 { 1 } else { -1 });
        let mut err = dx + dy;
        let mut changed = 0;
        loop {
            changed += self.stamp(x, y, size, c);
            if (x, y) == to {
                return changed;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Fill a rectangle, clipped to the canvas.
    pub fn rect(&mut self, x: i64, y: i64, w: u32, h: u32, c: [u8; 4]) -> usize {
        let mut changed = 0;
        for yy in y..y + h as i64 {
            for xx in x..x + w as i64 {
                changed += self.set(xx, yy, c) as usize;
            }
        }
        changed
    }

    /// 4-connected flood fill of the exact colour under (x, y).
    pub fn fill(&mut self, x: i64, y: i64, c: [u8; 4]) -> usize {
        let Some(target) = self.get(x, y) else {
            return 0;
        };
        if target == c {
            return 0;
        }
        let mut stack = vec![(x, y)];
        let mut changed = 0;
        while let Some((x, y)) = stack.pop() {
            if self.get(x, y) != Some(target) {
                continue;
            }
            self.set(x, y, c);
            changed += 1;
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(buf: &mut Vec<u8>, w: u32, h: u32) -> Canvas<'_> {
        buf.resize((w * h * 4) as usize, 0);
        Canvas {
            pixels: buf,
            width: w,
            height: h,
        }
    }

    #[test]
    fn one_pixel_changes_one_texel() {
        let mut buf = Vec::new();
        let mut c = canvas(&mut buf, 8, 8);
        assert_eq!(c.stamp(3, 4, 1, [255, 0, 0, 255]), 1);
        assert_eq!(c.get(3, 4), Some([255, 0, 0, 255]));
        assert_eq!(buf.chunks(4).filter(|p| p[3] != 0).count(), 1);
    }

    #[test]
    fn palette_snaps_to_the_nearest_entry_but_not_the_eraser() {
        let palette = [[0, 0, 0, 255], [255, 255, 255, 255], [200, 30, 30, 255]];
        assert_eq!(nearest([220, 40, 50, 255], &palette), [200, 30, 30, 255]);
        assert_eq!(nearest([30, 30, 30, 255], &palette), [0, 0, 0, 255]);
        assert_eq!(nearest([0, 0, 0, 0], &palette), [0, 0, 0, 0]);
    }

    #[test]
    fn pressure_gives_whole_pixel_sizes() {
        assert_eq!(pressure_size(Some(0.15), 3), 1);
        assert_eq!(pressure_size(Some(1.0), 3), 3);
        assert_eq!(pressure_size(Some(0.0), 3), 1, "never zero");
        assert_eq!(pressure_size(None, 3), 3, "no tablet: the set size");
    }

    #[test]
    fn lines_rects_and_fills() {
        let mut buf = Vec::new();
        let mut c = canvas(&mut buf, 16, 16);
        assert_eq!(c.line((0, 0), (5, 0), 1, [9, 9, 9, 255]), 6);
        // A 3 px brush overhangs each end by 1 px: columns 0..=10 (x = -1 is off the canvas).
        assert_eq!(c.line((0, 5), (9, 5), 3, [8, 8, 8, 255]), 11 * 3);
        c.rect(10, 10, 3, 3, [1, 1, 1, 255]);
        // Fill the empty area: everything not yet painted.
        let painted = buf.chunks(4).filter(|p| p[3] != 0).count();
        let mut c = canvas(&mut buf, 16, 16);
        assert_eq!(c.fill(15, 15, [2, 2, 2, 255]), 256 - painted);
    }
}
