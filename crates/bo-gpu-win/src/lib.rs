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

/// How glyph runs are antialiased on an opaque page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextRendering {
    /// The system's DirectWrite defaults for the primary monitor (ClearType if enabled).
    System,
    /// Grayscale antialiasing: no colour fringes; safe when the image is scaled or moved.
    Grayscale,
    /// ClearType with explicit parameters (`IDWriteFactory::CreateCustomRenderingParams`).
    ClearType {
        /// Gamma, normally 1.8 to 2.2.
        gamma: f32,
        /// Extra contrast, 0 or more.
        contrast: f32,
        /// Subpixel strength from 0 (grayscale) to 1 (full ClearType).
        level: f32,
    },
}

impl TextRendering {
    /// Light ClearType chosen by measurement against screenshots of a desktop word processor
    /// on the same monitor (`docs/perf/RESULTS.md`): about a third of the default subpixel
    /// colour and slightly heavier stems. Grayscale when the user turned ClearType off.
    pub fn document_default() -> Self {
        if system_cleartype() {
            Self::ClearType {
                gamma: 1.8,
                contrast: 2.0,
                level: 0.35,
            }
        } else {
            Self::Grayscale
        }
    }
}

/// Whether Windows font smoothing is set to ClearType (always false on other systems).
pub fn system_cleartype() -> bool {
    #[cfg(windows)]
    {
        windows::system_cleartype()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// An opaque, row-major RGBA image read back from an offscreen Direct2D surface.
#[derive(Debug)]
pub struct PageImage {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Four RGBA bytes per pixel (opaque, so premultiplied and straight alpha agree).
    pub pixels: Vec<u8>,
}

/// A decoded picture handed to the renderer: row-major premultiplied RGBA.
#[derive(Debug, Clone)]
pub struct Bitmap {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` premultiplied RGBA bytes.
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
    fn draws_basic_display_list_and_rejects_unbalanced_clips() {
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
            .render_page(&page, &fonts, 1.25, &|_| None)
            .expect("basic page");
        assert_eq!((image.width, image.height), (80, 80));
        assert_eq!(&image.pixels[..4], &[255, 0, 0, 255]);
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
        assert!(device.render_page(&page, &fonts, 1.0, &|_| None).is_err());
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
            .render_page(&page, &bo_text::FontManager::new(), 1.0, &|_| None)
            .unwrap();
        assert_eq!(&image.pixels[..4], &[255, 255, 255, 255]);
        let offset = (5 * 16 + 5) * 4;
        assert_eq!(&image.pixels[offset..offset + 4], &[255, 0, 0, 255]);
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
            .render_page(&page, &bo_text::FontManager::new(), 1.0, &|_| None)
            .unwrap();
        assert_eq!(&image.pixels[..4], &[255, 255, 255, 255]);
        let offset = (8 * 16 + 8) * 4;
        assert_eq!(&image.pixels[offset..offset + 4], &[0, 0, 255, 255]);
    }

    fn px(image: &super::PageImage, x: usize, y: usize) -> [u8; 4] {
        let o = (y * image.width as usize + x) * 4;
        [
            image.pixels[o],
            image.pixels[o + 1],
            image.pixels[o + 2],
            image.pixels[o + 3],
        ]
    }

    fn render(commands: Vec<bo_render::Command>) -> super::PageImage {
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let page = bo_render::PageList {
            width: 32.0,
            height: 32.0,
            commands,
        };
        let picture = |id: &str| {
            (id == "red").then(|| super::Bitmap {
                width: 2,
                height: 2,
                pixels: [255, 0, 0, 255].repeat(4),
            })
        };
        device
            .render_page(&page, &bo_text::FontManager::new(), 1.0, &picture)
            .expect("render")
    }

    fn square(x: f32, y: f32, w: f32) -> Vec<bo_render::PathSeg> {
        use bo_render::PathSeg::{L, M, Z};
        vec![M(x, y), L(x + w, y), L(x + w, y + w), L(x, y + w), Z]
    }

    #[test]
    fn linear_gradient_and_stroke() {
        use bo_render::{Command, GradientStop, Paint, Rgba, Stroke};
        let image = render(vec![Command::Path {
            segs: square(0.0, 0.0, 32.0),
            fill: Some(Paint::Linear {
                x1: 0.0,
                y1: 0.0,
                x2: 32.0,
                y2: 0.0,
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: Rgba(255, 0, 0, 255),
                    },
                    GradientStop {
                        offset: 1.0,
                        color: Rgba(0, 0, 255, 255),
                    },
                ],
            }),
            stroke: Some(Stroke {
                width: 2.0,
                color: Rgba(0, 255, 0, 255),
                dash: vec![],
                cap: "butt".into(),
                join: "miter".into(),
            }),
        }]);
        let left = px(&image, 4, 16);
        let right = px(&image, 27, 16);
        assert!(left[0] > left[2] && right[2] > right[0]);
        assert_eq!(px(&image, 16, 0)[1], 255);
    }

    #[test]
    fn transform_opacity_and_path_clip() {
        use bo_render::{Command, Rgba};
        let image = render(vec![
            Command::PushTransform {
                m: [1.0, 0.0, 0.0, 1.0, 16.0, 0.0],
            },
            Command::PushOpacity { alpha: 0.5 },
            Command::PushClipPath {
                segs: square(0.0, 0.0, 8.0),
            },
            Command::FillRect {
                x: 0.0,
                y: 0.0,
                w: 16.0,
                h: 16.0,
                color: Rgba(0, 0, 0, 255),
            },
            Command::PopClip,
            Command::PopOpacity,
            Command::PopTransform,
        ]);
        // Translated by 16, clipped to 8 × 8, half opacity.
        assert_eq!(px(&image, 4, 4), [255, 255, 255, 255]);
        let grey = px(&image, 20, 4);
        assert!((120..=136).contains(&grey[0]), "{grey:?}");
        assert_eq!(px(&image, 28, 4), [255, 255, 255, 255]);
    }

    #[test]
    fn rotated_rectangle_clip_is_not_its_bounding_box() {
        use bo_render::{Command, Rgba};
        let (s, c) = 45_f32.to_radians().sin_cos();
        let image = render(vec![
            Command::PushTransform {
                m: [c, s, -s, c, 16.0, 16.0 - 8.0 * 2_f32.sqrt()],
            },
            Command::PushClip {
                x: 0.0,
                y: 0.0,
                w: 16.0,
                h: 16.0,
            },
            Command::FillRect {
                x: -32.0,
                y: -32.0,
                w: 96.0,
                h: 96.0,
                color: Rgba(0, 0, 0, 255),
            },
            Command::PopClip,
            Command::PopTransform,
        ]);
        assert_eq!(px(&image, 16, 16), [0, 0, 0, 255]);
        // Corner of the bounding box lies outside the rotated square.
        assert_eq!(px(&image, 6, 6), [255, 255, 255, 255]);
    }

    #[test]
    fn pictures_placeholders_dashes_and_blur() {
        use bo_render::{Command, Rgba};
        let image = render(vec![
            Command::Image {
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 8.0,
                image: "red".into(),
                crop: None,
                rotation: 0.0,
            },
            Command::Image {
                x: 8.0,
                y: 0.0,
                w: 8.0,
                h: 8.0,
                image: "missing".into(),
                crop: None,
                rotation: 0.0,
            },
            Command::Line {
                x1: 0.0,
                y1: 12.0,
                x2: 32.0,
                y2: 12.0,
                width: 2.0,
                color: Rgba(0, 0, 0, 255),
                dash: Some([4.0, 4.0]),
            },
            Command::BlurPath {
                segs: square(20.0, 20.0, 8.0),
                color: Rgba(0, 0, 0, 255),
                radius: 4.0,
                spread: 0.0,
            },
        ]);
        assert_eq!(px(&image, 4, 4), [255, 0, 0, 255]);
        assert_eq!(px(&image, 12, 4), [224, 224, 224, 255]);
        assert_eq!(px(&image, 2, 12)[0], 0);
        assert_eq!(px(&image, 6, 12), [255, 255, 255, 255]);
        // Blur spreads ink outside the square and softens its inside.
        let outside = px(&image, 18, 24)[0];
        let inside = px(&image, 24, 24)[0];
        assert!(outside < 255 && outside > inside, "{outside} {inside}");
    }

    #[test]
    fn font_faces_are_cached_between_pages() {
        use bo_render::{Command, Glyph, PageList, Rgba};
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let mut fonts = bo_text::FontManager::new();
        let face =
            fonts.add_font(include_bytes!("../../../assets/fonts/Tinos-Regular.ttf").to_vec())[0];
        let run = |x| Command::Glyphs {
            face: face.0,
            size: 12.0,
            x,
            y: 20.0,
            color: Rgba(0, 0, 0, 255),
            text: "A".into(),
            glyphs: vec![Glyph(36, 0.0, 0.0, 0)],
        };
        let page = PageList {
            width: 64.0,
            height: 32.0,
            commands: vec![run(2.0), run(20.0), run(40.0)],
        };
        for _ in 0..3 {
            device.render_page(&page, &fonts, 1.0, &|_| None).unwrap();
        }
        assert_eq!(device.cached_faces(), 1);
    }

    #[test]
    #[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
    fn opaque_page_text_reports_its_antialiasing() {
        use bo_render::{Command, Glyph, PageList, Rgba};
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let mut fonts = bo_text::FontManager::new();
        let face =
            fonts.add_font(include_bytes!("../../../assets/fonts/Tinos-Regular.ttf").to_vec())[0];
        let page = PageList {
            width: 64.0,
            height: 32.0,
            commands: vec![Command::Glyphs {
                face: face.0,
                size: 14.0,
                x: 4.0,
                y: 20.0,
                color: Rgba(0, 0, 0, 255),
                text: "A".into(),
                glyphs: vec![Glyph(36, 0.0, 0.0, 0), Glyph(36, 12.0, 0.0, 1)],
            }],
        };
        let image = device.render_page(&page, &fonts, 1.0, &|_| None).unwrap();
        let fringe = image
            .pixels
            .chunks_exact(4)
            .filter(|p| p[0].abs_diff(p[2]) > 12)
            .count();
        // ClearType depends on the machine's text settings; both modes must draw opaque ink.
        eprintln!("subpixel-coloured pixels: {fringe}");
        assert!(image.pixels.chunks_exact(4).all(|p| p[3] == 255));
        assert!(image.pixels.chunks_exact(4).any(|p| p[1] < 100));
    }

    /// Manual comparison aid: `cargo test -p bo-gpu-win text_modes -- --ignored --nocapture`.
    #[test]
    #[ignore = "writes sample images for visual review"]
    #[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
    fn text_modes() {
        use super::TextRendering;
        use bo_render::{Command, Glyph, PageList, Rgba};
        let device = super::Device::new().expect("Direct2D hardware or WARP");
        let mut fonts = bo_text::FontManager::new();
        let face =
            fonts.add_font(include_bytes!("../../../assets/fonts/Tinos-Regular.ttf").to_vec())[0];
        let scale = 13.0 / f32::from(fonts.info(face).units_per_em);
        let mut pen = 0;
        let glyphs: Vec<Glyph> = fonts
            .shape(face, "thị tiếng Việt có dấu, căn lề")
            .iter()
            .map(|g| {
                let glyph = Glyph(g.glyph_id, (pen + g.x_offset) as f32 * scale, 0.0, 0);
                pen += g.x_advance;
                glyph
            })
            .collect();
        let page = PageList {
            width: 200.0,
            height: 24.0,
            commands: vec![Command::Glyphs {
                face: face.0,
                size: 13.0,
                x: 4.0,
                y: 17.0,
                color: Rgba(0, 0, 0, 255),
                text: String::new(),
                glyphs,
            }],
        };
        let out = std::env::var("BO_TEXT_MODES_OUT").unwrap_or_else(|_| ".".into());
        let modes = [
            ("system", TextRendering::System),
            ("gray", TextRendering::Grayscale),
            (
                "ct-0.5",
                TextRendering::ClearType {
                    gamma: 1.8,
                    contrast: 0.5,
                    level: 0.5,
                },
            ),
            (
                "ct-0.3",
                TextRendering::ClearType {
                    gamma: 1.8,
                    contrast: 0.5,
                    level: 0.3,
                },
            ),
            (
                "ct-0.35-c1",
                TextRendering::ClearType {
                    gamma: 1.8,
                    contrast: 1.0,
                    level: 0.35,
                },
            ),
            (
                "ct-0.35-c2",
                TextRendering::ClearType {
                    gamma: 1.8,
                    contrast: 2.0,
                    level: 0.35,
                },
            ),
        ];
        for (name, mode) in modes {
            device.set_text_rendering(mode);
            let img = device
                .render_page(&page, &fonts, 1.4667, &|_| None)
                .unwrap();
            let fringe = img
                .pixels
                .chunks_exact(4)
                .filter(|p| p[0].abs_diff(p[2]) > 12)
                .count() as f32
                / (img.pixels.len() / 4) as f32;
            println!("{name}: fringe {fringe:.3}");
            let mut ppm = format!("P6 {} {} 255\n", img.width, img.height).into_bytes();
            for p in img.pixels.chunks_exact(4) {
                ppm.extend_from_slice(&p[..3]);
            }
            std::fs::write(format!("{out}/text-{name}.ppm"), ppm).unwrap();
        }
    }
}
