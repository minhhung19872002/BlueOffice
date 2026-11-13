//! Windows GPU device setup, isolated from the platform-independent document model.
//!
//! Device creation alone does not render a page or prove that a particular surface uses
//! hardware. A caller must still connect the context to a surface and handle device loss.

/// The driver selected for the Direct2D device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// Direct3D hardware driver.
    Hardware,
    /// Direct3D WARP software rasterizer.
    Software,
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(windows)]
pub use windows::Device;

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn draws_and_reads_back_offscreen_pixels() {
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let (background, rectangle) = device.verify_offscreen().expect("offscreen render");
        assert_eq!(background, [255, 255, 255, 255]);
        assert_eq!(rectangle, [0, 0, 255, 255]);
    }

    #[test]
    fn draws_directwrite_glyph_to_direct2d_surface() {
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let font = include_bytes!("../../../assets/fonts/Tinos-Regular.ttf");
        let glyphs = [bo_dwrite::Glyph {
            id: 36,
            x: 0.0,
            y: 0.0,
        }];
        assert!(device.verify_text(font, &glyphs).expect("glyph draw") > 0);
    }
}
