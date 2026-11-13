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

/// An opaque, row-major BGRA image read back from an offscreen Direct2D surface.
#[derive(Debug)]
pub struct PageImage {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Four premultiplied BGRA bytes per pixel.
    pub pixels: Vec<u8>,
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

    #[test]
    #[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
    fn draws_basic_display_list_and_rejects_unsupported_operations() {
        use bo_render::{Command, Glyph, PageList, Rgba};
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let mut fonts = bo_text::FontManager::new();
        let face =
            fonts.add_font(include_bytes!("../../../assets/fonts/Tinos-Regular.ttf").to_vec())[0];
        let mut page = PageList {
            width: 64.0,
            height: 64.0,
            commands: vec![
                Command::FillRect {
                    x: 0.0,
                    y: 0.0,
                    w: 8.0,
                    h: 8.0,
                    color: Rgba(255, 0, 0, 255),
                },
                Command::Glyphs {
                    face: face.0,
                    size: 20.0,
                    x: 10.0,
                    y: 35.0,
                    color: Rgba(0, 0, 0, 255),
                    text: "A".into(),
                    glyphs: vec![Glyph(36, 0.0, 0.0, 0)],
                },
            ],
        };
        let image = device
            .render_basic_page(&page, &fonts, 1.25)
            .expect("basic page");
        assert_eq!((image.width, image.height), (80, 80));
        assert_eq!(&image.pixels[..4], &[0, 0, 255, 255]);
        assert!(image
            .pixels
            .chunks_exact(4)
            .any(|px| px[0] < 100 && px[1] < 100 && px[2] < 100));
        page.commands.push(Command::PushClip {
            x: 0.0,
            y: 0.0,
            w: 5.0,
            h: 5.0,
        });
        assert!(device.render_basic_page(&page, &fonts, 1.0).is_err());
    }

    #[test]
    fn rectangular_clip_limits_drawing() {
        use bo_render::{Command, PageList, Rgba};
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let page = PageList {
            width: 16.0,
            height: 16.0,
            commands: vec![
                Command::PushClip {
                    x: 4.0,
                    y: 4.0,
                    w: 4.0,
                    h: 4.0,
                },
                Command::FillRect {
                    x: 0.0,
                    y: 0.0,
                    w: 16.0,
                    h: 16.0,
                    color: Rgba(255, 0, 0, 255),
                },
                Command::PopClip,
            ],
        };
        let image = device
            .render_basic_page(&page, &bo_text::FontManager::new(), 1.0)
            .unwrap();
        assert_eq!(&image.pixels[..4], &[255, 255, 255, 255]);
        let offset = (5 * 16 + 5) * 4;
        assert_eq!(&image.pixels[offset..offset + 4], &[0, 0, 255, 255]);
    }

    #[test]
    fn draws_solid_curved_path() {
        use bo_render::{Command, PageList, Paint, PathSeg, Rgba};
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let page = PageList {
            width: 16.0,
            height: 16.0,
            commands: vec![Command::Path {
                segs: vec![
                    PathSeg::M(2.0, 2.0),
                    PathSeg::Q(14.0, 2.0, 14.0, 8.0),
                    PathSeg::C(14.0, 14.0, 2.0, 14.0, 2.0, 8.0),
                    PathSeg::Z,
                ],
                fill: Some(Paint::Solid {
                    color: Rgba(0, 0, 255, 255),
                }),
                stroke: None,
            }],
        };
        let image = device
            .render_basic_page(&page, &bo_text::FontManager::new(), 1.0)
            .unwrap();
        assert_eq!(&image.pixels[..4], &[255, 255, 255, 255]);
        let offset = (8 * 16 + 8) * 4;
        assert_eq!(&image.pixels[offset..offset + 4], &[255, 0, 0, 255]);
    }
}
