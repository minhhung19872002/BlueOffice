use super::surface::PageBitmap;
use super::Device;
use crate::{Bitmap, PageImage, TextRendering};
use bo_render::{Command, PageList, Paint, PathSeg, Rgba, Stroke};
use bo_text::{FaceId, FontManager};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F,
    D2D1_COMPOSITE_MODE_SOURCE_OVER, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_WINDING, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT,
    D2D_RECT_F, D2D_RECT_U, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1GaussianBlur, ID2D1Bitmap1, ID2D1Brush, ID2D1CommandList, ID2D1Geometry, ID2D1Image,
    ID2D1PathGeometry1, ID2D1RenderTarget, ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_ALIASED,
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES, D2D1_CAP_STYLE, D2D1_CAP_STYLE_FLAT,
    D2D1_CAP_STYLE_ROUND, D2D1_CAP_STYLE_SQUARE, D2D1_DASH_STYLE_CUSTOM, D2D1_DASH_STYLE_SOLID,
    D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION,
    D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_LINE_JOIN, D2D1_LINE_JOIN_BEVEL, D2D1_LINE_JOIN_MITER_OR_BEVEL, D2D1_LINE_JOIN_ROUND,
    D2D1_MAP_OPTIONS_READ, D2D1_PROPERTY_TYPE_FLOAT, D2D1_QUADRATIC_BEZIER_SEGMENT,
    D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, D2D1_STROKE_STYLE_PROPERTIES1,
    D2D1_STROKE_TRANSFORM_TYPE_NORMAL,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1_TEXT_ANTIALIAS_MODE_DEFAULT,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory2, IDWriteFactory3, IDWriteRenderingParams,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_GRID_FIT_MODE_DISABLED, DWRITE_GRID_FIT_MODE_ENABLED,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
    DWRITE_RENDERING_MODE1_GDI_CLASSIC, DWRITE_RENDERING_MODE1_GDI_NATURAL,
    DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC_DOWNSAMPLED, DWRITE_RENDERING_MODE1_OUTLINE,
    DWRITE_RENDERING_MODE_NATURAL, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows_numerics::{Matrix3x2, Vector2};

/// Gamma, enhanced contrast and ClearType level of grayscale document text, with
/// antialiasing in both directions: matched to a black-box capture of a desktop word
/// processor (same share of dark and half-covered pixels on body text, docs/ui/REPORT.md).
const GRAY_PARAMS: (f32, f32, f32) = (1.5, 1.0, 0.0);

const IDENTITY: Matrix3x2 = Matrix3x2 {
    M11: 1.0,
    M12: 0.0,
    M21: 0.0,
    M22: 1.0,
    M31: 0.0,
    M32: 0.0,
};
const INFINITE: D2D_RECT_F = D2D_RECT_F {
    left: -f32::MAX,
    top: -f32::MAX,
    right: f32::MAX,
    bottom: f32::MAX,
};
// Bounds that keep one malformed page from exhausting GPU or CPU memory.
const MAX_FONT_BYTES: usize = 256 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;

fn color(c: Rgba, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(c.0) / 255.0,
        g: f32::from(c.1) / 255.0,
        b: f32::from(c.2) / 255.0,
        a: f32::from(c.3) / 255.0 * alpha,
    }
}

/// `local` applied first, then `outer` (Direct2D row-vector convention).
fn concat(local: &Matrix3x2, outer: &Matrix3x2) -> Matrix3x2 {
    Matrix3x2 {
        M11: local.M11 * outer.M11 + local.M12 * outer.M21,
        M12: local.M11 * outer.M12 + local.M12 * outer.M22,
        M21: local.M21 * outer.M11 + local.M22 * outer.M21,
        M22: local.M21 * outer.M12 + local.M22 * outer.M22,
        M31: local.M31 * outer.M11 + local.M32 * outer.M21 + outer.M31,
        M32: local.M31 * outer.M12 + local.M32 * outer.M22 + outer.M32,
    }
}

fn matrix(m: &[f32; 6]) -> Matrix3x2 {
    // bo-render: x' = a·x + c·y + e, y' = b·x + d·y + f.
    Matrix3x2 {
        M11: m[0],
        M12: m[1],
        M21: m[2],
        M22: m[3],
        M31: m[4],
        M32: m[5],
    }
}

fn rotation_about(degrees: f32, cx: f32, cy: f32) -> Matrix3x2 {
    let (s, c) = degrees.to_radians().sin_cos();
    Matrix3x2 {
        M11: c,
        M12: s,
        M21: -s,
        M22: c,
        M31: cx - c * cx + s * cy,
        M32: cy - s * cx - c * cy,
    }
}

/// Device pixels per user unit along one axis and the device offset of user 0.
#[derive(Clone, Copy)]
struct Axis {
    scale: f32,
    offset: f32,
}

impl Axis {
    fn x(m: &Matrix3x2, ppp: f32) -> Self {
        Self {
            scale: m.M11 * ppp,
            offset: m.M31 * ppp,
        }
    }

    fn y(m: &Matrix3x2, ppp: f32) -> Self {
        Self {
            scale: m.M22 * ppp,
            offset: m.M32 * ppp,
        }
    }

