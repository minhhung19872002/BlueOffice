use super::Device;
use crate::PageImage;
use bo_render::{Command, PageList, Paint, PathSeg, Rgba};
use bo_text::{FaceId, FontManager};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_WINDING, D2D1_PIXEL_FORMAT,
    D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Brush, ID2D1Image, ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_MAP_OPTIONS_READ, D2D1_QUADRATIC_BEZIER_SEGMENT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows_numerics::Vector2;

fn color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(c.0) / 255.0,
        g: f32::from(c.1) / 255.0,
        b: f32::from(c.2) / 255.0,
        a: f32::from(c.3) / 255.0,
    }
}

fn valid_path(segs: &[PathSeg]) -> bool {
    let mut open = false;
    if segs.is_empty() {
        return false;
    }
    for seg in segs {
        match seg {
            PathSeg::M(..) => open = true,
            PathSeg::Z if open => open = false,
            PathSeg::L(..) | PathSeg::Q(..) | PathSeg::C(..) if open => {}
            _ => return false,
        }
    }
    true
}

impl Device {
    /// Render a limited display list into an offscreen image using Direct2D and DirectWrite.
    ///
    /// Only solid rectangles, solid lines, solid-filled paths, rectangular clips and glyph runs
    /// are supported. The function
    /// rejects every other operation before drawing, so a caller can select a complete fallback
    /// instead of silently dropping content. This is not yet the editor's window surface.
    pub fn render_basic_page(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
    ) -> Result<PageImage, String> {
        if !pixels_per_point.is_finite()
            || pixels_per_point <= 0.0
            || !page.width.is_finite()
            || !page.height.is_finite()
            || page.width <= 0.0
            || page.height <= 0.0
        {
            return Err("invalid page size or scale".into());
        }
        let width = (page.width * pixels_per_point).ceil() as u32;
        let height = (page.height * pixels_per_point).ceil() as u32;
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or("page too large")?;
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err("page too large or empty".into());
        }
        let mut depth = 0_usize;
        for command in &page.commands {
            match command {
                Command::FillRect { .. } | Command::Line { dash: None, .. } => {}
                Command::Glyphs { face, .. } if (*face as usize) < fonts.len() => {}
                Command::PushClip { .. } => depth += 1,
                Command::PopClip if depth > 0 => depth -= 1,
                Command::Path {
                    segs,
                    fill: Some(Paint::Solid { .. }),
                    stroke: None,
                } if valid_path(segs) => {}
                _ => return Err("unsupported display command or font face".into()),
            }
        }
        if depth != 0 {
            return Err("unbalanced rectangular clips".into());
        }
        // SAFETY: the Direct2D interfaces own the target and staging bitmaps. Glyph-run buffers
        // are valid during each synchronous draw. The mapped staging bitmap is read within its
        // documented dimensions and unmapped before return; the context target is always reset.
        unsafe {
            let size = D2D_SIZE_U { width, height };
            let pixel = D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            };
            let dpi = 96.0 * pixels_per_point;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let target = self
                .context
                .CreateBitmap(size, None, 0, &props)
                .map_err(|e| e.to_string())?;
            self.context.SetTarget(&target);
            self.context.SetDpi(dpi, dpi);
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }));
            let mut active_clips = 0_usize;
            let drawn = (|| -> Result<(), String> {
                for command in &page.commands {
                    match command {
                        Command::FillRect {
                            x,
                            y,
                            w,
                            h,
                            color: c,
                        } => {
                            let brush = self
                                .context
                                .CreateSolidColorBrush(&color(*c), None)
                                .map_err(|e| e.to_string())?;
                            self.context.FillRectangle(
                                &D2D_RECT_F {
                                    left: *x,
                                    top: *y,
                                    right: x + w,
                                    bottom: y + h,
                                },
                                &brush,
                            );
                        }
                        Command::Line {
                            x1,
                            y1,
                            x2,
                            y2,
                            width,
                            color: c,
                            ..
                        } => {
                            let brush = self
                                .context
                                .CreateSolidColorBrush(&color(*c), None)
                                .map_err(|e| e.to_string())?;
                            self.context.DrawLine(
                                Vector2 { X: *x1, Y: *y1 },
                                Vector2 { X: *x2, Y: *y2 },
                                &brush,
                                *width,
                                None::<&ID2D1StrokeStyle>,
                            );
                        }
                        Command::Glyphs {
                            face,
                            size,
                            x,
                            y,
                            color: c,
                            glyphs,
                            ..
                        } => {
                            if glyphs.is_empty() {
                                continue;
                            }
                            let brush = self
                                .context
                                .CreateSolidColorBrush(&color(*c), None)
                                .map_err(|e| e.to_string())?;
                            let (font, index) = fonts.data(FaceId(*face));
                            let positions: Vec<bo_dwrite::Glyph> = glyphs
                                .iter()
                                .map(|g| bo_dwrite::Glyph {
                                    id: g.0,
                                    x: g.1,
                                    y: g.2,
                                })
                                .collect();
                            bo_dwrite::with_glyph_run(font, index, &positions, *size, |run| {
                                self.context.DrawGlyphRun(
                                    Vector2 { X: *x, Y: *y },
                                    run,
                                    None,
                                    &brush,
                                    DWRITE_MEASURING_MODE_NATURAL,
                                );
                                Ok(())
                            })?;
                        }
                        Command::PushClip { x, y, w, h } => {
                            self.context.PushAxisAlignedClip(
                                &D2D_RECT_F {
                                    left: *x,
                                    top: *y,
                                    right: x + w,
                                    bottom: y + h,
                                },
                                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                            );
                            active_clips += 1;
                        }
                        Command::PopClip => {
                            self.context.PopAxisAlignedClip();
                            active_clips -= 1;
                        }
                        Command::Path {
                            segs,
                            fill: Some(Paint::Solid { color: c }),
                            stroke: None,
                        } => {
                            let geometry = self
                                .factory
                                .CreatePathGeometry()
                                .map_err(|e| e.to_string())?;
                            let sink = geometry.Open().map_err(|e| e.to_string())?;
                            sink.SetFillMode(D2D1_FILL_MODE_WINDING);
                            let mut open = false;
                            for segment in segs {
                                match *segment {
                                    PathSeg::M(x, y) => {
                                        if open {
                                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                                        }
                                        sink.BeginFigure(
                                            Vector2 { X: x, Y: y },
                                            D2D1_FIGURE_BEGIN_FILLED,
                                        );
                                        open = true;
                                    }
                                    PathSeg::L(x, y) => sink.AddLine(Vector2 { X: x, Y: y }),
                                    PathSeg::Q(x1, y1, x2, y2) => {
                                        sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                                            point1: Vector2 { X: x1, Y: y1 },
                                            point2: Vector2 { X: x2, Y: y2 },
                                        })
                                    }
                                    PathSeg::C(x1, y1, x2, y2, x3, y3) => {
                                        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                                            point1: Vector2 { X: x1, Y: y1 },
                                            point2: Vector2 { X: x2, Y: y2 },
                                            point3: Vector2 { X: x3, Y: y3 },
                                        })
                                    }
                                    PathSeg::Z => {
                                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                                        open = false;
                                    }
                                }
                            }
                            if open {
                                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                            }
                            sink.Close().map_err(|e| e.to_string())?;
                            let brush = self
                                .context
                                .CreateSolidColorBrush(&color(*c), None)
                                .map_err(|e| e.to_string())?;
                            self.context
                                .FillGeometry(&geometry, &brush, None::<&ID2D1Brush>);
                        }
                        _ => return Err("unsupported display command".into()),
                    }
                }
                Ok(())
            })();
            for _ in 0..active_clips {
                self.context.PopAxisAlignedClip();
            }
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            self.context.SetDpi(96.0, 96.0);
            drawn?;
            ended.map_err(|e| e.to_string())?;
            let staging_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let staging = self
                .context
                .CreateBitmap(size, None, 0, &staging_props)
                .map_err(|e| e.to_string())?;
            staging
                .CopyFromBitmap(None, &target, None)
                .map_err(|e| e.to_string())?;
            let mapped = staging
                .Map(D2D1_MAP_OPTIONS_READ)
                .map_err(|e| e.to_string())?;
            let row_bytes = width as usize * 4;
            if mapped.bits.is_null() || (mapped.pitch as usize) < row_bytes {
                staging.Unmap().map_err(|e| e.to_string())?;
                return Err("invalid mapped Direct2D surface".into());
            }
            let mut pixels = vec![0_u8; bytes];
            for y in 0..height as usize {
                let src = std::slice::from_raw_parts(
                    mapped.bits.add(y * mapped.pitch as usize),
                    row_bytes,
                );
                pixels[y * row_bytes..(y + 1) * row_bytes].copy_from_slice(src);
            }
            staging.Unmap().map_err(|e| e.to_string())?;
            Ok(PageImage {
                width,
                height,
                pixels,
            })
        }
    }
}
