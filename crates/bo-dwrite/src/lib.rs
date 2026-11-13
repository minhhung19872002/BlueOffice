//! DirectWrite glyph masks for the native desktop rendering pipeline.
//!
//! The caller supplies glyph IDs and positions from the layout engine. DirectWrite does not
//! shape or choose fonts here, so its output cannot change line breaking. The mask has three
//! coverage values per pixel and must be composited against the actual destination; it is not
//! an ordinary RGBA image and cannot safely be placed over arbitrary content as one.

/// One glyph ID and its position relative to the run baseline, in device-independent units.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    /// Font glyph index.
    pub id: u16,
    /// Horizontal offset from the run origin.
    pub x: f32,
    /// Vertical offset from the baseline, positive downwards.
    pub y: f32,
}

/// A ClearType mask in device pixels; `right` and `bottom` are exclusive.
#[derive(Debug, Clone)]
pub struct Mask {
    /// Left edge in device pixels.
    pub left: i32,
    /// Top edge in device pixels.
    pub top: i32,
    /// Right edge in device pixels.
    pub right: i32,
    /// Bottom edge in device pixels.
    pub bottom: i32,
    /// Three coverage bytes per pixel, row-major.
    pub coverage: Vec<u8>,
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

/// Rasterize a positioned run with DirectWrite, preserving the engine's glyph positions.
///
/// `size`, `origin` and glyph offsets use the same device-independent coordinate system;
/// `pixels_per_unit` converts those coordinates to physical pixels. `face_index` selects a
/// face from a font collection. This only generates a mask, not a complete page or surface.
#[cfg(windows)]
pub fn rasterize(
    font: &[u8],
    face_index: u32,
    glyphs: &[Glyph],
    size: f32,
    pixels_per_unit: f32,
    origin: (f32, f32),
) -> Result<Mask, String> {
    windows::rasterize(font, face_index, glyphs, size, pixels_per_unit, origin)
}

#[cfg(all(test, windows))]
mod tests {
    use super::{rasterize, Glyph};

    const FONT: &[u8] = include_bytes!("../../../assets/fonts/Tinos-Regular.ttf");

    #[test]
    fn masks_follow_physical_scale_and_integral_baseline_moves() {
        let glyph = [Glyph {
            id: 36,
            x: 0.0,
            y: 0.0,
        }];
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let first = rasterize(FONT, 0, &glyph, 20.0, scale, (0.0, 25.0)).unwrap();
            let moved = rasterize(FONT, 0, &glyph, 20.0, scale, (8.0, 25.0)).unwrap();
            assert!(first.coverage.iter().any(|&sample| sample != 0));
            assert_eq!(moved.left - first.left, (8.0 * scale) as i32);
            assert_eq!(first.coverage, moved.coverage);
        }
    }

    #[test]
    fn rejects_invalid_input_and_collection_index() {
        let glyph = [Glyph {
            id: 36,
            x: 0.0,
            y: 0.0,
        }];
        assert!(rasterize(FONT, 1, &glyph, 20.0, 1.0, (0.0, 25.0)).is_err());
        assert!(rasterize(FONT, 0, &glyph, f32::NAN, 1.0, (0.0, 25.0)).is_err());
        assert!(rasterize(FONT, 0, &[], 20.0, 1.0, (0.0, 25.0)).is_err());
    }

    #[test]
    fn positioned_glyph_uses_engine_offsets() {
        let positioned = [Glyph {
            id: 36,
            x: 8.0,
            y: 4.0,
        }];
        let origin = [Glyph {
            id: 36,
            x: 0.0,
            y: 0.0,
        }];
        let a = rasterize(FONT, 0, &positioned, 20.0, 1.25, (0.0, 25.0)).unwrap();
        let b = rasterize(FONT, 0, &origin, 20.0, 1.25, (8.0, 29.0)).unwrap();
        assert_eq!(
            (a.left, a.top, a.right, a.bottom),
            (b.left, b.top, b.right, b.bottom)
        );
        assert_eq!(a.coverage, b.coverage);
    }

    #[test]
    fn vietnamese_composed_and_decomposed_text_match() {
        let mut fonts = bo_text::FontManager::new();
        let face = fonts.add_font(FONT.to_vec())[0];
        let scale = 20.0 / f32::from(fonts.info(face).units_per_em);
        let mut shape = |text: &str| {
            let mut pen = 0;
            fonts
                .shape(face, text)
                .iter()
                .map(|g| {
                    let glyph = Glyph {
                        id: g.glyph_id,
                        x: (pen + g.x_offset) as f32 * scale,
                        y: -(g.y_offset as f32) * scale,
                    };
                    pen += g.x_advance;
                    glyph
                })
                .collect::<Vec<_>>()
        };
        let composed = shape("Việt Nam");
        let decomposed = shape("Vie\u{0323}\u{0302}t Nam");
        assert!(composed.iter().chain(&decomposed).all(|g| g.id != 0));
        for dpi in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let a = rasterize(FONT, 0, &composed, 20.0, dpi, (0.0, 25.0)).unwrap();
            let b = rasterize(FONT, 0, &decomposed, 20.0, dpi, (0.0, 25.0)).unwrap();
            assert_eq!(
                (a.left, a.top, a.right, a.bottom),
                (b.left, b.top, b.right, b.bottom)
            );
            assert_eq!(a.coverage, b.coverage);
        }
    }
}