    /// `v` moved to the nearest device pixel boundary (at most half a pixel).
    fn snap(self, v: f32) -> f32 {
        if self.scale.abs() < 1e-6 {
            return v;
        }
        let device = v * self.scale + self.offset;
        v + (device.round() - device) / self.scale
    }

    /// Edges `a`..`b` snapped to pixel boundaries, at least one device pixel apart.
    fn snap_span(self, a: f32, b: f32) -> (f32, f32) {
        if self.scale.abs() < 1e-6 || (b - a).abs() < 1e-6 {
            return (a, b);
        }
        let (sa, mut sb) = (self.snap(a), self.snap(b));
        if (sb - sa).abs() < 0.5 / self.scale.abs() {
            sb = sa + (b - a).signum() / self.scale;
        }
        (sa, sb)
    }

    /// A hairline centred at `c` of `width` user units: (centre, width) covering whole pixels.
    fn snap_line(self, c: f32, width: f32) -> (f32, f32) {
        let scale = self.scale.abs();
        if scale < 1e-6 {
            return (c, width);
        }
        let pixels = (width * scale).round().max(1.0);
        let device = c * self.scale + self.offset;
        // Odd widths centre on a pixel centre, even widths on a pixel boundary.
        let centre = if pixels % 2.0 == 1.0 {
            (device - 0.5).round() + 0.5
        } else {
            device.round()
        };
        (c + (centre - device) / self.scale, pixels / scale)
    }
}

fn axis_aligned(m: &Matrix3x2) -> bool {
    m.M12 == 0.0 && m.M21 == 0.0
}

fn scale_of(m: &Matrix3x2) -> f32 {
    (m.M11 * m.M11 + m.M12 * m.M12).sqrt().max(1e-3)
}

fn valid_path(segs: &[PathSeg]) -> bool {
    let mut open = false;
    if segs.is_empty() {
        return false;
    }
    let finite = |v: &[f32]| v.iter().all(|f| f.is_finite());
    for seg in segs {
        match *seg {
            PathSeg::M(x, y) if finite(&[x, y]) => open = true,
            PathSeg::Z if open => open = false,
            PathSeg::L(x, y) if open && finite(&[x, y]) => {}
            PathSeg::Q(a, b, c, d) if open && finite(&[a, b, c, d]) => {}
            PathSeg::C(a, b, c, d, e, f) if open && finite(&[a, b, c, d, e, f]) => {}
            _ => return false,
        }
    }
    true
}

fn cap(name: &str) -> D2D1_CAP_STYLE {
    match name {
        "round" => D2D1_CAP_STYLE_ROUND,
        "square" => D2D1_CAP_STYLE_SQUARE,
        _ => D2D1_CAP_STYLE_FLAT,
    }
}

fn join(name: &str) -> D2D1_LINE_JOIN {
    match name {
        "round" => D2D1_LINE_JOIN_ROUND,
        "bevel" => D2D1_LINE_JOIN_BEVEL,
        // Canvas and PDF miter joins fall back to bevel beyond the miter limit.
        _ => D2D1_LINE_JOIN_MITER_OR_BEVEL,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Saved {
    AxisClip,
    LayerClip,
    Transform(Matrix3x2),
    Opacity(f32),
}

/// Walks the list once: checks nesting, the transform and opacity in force for each command.
struct State {
    stack: Vec<Saved>,
    transform: Matrix3x2,
    alpha: f32,
}

impl State {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            transform: IDENTITY,
            alpha: 1.0,
        }
    }

    /// Apply a state command; `Ok(false)` when `command` draws instead.
    fn apply(&mut self, command: &Command) -> Result<bool, String> {
        match command {
            Command::PushClip { .. } => self.stack.push(if axis_aligned(&self.transform) {
                Saved::AxisClip
            } else {
                Saved::LayerClip
            }),
            Command::PushClipPath { .. } => self.stack.push(Saved::LayerClip),
            Command::PopClip => match self.stack.pop() {
                Some(Saved::AxisClip | Saved::LayerClip) => {}
                _ => return Err("unbalanced clip".into()),
            },
            Command::PushTransform { m } => {
                if !m.iter().all(|v| v.is_finite()) {
                    return Err("invalid transform".into());
                }
                self.stack.push(Saved::Transform(self.transform));
                self.transform = concat(&matrix(m), &self.transform);
            }
            Command::PopTransform => match self.stack.pop() {
                Some(Saved::Transform(t)) => self.transform = t,
                _ => return Err("unbalanced transform".into()),
            },
            Command::PushOpacity { alpha } => {
                self.stack.push(Saved::Opacity(self.alpha));
                // Same as the canvas and tiny-skia backends: opacity multiplies each command.
                self.alpha *= if alpha.is_finite() {
                    alpha.clamp(0.0, 1.0)
                } else {
                    1.0
                };
            }
            Command::PopOpacity => match self.stack.pop() {
                Some(Saved::Opacity(a)) => self.alpha = a,
                _ => return Err("unbalanced opacity".into()),
            },
            _ => return Ok(false),
        }
        Ok(true)
    }
}

impl Device {
    pub(super) fn geometry(&self, segs: &[PathSeg]) -> Result<ID2D1PathGeometry1, String> {
        // SAFETY: the factory owns the geometry; the sink is closed before the geometry is used.
        unsafe {
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
                        sink.BeginFigure(Vector2 { X: x, Y: y }, D2D1_FIGURE_BEGIN_FILLED);
                        open = true;
                    }
                    PathSeg::L(x, y) => sink.AddLine(Vector2 { X: x, Y: y }),
                    PathSeg::Q(x1, y1, x2, y2) => {
                        sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                            point1: Vector2 { X: x1, Y: y1 },
                            point2: Vector2 { X: x2, Y: y2 },
                        });
                    }
                    PathSeg::C(x1, y1, x2, y2, x3, y3) => sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                        point1: Vector2 { X: x1, Y: y1 },
                        point2: Vector2 { X: x2, Y: y2 },
                        point3: Vector2 { X: x3, Y: y3 },
                    }),
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
            Ok(geometry)
        }
    }

    pub(super) fn solid(&self, c: Rgba, alpha: f32) -> Result<ID2D1Brush, String> {
        // SAFETY: the context owns the returned brush.
        unsafe {
            self.context
                .CreateSolidColorBrush(&color(c, alpha), None)
                .map_err(|e| e.to_string())?
                .cast()
                .map_err(|e| e.to_string())
        }
    }

    fn paint(&self, paint: &Paint, alpha: f32) -> Result<ID2D1Brush, String> {
        let stops = |stops: &[bo_render::GradientStop]| -> Vec<D2D1_GRADIENT_STOP> {
            stops
                .iter()
                .map(|s| D2D1_GRADIENT_STOP {
                    position: if s.offset.is_finite() {
                        s.offset.clamp(0.0, 1.0)
                    } else {
                        0.0
                    },
                    color: color(s.color, 1.0),
                })
                .collect()
        };
        let props = D2D1_BRUSH_PROPERTIES {
            opacity: alpha,
            transform: IDENTITY,
        };
        // SAFETY: the context owns the stop collection and brushes it creates.
        unsafe {
            match paint {
                Paint::Solid { color: c } => self.solid(*c, alpha),
                Paint::Linear {
                    x1,
                    y1,
                    x2,
                    y2,
                    stops: s,
                } => {
                    if s.is_empty() {
                        return self.solid(Rgba(0, 0, 0, 0), 0.0);
                    }
                    // ECMA-376 §20.1.8.33 gsLst; pad spread = clamp. Gamma 2.2 interpolates in
                    // sRGB like the canvas and tiny-skia backends.
                    // The ID2D1RenderTarget overload: gamma + extend mode only.
                    let target: &ID2D1RenderTarget = &self.context;
                    let collection = target
                        .CreateGradientStopCollection(
                            &stops(s),
                            D2D1_GAMMA_2_2,
                            D2D1_EXTEND_MODE_CLAMP,
                        )
                        .map_err(|e| e.to_string())?;
                    self.context
                        .CreateLinearGradientBrush(
                            &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                                startPoint: Vector2 { X: *x1, Y: *y1 },
                                endPoint: Vector2 { X: *x2, Y: *y2 },
                            },
                            Some(&props),
                            &collection,
                        )
                        .map_err(|e| e.to_string())?
                        .cast()
                        .map_err(|e| e.to_string())
                }
                Paint::Radial {
                    cx,
                    cy,
                    r,
                    stops: s,
                } => {
                    if s.is_empty() {
                        return self.solid(Rgba(0, 0, 0, 0), 0.0);
                    }
                    // The ID2D1RenderTarget overload: gamma + extend mode only.
                    let target: &ID2D1RenderTarget = &self.context;
                    let collection = target
                        .CreateGradientStopCollection(
                            &stops(s),
                            D2D1_GAMMA_2_2,
                            D2D1_EXTEND_MODE_CLAMP,
                        )
                        .map_err(|e| e.to_string())?;
                    self.context
                        .CreateRadialGradientBrush(
                            &D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                                center: Vector2 { X: *cx, Y: *cy },
                                gradientOriginOffset: Vector2 { X: 0.0, Y: 0.0 },
                                radiusX: *r,
                                radiusY: *r,
                            },
                            Some(&props),
                            &collection,
                        )
                        .map_err(|e| e.to_string())?
                        .cast()
                        .map_err(|e| e.to_string())
                }
            }
        }
    }

    pub(super) fn stroke_style(
        &self,
        width: f32,
        dash: &[f32],
        cap_style: D2D1_CAP_STYLE,
        line_join: D2D1_LINE_JOIN,
    ) -> Result<ID2D1StrokeStyle, String> {
        // Direct2D dash lengths are multiples of the stroke width; the display list uses points.
        let mut dashes: Vec<f32> = dash
            .iter()
            .map(|d| if d.is_finite() { d.max(0.0) } else { 0.0 } / width)
            .collect();
        let usable = dashes.iter().any(|d| *d > 0.0);
        if dashes.len() % 2 == 1 {
            // Canvas repeats an odd dash list to make it even.
            dashes.extend_from_within(..);
        }
        let props = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: cap_style,
            endCap: cap_style,
            dashCap: cap_style,
            lineJoin: line_join,
            miterLimit: 10.0,
            dashStyle: if usable {
                D2D1_DASH_STYLE_CUSTOM
            } else {
                D2D1_DASH_STYLE_SOLID
            },
            dashOffset: 0.0,
            transformType: D2D1_STROKE_TRANSFORM_TYPE_NORMAL,
        };
        // SAFETY: the factory copies the dash array during the call.
        unsafe {
            self.factory
                .CreateStrokeStyle(&props, if usable { Some(&dashes) } else { None })
                .map_err(|e| e.to_string())?
                .cast()
                .map_err(|e| e.to_string())
        }
    }

    fn stroke(&self, s: &Stroke) -> Result<ID2D1StrokeStyle, String> {
        self.stroke_style(s.width, &s.dash, cap(&s.cap), join(&s.join))
    }

    #[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
    fn bitmap(&self, image: &Bitmap) -> Result<ID2D1Bitmap1, String> {
        let len = (image.width as usize)
            .checked_mul(image.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("image too large")?;
        if image.width == 0 || image.height == 0 || image.pixels.len() != len {
            return Err("invalid image".into());
        }
        // Premultiplied RGBA → BGRA, the format Direct2D bitmaps support everywhere.
        let mut bgra = image.pixels.clone();
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        // SAFETY: `bgra` holds width × height × 4 bytes with the given pitch during the copy.
        unsafe {
            self.context
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: image.width,
                        height: image.height,
                    },
                    Some(bgra.as_ptr().cast()),
                    image.width * 4,
                    &props,
                )
                .map_err(|e| e.to_string())
        }
    }

    /// Record a blurred, grown fill in device-independent page space (identity world transform).
    fn blur_image(
        &self,
        segs: &[PathSeg],
        c: Rgba,
        (radius, spread): (f32, f32),
        state: &State,
    ) -> Result<ID2D1Image, String> {
        let geometry = self.geometry(segs)?;
        // SAFETY: the command list is recorded while the context targets it, outside the page's
        // BeginDraw; the target is reset before returning. The effect holds its own input reference.
        unsafe {
            let transformed = self
                .factory
                .CreateTransformedGeometry(&geometry, &state.transform)
                .map_err(|e| e.to_string())?;
            let scale = scale_of(&state.transform);
            let list: ID2D1CommandList = self
                .context
                .CreateCommandList()
                .map_err(|e| e.to_string())?;
            let brush = self.solid(c, state.alpha)?;
            self.context.SetTarget(&list);
            self.context.BeginDraw();
            self.context.SetTransform(&IDENTITY);
            self.context
                .FillGeometry(&transformed, &brush, None::<&ID2D1Brush>);
            if spread > 0.0 {
                // DrawingML glow §20.1.8.32: the outline stroked 2 × spread wide, round joins.
                let style =
                    self.stroke_style(1.0, &[], D2D1_CAP_STYLE_ROUND, D2D1_LINE_JOIN_ROUND)?;
                self.context
                    .DrawGeometry(&transformed, &brush, spread * 2.0 * scale, &style);
            }
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            ended.map_err(|e| e.to_string())?;
            list.Close().map_err(|e| e.to_string())?;
            let effect = self
                .context
                .CreateEffect(&CLSID_D2D1GaussianBlur)
                .map_err(|e| e.to_string())?;
            effect.SetInput(
                0,
                &list.cast::<ID2D1Image>().map_err(|e| e.to_string())?,
                true,
            );
            // Canvas `blur(radius × scale / 2)` and tiny-skia's three box blurs use the
            // same standard deviation: half the radius (§20.1.8.49 outerShdw@blurRad).
            let sigma = (radius.max(0.0) * scale / 2.0).to_le_bytes();
            effect
                .SetValue(
                    D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32,
                    D2D1_PROPERTY_TYPE_FLOAT,
                    &sigma,
                )
                .map_err(|e| e.to_string())?;
            effect.GetOutput().map_err(|e| e.to_string())
        }
    }

    /// Render a page display list into an offscreen image using Direct2D and DirectWrite.
    ///
    /// Every display command is drawn: rectangles, solid and dashed lines, filled and stroked
    /// paths with solid or gradient paint, glyph runs, rectangular and path clips, transforms,
    /// opacity, pictures (`images` resolves an id to premultiplied RGBA, `None` draws the grey
    /// placeholder of the other backends) and blurred shadows. Malformed nesting or geometry
    /// is rejected before drawing so the caller can fall back for the whole page.
    pub fn render_page(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
        images: &dyn Fn(&str) -> Option<Bitmap>,
    ) -> Result<PageImage, String> {
        self.render_page_region(page, fonts, pixels_per_point, images, None)
    }

    /// [`Device::render_page`] limited to `region` (left, top, width, height in device pixels,
    /// clamped to the page): only that band is drawn and read back, for incremental updates.
    pub fn render_page_region(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
        images: &dyn Fn(&str) -> Option<Bitmap>,
        region: Option<[u32; 4]>,
    ) -> Result<PageImage, String> {
        self.render_impl(page, fonts, pixels_per_point, images, region, None)
    }

    /// Draw a page (or the `region` band of it) into a GPU-resident [`PageBitmap`] made by
    /// [`Device::create_page_bitmap`] for the same page size and scale. Nothing is read back:
    /// a window composes the bitmap directly, and later edits redraw only their band.
    pub fn draw_page(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
        images: &dyn Fn(&str) -> Option<Bitmap>,
        region: Option<[u32; 4]>,
        into: &PageBitmap,
    ) -> Result<(), String> {
        let expected = (
            (page.width * pixels_per_point).ceil() as u32,
            (page.height * pixels_per_point).ceil() as u32,
        );
        if expected != (into.width, into.height) {
            return Err("page bitmap size does not match the page".into());
        }
        self.render_impl(
            page,
            fonts,
            pixels_per_point,
            images,
            region,
            Some(&into.bitmap),
        )
        .map(|_| ())
    }

    #[allow(clippy::too_many_lines, clippy::chunks_exact_to_as_chunks)]
    fn render_impl(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
        images: &dyn Fn(&str) -> Option<Bitmap>,
        region: Option<[u32; 4]>,
        into: Option<&ID2D1Bitmap1>,
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
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err("page too large or empty".into());
        }
        if (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .is_none_or(|n| n > 256 * 1024 * 1024)
        {
            return Err("page too large".into());
        }
        let [left, top, band_w, band_h] = match region {
            Some([x, y, w, h]) => {
                let (x, y) = (x.min(width), y.min(height));
                [x, y, w.min(width - x), h.min(height - y)]
            }
            None => [0, 0, width, height],
        };
        if band_w == 0 || band_h == 0 {
            return Ok(PageImage {
                width: 0,
                height: 0,
                left,
                top,
                page_width: width,
                page_height: height,
                pixels: Vec::new(),
            });
        }
        let bytes = band_w as usize * band_h as usize * 4;

        // Pass 1: validate, prepare glyph runs, decode pictures and record blurred shapes.
        let mut faces = self.faces.try_borrow_mut().map_err(|_| "font cache busy")?;
        let mut state = State::new();
        let mut runs = HashMap::new();
        let mut blurs = HashMap::new();
        let mut bitmaps: HashMap<&str, Option<ID2D1Bitmap1>> = HashMap::new();
        let mut image_bytes = 0_usize;
        for (index, command) in page.commands.iter().enumerate() {
            if state.apply(command)? {
                continue;
            }
            match command {
                Command::Glyphs {
                    face, size, glyphs, ..
                } => {
                    if (*face as usize) >= fonts.len() {
                        return Err("unknown font face".into());
                    }
                    if glyphs.is_empty() {
                        continue;
                    }
                    let (font, collection) = fonts.data(FaceId(*face));
                    let positions: Vec<bo_dwrite::Glyph> = glyphs
                        .iter()
                        .map(|g| bo_dwrite::Glyph {
                            id: g.0,
                            x: g.1,
                            y: g.2,
                        })
                        .collect();
                    runs.insert(
                        index,
                        faces.run(*face, font, collection, &positions, *size)?,
                    );
                }
                Command::Path { segs, .. } | Command::PushClipPath { segs }
                    if !valid_path(segs) =>
                {
                    return Err("invalid path".into());
                }
                Command::BlurPath {
                    segs,
                    color: c,
                    radius,
                    spread,
                } => {
                    if !valid_path(segs) || !radius.is_finite() || !spread.is_finite() {
                        return Err("invalid blurred path".into());
                    }
                    blurs.insert(
                        index,
                        self.blur_image(segs, *c, (*radius, *spread), &state)?,
                    );
                }
                Command::Image { image, .. } if !bitmaps.contains_key(image.as_str()) => {
                    let decoded = images(image);
                    let bitmap = match decoded {
                        Some(b) => {
                            image_bytes = image_bytes.saturating_add(b.pixels.len());
                            if image_bytes > MAX_IMAGE_BYTES {
                                return Err("pictures too large".into());
                            }
                            self.bitmap(&b).ok()
                        }
                        None => None,
                    };
                    bitmaps.insert(image.as_str(), bitmap);
                }
                _ => {}
            }
        }
        if !state.stack.is_empty() {
            return Err("unbalanced clip, transform or opacity".into());
        }
        if faces.font_bytes() > MAX_FONT_BYTES {
            // Drop cached copies of a very large font set rather than keep growing.
            faces.clear();
        }
        drop(faces);

        // SAFETY: the Direct2D interfaces own the target and staging bitmaps. Glyph runs, their
        // buffers, bitmaps and blur images outlive EndDraw. Layers and clips pushed here are
        // popped before EndDraw; the mapped staging bitmap is read within its dimensions and
        // unmapped before return; the context target, transform and DPI are always reset.
        unsafe {
            let size = D2D_SIZE_U { width, height };
            // The page is opaque: an alpha-ignoring target lets Direct2D draw ClearType text
            // (premultiplied targets fall back to grayscale), following the system setting.
            let pixel = D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            };
            let dpi = 96.0 * pixels_per_point;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let target = match into {
                Some(bitmap) => bitmap.clone(),
                None => self
                    .context
                    .CreateBitmap(size, None, 0, &props)
                    .map_err(|e| e.to_string())?,
            };
            self.context.SetTarget(&target);
            self.context.SetDpi(dpi, dpi);
            self.apply_text_rendering()?;
            self.context.BeginDraw();
            self.context.SetTransform(&IDENTITY);
            let banded = region.is_some();
            if banded {
                // Direct2D culls everything outside the band; coordinates are in DIPs (points).
                self.context.PushAxisAlignedClip(
                    &D2D_RECT_F {
                        left: left as f32 / pixels_per_point,
                        top: top as f32 / pixels_per_point,
                        right: (left + band_w) as f32 / pixels_per_point,
                        bottom: (top + band_h) as f32 / pixels_per_point,
                    },
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
            }
            // Clear respects the axis-aligned band clip: a reused bitmap keeps its other pixels.
            self.context
                .Clear(Some(&color(Rgba(255, 255, 255, 255), 1.0)));
            let mut state = State::new();
            let drawn = (|| -> Result<(), String> {
                for (index, command) in page.commands.iter().enumerate() {
                    match command {
                        Command::PushClip { x, y, w, h } => {
                            let rect = D2D_RECT_F {
                                left: *x,
                                top: *y,
                                right: x + w,
                                bottom: y + h,
                            };
                            if axis_aligned(&state.transform) {
                                self.context
                                    .PushAxisAlignedClip(&rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                            } else {
                                let geometry: ID2D1Geometry = self
                                    .factory
                                    .CreateRectangleGeometry(&rect)
                                    .map_err(|e| e.to_string())?
                                    .cast()
                                    .map_err(|e| e.to_string())?;
                                self.push_mask(geometry);
                            }
                        }
                        Command::PushClipPath { segs } => {
                            let geometry: ID2D1Geometry =
                                self.geometry(segs)?.cast().map_err(|e| e.to_string())?;
                            self.push_mask(geometry);
                        }
                        Command::PopClip => match state.stack.last() {
                            Some(Saved::AxisClip) => self.context.PopAxisAlignedClip(),
                            _ => self.context.PopLayer(),
                        },
                        _ => {}
                    }
                    if state.apply(command)? {
                        if matches!(
                            command,
                            Command::PushTransform { .. } | Command::PopTransform
                        ) {
                            self.context.SetTransform(&state.transform);
                        }
                        continue;
                    }
                    let alpha = state.alpha;
                    match command {
                        Command::FillRect {
                            x,
                            y,
                            w,
                            h,
                            color: c,
                        } => {
                            let brush = self.solid(*c, alpha)?;
                            let (mut left, mut right) = (*x, x + w);
                            let (mut top, mut bottom) = (*y, y + h);
                            if axis_aligned(&state.transform) {
                                // Borders, rules and shading land on whole device pixels.
                                (left, right) = Axis::x(&state.transform, pixels_per_point)
                                    .snap_span(left, right);
                                (top, bottom) = Axis::y(&state.transform, pixels_per_point)
                                    .snap_span(top, bottom);
                            }
                            self.context.FillRectangle(
                                &D2D_RECT_F {
                                    left,
                                    top,
                                    right,
                                    bottom,
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
                            dash,
                        } => {
                            if *width <= 0.0 || !width.is_finite() {
                                continue;
                            }
                            let brush = self.solid(*c, alpha)?;
                            let style = self.stroke_style(
                                *width,
                                dash.as_ref().map_or(&[][..], |d| &d[..]),
                                D2D1_CAP_STYLE_FLAT,
                                D2D1_LINE_JOIN_MITER_OR_BEVEL,
                            )?;
                            let (mut a, mut b, mut stroke) = (
                                Vector2 { X: *x1, Y: *y1 },
                                Vector2 { X: *x2, Y: *y2 },
                                *width,
                            );
                            if axis_aligned(&state.transform) {
                                let ax = Axis::x(&state.transform, pixels_per_point);
                                let ay = Axis::y(&state.transform, pixels_per_point);
                                if y1 == y2 {
                                    // Horizontal rule: a crisp band of whole pixel rows.
                                    let (y, w) = ay.snap_line(*y1, *width);
                                    (a.Y, b.Y, stroke) = (y, y, w);
                                    (a.X, b.X) = ax.snap_span(*x1, *x2);
                                } else if x1 == x2 {
                                    let (x, w) = ax.snap_line(*x1, *width);
                                    (a.X, b.X, stroke) = (x, x, w);
                                    (a.Y, b.Y) = ay.snap_span(*y1, *y2);
                                }
                            }
                            self.context.DrawLine(a, b, &brush, stroke, &style);
                        }
                        Command::Glyphs { color: c, x, y, .. } => {
                            let Some(run) = runs.get(&index) else {
                                continue;
                            };
                            let brush = self.solid(*c, alpha)?;
                            // Baseline on a device pixel row, so vertical hinting keeps
                            // horizontal strokes on single rows; x keeps the layout position.
                            let baseline = if axis_aligned(&state.transform) {
                                Axis::y(&state.transform, pixels_per_point).snap(*y)
                            } else {
                                *y
                            };
                            self.context.DrawGlyphRun(
                                Vector2 { X: *x, Y: baseline },
                                run.as_raw(),
                                None,
                                &brush,
                                DWRITE_MEASURING_MODE_NATURAL,
                            );
                        }
                        Command::Path { segs, fill, stroke } => {
                            let geometry = self.geometry(segs)?;
                            if let Some(fill) = fill {
                                let brush = self.paint(fill, alpha)?;
                                self.context
                                    .FillGeometry(&geometry, &brush, None::<&ID2D1Brush>);
                            }
                            if let Some(s) = stroke.as_ref().filter(|s| s.width > 0.0) {
                                let brush = self.solid(s.color, alpha)?;
                                let style = self.stroke(s)?;
                                self.context
                                    .DrawGeometry(&geometry, &brush, s.width, &style);
                            }
                        }
                        Command::BlurPath { .. } => {
                            let Some(image) = blurs.get(&index) else {
                                continue;
                            };
                            // The blur was recorded in page space: draw without the transform.
                            self.context.SetTransform(&IDENTITY);
                            self.context.DrawImage(
                                image,
                                None,
                                None,
                                D2D1_INTERPOLATION_MODE_LINEAR,
                                D2D1_COMPOSITE_MODE_SOURCE_OVER,
                            );
                            self.context.SetTransform(&state.transform);
                        }
                        Command::Image {
                            x,
                            y,
                            w,
                            h,
                            image,
                            crop,
                            rotation,
                        } => {
                            let local = if *rotation == 0.0 || !rotation.is_finite() {
                                state.transform
                            } else {
                                concat(
                                    &rotation_about(*rotation, x + w / 2.0, y + h / 2.0),
                                    &state.transform,
                                )
                            };
                            self.context.SetTransform(&local);
                            let dest = D2D_RECT_F {
                                left: *x,
                                top: *y,
                                right: x + w,
                                bottom: y + h,
                            };
                            match bitmaps.get(image.as_str()).and_then(Option::as_ref) {
                                Some(bitmap) => {
                                    let s = bitmap.GetPixelSize();
                                    let (iw, ih) = (s.width as f32, s.height as f32);
                                    let [l, t, r, b] = crop.unwrap_or([0.0; 4]);
                                    // Same source rectangle as the canvas backend.
                                    let source = D2D_RECT_F {
                                        left: iw * l,
                                        top: ih * t,
                                        right: iw * l + iw * (1.0 - l - r).max(0.0001),
                                        bottom: ih * t + ih * (1.0 - t - b).max(0.0001),
                                    };
                                    self.context.DrawBitmap(
                                        bitmap,
                                        Some(&dest),
                                        alpha,
                                        D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                                        Some(&source),
                                        None,
                                    );
                                }
                                None => {
                                    let brush = self.solid(Rgba(224, 224, 224, 255), alpha)?;
                                    self.context.FillRectangle(&dest, &brush);
                                }
                            }
                            self.context.SetTransform(&state.transform);
                        }
                        _ => {}
                    }
                }
                Ok(())
            })();
            // Unwind whatever an early error left pushed, innermost first.
            while let Some(saved) = state.stack.pop() {
                match saved {
                    Saved::AxisClip => self.context.PopAxisAlignedClip(),
                    Saved::LayerClip => self.context.PopLayer(),
                    Saved::Transform(_) | Saved::Opacity(_) => {}
                }
            }
            if banded {
                self.context.PopAxisAlignedClip();
            }
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            self.context.SetTransform(&IDENTITY);
            self.context.SetDpi(96.0, 96.0);
            drawn?;
            ended.map_err(|e| e.to_string())?;
            if into.is_some() {
                return Ok(PageImage {
                    width: band_w,
                    height: band_h,
                    left,
                    top,
                    page_width: width,
                    page_height: height,
                    pixels: Vec::new(),
                });
            }
            let staging_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: dpi,
                dpiY: dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let staging = self
                .context
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: band_w,
                        height: band_h,
                    },
                    None,
                    0,
                    &staging_props,
                )
                .map_err(|e| e.to_string())?;
            staging
                .CopyFromBitmap(
                    None,
                    &target,
                    Some(&D2D_RECT_U {
                        left,
                        top,
                        right: left + band_w,
                        bottom: top + band_h,
                    }),
                )
                .map_err(|e| e.to_string())?;
            let mapped = staging
                .Map(D2D1_MAP_OPTIONS_READ)
                .map_err(|e| e.to_string())?;
            let row_bytes = band_w as usize * 4;
            if mapped.bits.is_null() || (mapped.pitch as usize) < row_bytes {
                staging.Unmap().map_err(|e| e.to_string())?;
                return Err("invalid mapped Direct2D surface".into());
            }
            let mut pixels = vec![0_u8; bytes];
            for y in 0..band_h as usize {
                let src = std::slice::from_raw_parts(
                    mapped.bits.add(y * mapped.pitch as usize),
                    row_bytes,
                );
                let dst = &mut pixels[y * row_bytes..(y + 1) * row_bytes];
                // BGRA → RGBA; the target ignores alpha, so the page is written fully opaque.
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d.copy_from_slice(&[s[2], s[1], s[0], 255]);
                }
            }
            staging.Unmap().map_err(|e| e.to_string())?;
            Ok(PageImage {
                width: band_w,
                height: band_h,
                left,
                top,
                page_width: width,
                page_height: height,
                pixels,
            })
        }
    }

    /// Set the text antialiasing mode and DirectWrite parameters for the chosen mode.
    unsafe fn apply_text_rendering(&self) -> Result<(), String> {
        let mode = *self.text.borrow();
        // SAFETY: the factory and rendering parameters are owned COM references; the context
        // keeps its own reference to the parameters it is given.
        unsafe {
            let (mut symmetric, mut grid_fit) = (false, true);
            // Grayscale rendering mode (BO_GRAY_MODE compares: `legacy`, `sym nofit`, `down`
            // supersampled, `outline`, `sym1`).
            let mut mode1 = None;
            let (antialias, level) = match mode {
                TextRendering::System => {
                    self.context
                        .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_DEFAULT);
                    self.context
                        .SetTextRenderingParams(None::<&IDWriteRenderingParams>);
                    return Ok(());
                }
                TextRendering::Grayscale => {
                    // BO_GRAY_PARAMS="gamma,contrast" tunes grayscale text for comparisons.
                    let tuned = std::env::var("BO_GRAY_PARAMS").ok().and_then(|v| {
                        let (g, c) = v.split_once(',')?;
                        Some((g.parse().ok()?, c.parse().ok()?, 0.0))
                    });
                    symmetric = true;
                    // Symmetric antialiasing through the newer rendering-mode API: 17 grey
                    // levels at every zoom, like the reference capture (16) — the older API
                    // drops to 4 levels (horizontal 3x oversampling) once the font's gasp
                    // table turns hinting off, about 140 % zoom and up (docs/ui/REPORT.md).
                    mode1 = Some(DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC);
                    if let Ok(v) = std::env::var("BO_GRAY_MODE") {
                        mode1 = None;
                        symmetric = v.contains("sym") || v.contains("legacy");
                        grid_fit = !v.contains("nofit");
                        if v.contains("down") {
                            mode1 = Some(DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC_DOWNSAMPLED);
                        } else if v.contains("outline") {
                            mode1 = Some(DWRITE_RENDERING_MODE1_OUTLINE);
                        } else if v.contains("gdinat") {
                            mode1 = Some(DWRITE_RENDERING_MODE1_GDI_NATURAL);
                        } else if v.contains("gdiclassic") {
                            mode1 = Some(DWRITE_RENDERING_MODE1_GDI_CLASSIC);
                        } else if v.contains("sym1") {
                            mode1 = Some(DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC);
                        }
                    }
                    (
                        D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                        tuned.unwrap_or(GRAY_PARAMS),
                    )
                }
                TextRendering::ClearType {
                    gamma,
                    contrast,
                    level,
                } => (D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, (gamma, contrast, level)),
                TextRendering::Tuned {
                    gamma,
                    contrast,
                    level,
                    symmetric: s,
                    grid_fit: g,
                } => {
                    (symmetric, grid_fit) = (s, g);
                    (D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, (gamma, contrast, level))
                }
            };
            let (gamma, contrast, level) = level;
            let factory: IDWriteFactory2 =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| e.to_string())?;
            // Offscreen callers use the primary monitor; window hosts can select their monitor.
            let geometry = match self.pixel_geometry.get() {
                Some(geometry) => geometry,
                None => factory
                    .CreateRenderingParams()
                    .map_err(|e| e.to_string())?
                    .GetPixelGeometry(),
            };
            if let Some(m) = mode1 {
                let f3: IDWriteFactory3 = factory.cast().map_err(|e| e.to_string())?;
                let params = f3
                    .CreateCustomRenderingParams(
                        gamma.clamp(1.0, 3.0),
                        contrast.clamp(0.0, 4.0),
                        contrast.clamp(0.0, 4.0),
                        level.clamp(0.0, 1.0),
                        geometry,
                        m,
                        if grid_fit {
                            DWRITE_GRID_FIT_MODE_ENABLED
                        } else {
                            DWRITE_GRID_FIT_MODE_DISABLED
                        },
                    )
                    .map_err(|e| e.to_string())?;
                self.context.SetTextAntialiasMode(antialias);
                self.context.SetTextRenderingParams(&params);
                return Ok(());
            }
            // NATURAL: antialiasing along the line only, outlines grid-fitted vertically
            // (baselines, x-height and horizontal strokes on whole rows, as desktop word
            // processors draw body text); glyph x positions stay exactly the engine's.
            let params = factory
                .CreateCustomRenderingParams(
                    gamma.clamp(1.0, 3.0),
                    contrast.clamp(0.0, 4.0),
                    contrast.clamp(0.0, 4.0),
                    level.clamp(0.0, 1.0),
                    geometry,
                    if symmetric {
                        DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC
                    } else {
                        DWRITE_RENDERING_MODE_NATURAL
                    },
                    if grid_fit {
                        DWRITE_GRID_FIT_MODE_ENABLED
                    } else {
                        DWRITE_GRID_FIT_MODE_DISABLED
                    },
                )
                .map_err(|e| e.to_string())?;
            self.context.SetTextAntialiasMode(antialias);
            self.context.SetTextRenderingParams(&params);
        }
        Ok(())
    }

    /// Push a layer clipped to `geometry` (in the current world space).
    unsafe fn push_mask(&self, geometry: ID2D1Geometry) {
        let mut params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: INFINITE,
            geometricMask: ManuallyDrop::new(Some(geometry)),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: IDENTITY,
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        // SAFETY: Direct2D adds its own reference to the mask; ours is released right after.
        unsafe {
            self.context.PushLayer(&params, None);
            ManuallyDrop::drop(&mut params.geometricMask);
        }
    }
}
