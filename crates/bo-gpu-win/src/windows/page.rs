use super::surface::PageBitmap;
use super::Device;
use crate::{Bitmap, PageImage, TextRendering};
use bo_render::{BlendMode, Command, PageList, Paint, PathSeg, Rgba, SoftMask, Stroke};
use bo_text::{FaceId, FontManager};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F,
    D2D1_COMPOSITE_MODE_SOURCE_COPY, D2D1_COMPOSITE_MODE_SOURCE_OVER, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_WINDING, D2D1_GRADIENT_STOP,
    D2D1_PIXEL_FORMAT, D2D_POINT_2U, D2D_RECT_F, D2D_RECT_U, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_BLEND_MODE, D2D1_BLEND_MODE_COLOR, D2D1_BLEND_MODE_COLOR_BURN,
    D2D1_BLEND_MODE_COLOR_DODGE, D2D1_BLEND_MODE_DARKEN, D2D1_BLEND_MODE_DIFFERENCE,
    D2D1_BLEND_MODE_EXCLUSION, D2D1_BLEND_MODE_HARD_LIGHT, D2D1_BLEND_MODE_HUE,
    D2D1_BLEND_MODE_LIGHTEN, D2D1_BLEND_MODE_LUMINOSITY, D2D1_BLEND_MODE_MULTIPLY,
    D2D1_BLEND_MODE_OVERLAY, D2D1_BLEND_MODE_SATURATION, D2D1_BLEND_MODE_SCREEN,
    D2D1_BLEND_MODE_SOFT_LIGHT,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1Blend, CLSID_D2D1LuminanceToAlpha, D2D1_BLEND_PROP_MODE, D2D1_IMAGE_BRUSH_PROPERTIES,
    D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_PROPERTY_TYPE_ENUM,
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
    D2D1_PRIMITIVE_BLEND_COPY, D2D1_PRIMITIVE_BLEND_SOURCE_OVER,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1_TEXT_ANTIALIAS_MODE_DEFAULT,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory2, IDWriteFactory3, IDWriteRenderingParams,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_GRID_FIT_MODE_DISABLED, DWRITE_GRID_FIT_MODE_ENABLED,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_RENDERING_MODE1_GDI_CLASSIC,
    DWRITE_RENDERING_MODE1_GDI_NATURAL, DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
    DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC_DOWNSAMPLED, DWRITE_RENDERING_MODE1_OUTLINE,
    DWRITE_RENDERING_MODE_NATURAL, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows_numerics::{Matrix3x2, Vector2};

/// Gamma, enhanced contrast and ClearType level of grayscale document text, with
/// antialiasing in both directions: matched to a black-box capture of a desktop word
/// processor (same share of dark and half-covered pixels on body text, docs/ui/REPORT.md).
const GRAY_PARAMS: (f32, f32, f32) = (1.5, 1.0, 0.0);
/// Gamma and enhanced contrast of small grayscale sans-serif text (Calibri, Arial, …,
/// below [`SANS_SMALL_PX`]): with the serif setting their horizontal strokes came out
/// softer than the reference capture at 100 % (hard edges 0.21 vs 0.26); this pair gives
/// 0.25. Larger sizes already matched it with the serif setting (docs/ui/REPORT.md
/// rounds 23–24).
const SANS_GRAY_PARAMS: (f32, f32) = (1.8, 3.0);
/// Em size in device pixels below which sans-serif text uses [`SANS_GRAY_PARAMS`].
const SANS_SMALL_PX: f32 = 20.0;

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
    /// A group; `true` when it pushed a soft mask layer.
    Group(bool),
}

/// Identity of a command within the page being drawn (the page is borrowed throughout).
fn key(c: &Command) -> usize {
    std::ptr::from_ref(c) as usize
}

/// A clip pushed while drawing a list, kept so it can be closed and opened again around a
/// blended group (the backdrop copy cannot see through a layer).
struct ClipItem {
    rect: D2D_RECT_F,
    /// The layer's geometry; `None` for an axis-aligned clip of `rect`.
    geometry: Option<ID2D1Geometry>,
    transform: Matrix3x2,
}

/// Index of the `PopGroup` matching the `PushGroup` at `start`.
fn group_end(commands: &[Command], start: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (i, c) in commands.iter().enumerate().skip(start) {
        match c {
            Command::PushGroup { .. } => depth += 1,
            Command::PopGroup => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The Direct2D blend effect mode for a PDF blend mode (ISO 32000-1 §11.3.5).
fn d2d_blend(mode: BlendMode) -> D2D1_BLEND_MODE {
    match mode {
        BlendMode::Multiply => D2D1_BLEND_MODE_MULTIPLY,
        BlendMode::Screen => D2D1_BLEND_MODE_SCREEN,
        BlendMode::Overlay => D2D1_BLEND_MODE_OVERLAY,
        BlendMode::Darken => D2D1_BLEND_MODE_DARKEN,
        BlendMode::Lighten => D2D1_BLEND_MODE_LIGHTEN,
        BlendMode::ColorDodge => D2D1_BLEND_MODE_COLOR_DODGE,
        BlendMode::ColorBurn => D2D1_BLEND_MODE_COLOR_BURN,
        BlendMode::HardLight => D2D1_BLEND_MODE_HARD_LIGHT,
        BlendMode::SoftLight => D2D1_BLEND_MODE_SOFT_LIGHT,
        BlendMode::Difference => D2D1_BLEND_MODE_DIFFERENCE,
        BlendMode::Exclusion => D2D1_BLEND_MODE_EXCLUSION,
        BlendMode::Hue => D2D1_BLEND_MODE_HUE,
        BlendMode::Saturation => D2D1_BLEND_MODE_SATURATION,
        BlendMode::Color => D2D1_BLEND_MODE_COLOR,
        BlendMode::Luminosity => D2D1_BLEND_MODE_LUMINOSITY,
    }
}

/// Resources prepared before drawing a page, keyed by command identity ([key]).
struct Prep<'p> {
    runs: HashMap<usize, bo_dwrite::PreparedGlyphRun>,
    blurs: HashMap<usize, ID2D1Image>,
    bitmaps: HashMap<&'p str, Option<ID2D1Bitmap1>>,
    image_bytes: usize,
    /// Groups to record (blend mode or soft mask), in list order.
    jobs: Vec<GroupJob<'p>>,
    /// Recorded groups by their `PushGroup`.
    groups: HashMap<usize, GroupImages>,
    /// Page size in points.
    page: (f32, f32),
    ppp: f32,
    /// Band being redrawn (top, bottom in points), when glyph runs wholly outside it can be
    /// skipped.
    band: Option<(f32, f32)>,
    /// World transform of the page (a whole-pixel shift for `draw_page_at`, else identity).
    base: Matrix3x2,
}

struct GroupJob<'p> {
    key: usize,
    content: &'p [Command],
    mask: Option<&'p SoftMask>,
    blend: Option<BlendMode>,
    transform: Matrix3x2,
    alpha: f32,
}

struct GroupImages {
    /// Opacity image of the soft mask, page space.
    mask: Option<ID2D1Image>,
    /// The group's drawing, page space (recorded only for blend modes).
    content: Option<ID2D1Image>,
    blend: Option<BlendMode>,
}

/// The page target blend modes read their backdrop from.
struct Backdrop<'t> {
    target: &'t ID2D1Bitmap1,
    /// Page-sized copy of the target under blended groups (made on first use).
    scratch: std::cell::RefCell<Option<ID2D1Bitmap1>>,
    /// Band being drawn: left, top, width, height in device pixels.
    band: [u32; 4],
    /// Page size in device pixels.
    size: (u32, u32),
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
            Command::PushGroup { .. } => self.stack.push(Saved::Group(false)),
            Command::PopGroup => match self.stack.pop() {
                Some(Saved::Group(_)) => {}
                _ => return Err("unbalanced group".into()),
            },
            _ => return Ok(false),
        }
        Ok(true)
    }
}

impl Device {
    /// Hand the driver's freed allocations back to the system (`IDXGIDevice3::Trim`, after
    /// clearing the immediate context's state) once page images were dropped; call between
    /// frames. Readers of long documents call it so memory follows the pages kept.
    pub fn trim(&self) {
        use windows::core::Interface as _;
        // SAFETY: plain calls on this device's live COM objects, outside any drawing.
        unsafe {
            if let Ok(ctx) = self.d3d.GetImmediateContext() {
                ctx.ClearState();
                ctx.Flush();
            }
            if let Ok(d) = self
                .d3d
                .cast::<windows::Win32::Graphics::Dxgi::IDXGIDevice3>()
            {
                d.Trim();
            }
        }
    }

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
        self.render_impl(page, fonts, pixels_per_point, images, region, None, [0, 0])
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
            [0, 0],
        )
        .map(|_| ())
    }

    /// [`Device::draw_page`] of the `region` band (left, top, width, height in device pixels
    /// of the page) drawn `shift` device pixels away in `into`: a grid image kept as a ring
    /// (it wraps around when scrolling) draws a newly exposed band where the ring holds it.
    /// The band must land inside `into`; pages with blurred shapes are drawn unshifted only.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_page_at(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
        images: &dyn Fn(&str) -> Option<Bitmap>,
        region: [u32; 4],
        shift: [i32; 2],
        into: &PageBitmap,
    ) -> Result<(), String> {
        let expected = (
            (page.width * pixels_per_point).ceil() as u32,
            (page.height * pixels_per_point).ceil() as u32,
        );
        if expected != (into.width, into.height) {
            return Err("page bitmap size does not match the page".into());
        }
        let [x, y, w, h] = region;
        let inside = |a: u32, l: u32, s: i32, n: u32| {
            i64::from(a) + i64::from(s) >= 0
                && i64::from(a) + i64::from(l) + i64::from(s) <= i64::from(n)
        };
        if !inside(x, w, shift[0], into.width) || !inside(y, h, shift[1], into.height) {
            return Err("shifted band outside the bitmap".into());
        }
        if shift != [0, 0]
            && page
                .commands
                .iter()
                .any(|c| matches!(c, Command::BlurPath { .. } | Command::PushGroup { .. }))
        {
            return Err("blurred shapes are not drawn shifted".into());
        }
        self.render_impl(
            page,
            fonts,
            pixels_per_point,
            images,
            Some(region),
            Some(&into.bitmap),
            shift,
        )
        .map(|_| ())
    }

    #[allow(
        clippy::too_many_lines,
        clippy::chunks_exact_to_as_chunks,
        clippy::too_many_arguments
    )]
    fn render_impl(
        &self,
        page: &PageList,
        fonts: &FontManager,
        pixels_per_point: f32,
        images: &dyn Fn(&str) -> Option<Bitmap>,
        region: Option<[u32; 4]>,
        into: Option<&ID2D1Bitmap1>,
        shift: [i32; 2],
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
        let mut prep = Prep {
            runs: HashMap::new(),
            blurs: HashMap::new(),
            bitmaps: HashMap::new(),
            image_bytes: 0,
            jobs: Vec::new(),
            groups: HashMap::new(),
            page: (page.width, page.height),
            ppp: pixels_per_point,
            band: region.map(|_| {
                (
                    top as f32 / pixels_per_point,
                    (top + band_h) as f32 / pixels_per_point,
                )
            }),
            base: Matrix3x2 {
                M31: shift[0] as f32 / pixels_per_point,
                M32: shift[1] as f32 / pixels_per_point,
                ..IDENTITY
            },
        };
        self.prepare(
            &page.commands,
            State::new(),
            fonts,
            &mut faces,
            images,
            &mut prep,
        )?;
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
            self.context.SetDpi(dpi, dpi);
            self.apply_text_rendering()?;
            // Groups with a blend mode or soft mask, innermost first (they start later).
            let jobs = std::mem::take(&mut prep.jobs);
            let recorded = (|| -> Result<(), String> {
                for job in jobs.iter().rev() {
                    let mask = match job.mask {
                        Some(m) => Some(self.record_mask(m, job.transform, fonts, &prep)?),
                        None => None,
                    };
                    let content = match job.blend {
                        Some(_) => Some(self.record(
                            job.content,
                            State {
                                stack: Vec::new(),
                                transform: job.transform,
                                alpha: job.alpha,
                            },
                            fonts,
                            &prep,
                        )?),
                        None => None,
                    };
                    prep.groups.insert(
                        job.key,
                        GroupImages {
                            mask,
                            content,
                            blend: job.blend,
                        },
                    );
                }
                Ok(())
            })();
            if let Err(e) = recorded {
                self.context.SetDpi(96.0, 96.0);
                return Err(e);
            }
            self.context.SetTarget(&target);
            self.context.BeginDraw();
            self.context.SetTransform(&IDENTITY);
            let banded = region.is_some();
            if banded {
                // Direct2D culls everything outside the band; coordinates are in DIPs (points).
                let (sx, sy) = (shift[0] as f32, shift[1] as f32);
                self.context.PushAxisAlignedClip(
                    &D2D_RECT_F {
                        left: (left as f32 + sx) / pixels_per_point,
                        top: (top as f32 + sy) / pixels_per_point,
                        right: ((left + band_w) as f32 + sx) / pixels_per_point,
                        bottom: ((top + band_h) as f32 + sy) / pixels_per_point,
                    },
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
            }
            // Clear respects the axis-aligned band clip: a reused bitmap keeps its other pixels.
            // A layer bitmap (premultiplied alpha, `create_layer_bitmap`) starts transparent.
            let layer =
                into.is_some_and(|b| b.GetPixelFormat().alphaMode == D2D1_ALPHA_MODE_PREMULTIPLIED);
            let paper = if layer {
                Rgba(0, 0, 0, 0)
            } else {
                Rgba(255, 255, 255, 255)
            };
            self.context.Clear(Some(&color(paper, 1.0)));
            let backdrop = Backdrop {
                target: &target,
                scratch: std::cell::RefCell::new(None),
                band: [left, top, band_w, band_h],
                size: (width, height),
            };
            // A shifted band (`draw_page_at`): the whole page moves by whole device pixels.
            let mut start = State::new();
            start.transform = prep.base;
            self.context.SetTransform(&prep.base);
            let mut sans_on: Option<bool> = None;
            let drawn = self.draw_list(
                &page.commands,
                start,
                fonts,
                &prep,
                Some(&backdrop),
                &mut sans_on,
            );
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

    /// Pass 1 over `commands` (recursing into soft masks): check nesting and geometry,
    /// prepare glyph runs, decode pictures, record blurred shapes and note groups.
    fn prepare<'p>(
        &self,
        commands: &'p [Command],
        mut state: State,
        fonts: &FontManager,
        faces: &mut bo_dwrite::FaceCache,
        images: &dyn Fn(&str) -> Option<Bitmap>,
        prep: &mut Prep<'p>,
    ) -> Result<(), String> {
        for (index, command) in commands.iter().enumerate() {
            if let Command::PushGroup { blend, mask, .. } = command {
                if blend.is_some() || mask.is_some() {
                    let end = group_end(commands, index).ok_or("unbalanced group")?;
                    if let Some(m) = mask {
                        let mut inner = State::new();
                        inner.transform = state.transform;
                        self.prepare(&m.commands, inner, fonts, faces, images, prep)?;
                    }
                    prep.jobs.push(GroupJob {
                        key: key(command),
                        content: &commands[index + 1..end],
                        mask: mask.as_deref(),
                        blend: *blend,
                        transform: state.transform,
                        alpha: state.alpha,
                    });
                }
            }
            if state.apply(command)? {
                continue;
            }
            match command {
                Command::Glyphs {
                    face,
                    size,
                    glyphs,
                    y,
                    ..
                } => {
                    if (*face as usize) >= fonts.len() {
                        return Err("unknown font face".into());
                    }
                    if glyphs.is_empty() {
                        continue;
                    }
                    // A band redraw skips runs that lie wholly above or below the band (no
                    // pixel of them would be drawn): preparing every run of a large page
                    // costs more than drawing the band.
                    if let (Some(band), true) = (prep.band, axis_aligned(&state.transform)) {
                        let (lo, hi) = glyphs
                            .iter()
                            .fold((f32::MAX, f32::MIN), |(a, b), g| (a.min(g.2), b.max(g.2)));
                        let k = state.transform.M22;
                        let (y0, y1) = ((y + lo) * k, (y + hi) * k);
                        let (y0, y1) = (
                            y0.min(y1) + state.transform.M32,
                            y0.max(y1) + state.transform.M32,
                        );
                        let margin = size.abs() * 2.0 * k.abs() + 2.0;
                        if y1 + margin < band.0 || y0 - margin > band.1 {
                            continue;
                        }
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
                    prep.runs.insert(
                        key(command),
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
                    prep.blurs.insert(
                        key(command),
                        self.blur_image(segs, *c, (*radius, *spread), &state)?,
                    );
                }
                Command::Image { image, .. } if !prep.bitmaps.contains_key(image.as_str()) => {
                    let decoded = images(image);
                    let bitmap = match decoded {
                        Some(b) => {
                            prep.image_bytes = prep.image_bytes.saturating_add(b.pixels.len());
                            if prep.image_bytes > MAX_IMAGE_BYTES {
                                return Err("pictures too large".into());
                            }
                            self.bitmap(&b).ok()
                        }
                        None => None,
                    };
                    prep.bitmaps.insert(image.as_str(), bitmap);
                }
                _ => {}
            }
        }
        if !state.stack.is_empty() {
            return Err("unbalanced clip, transform, opacity or group".into());
        }
        Ok(())
    }

    /// Record `commands` into a command list in page space (identity world transform).
    unsafe fn record(
        &self,
        commands: &[Command],
        state: State,
        fonts: &FontManager,
        prep: &Prep<'_>,
    ) -> Result<ID2D1Image, String> {
        // SAFETY: the command list is the target only between BeginDraw and EndDraw here;
        // the target is reset before returning.
        unsafe {
            let list: ID2D1CommandList = self
                .context
                .CreateCommandList()
                .map_err(|e| e.to_string())?;
            self.context.SetTarget(&list);
            self.context.BeginDraw();
            self.context.SetTransform(&state.transform);
            let mut sans_on = None;
            let drawn = self.draw_list(commands, state, fonts, prep, None, &mut sans_on);
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            self.context.SetTransform(&IDENTITY);
            drawn?;
            ended.map_err(|e| e.to_string())?;
            list.Close().map_err(|e| e.to_string())?;
            list.cast().map_err(|e| e.to_string())
        }
    }

    /// The opacity image of a soft mask (ISO 32000-1 §11.6.5.2): its group drawn over the
    /// backdrop colour, luminosity turned into alpha for a luminosity mask.
    unsafe fn record_mask(
        &self,
        mask: &SoftMask,
        transform: Matrix3x2,
        fonts: &FontManager,
        prep: &Prep<'_>,
    ) -> Result<ID2D1Image, String> {
        // SAFETY: as `record`; the effect holds its own reference to its input.
        unsafe {
            let list: ID2D1CommandList = self
                .context
                .CreateCommandList()
                .map_err(|e| e.to_string())?;
            self.context.SetTarget(&list);
            self.context.BeginDraw();
            self.context.SetTransform(&IDENTITY);
            if mask.luminosity {
                let (w, h) = prep.page;
                let brush = self.solid(
                    Rgba(mask.backdrop.0, mask.backdrop.1, mask.backdrop.2, 255),
                    1.0,
                )?;
                self.context.FillRectangle(
                    &D2D_RECT_F {
                        left: -w,
                        top: -h,
                        right: 2.0 * w,
                        bottom: 2.0 * h,
                    },
                    &brush,
                );
            }
            self.context.SetTransform(&transform);
            let state = State {
                stack: Vec::new(),
                transform,
                alpha: 1.0,
            };
            let mut sans_on = None;
            let drawn = self.draw_list(&mask.commands, state, fonts, prep, None, &mut sans_on);
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            self.context.SetTransform(&IDENTITY);
            drawn?;
            ended.map_err(|e| e.to_string())?;
            list.Close().map_err(|e| e.to_string())?;
            let image: ID2D1Image = list.cast().map_err(|e| e.to_string())?;
            if !mask.luminosity {
                return Ok(image);
            }
            let effect = self
                .context
                .CreateEffect(&CLSID_D2D1LuminanceToAlpha)
                .map_err(|e| e.to_string())?;
            effect.SetInput(0, &image, true);
            effect.GetOutput().map_err(|e| e.to_string())
        }
    }

    /// Push a layer without mask or opacity (a knockout group draws into it).
    unsafe fn push_plain_layer(&self) {
        // SAFETY: the parameters hold no COM reference.
        unsafe {
            let params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: INFINITE,
                geometricMask: ManuallyDrop::new(None),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: IDENTITY,
                opacity: 1.0,
                opacityBrush: ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            };
            self.context.PushLayer(&params, None);
        }
    }

    /// Push a layer whose opacity comes from `mask` (page space; the world transform is
    /// identity during the call).
    unsafe fn push_opacity_mask(&self, mask: &ID2D1Image, page: (f32, f32)) -> Result<(), String> {
        // SAFETY: Direct2D adds its own reference to the brush; ours is released right after.
        unsafe {
            let (w, h) = page;
            let brush = self
                .context
                .CreateImageBrush(
                    mask,
                    &D2D1_IMAGE_BRUSH_PROPERTIES {
                        sourceRectangle: D2D_RECT_F {
                            left: 0.0,
                            top: 0.0,
                            right: w,
                            bottom: h,
                        },
                        extendModeX: D2D1_EXTEND_MODE_CLAMP,
                        extendModeY: D2D1_EXTEND_MODE_CLAMP,
                        interpolationMode: D2D1_INTERPOLATION_MODE_LINEAR,
                    },
                    None,
                )
                .map_err(|e| e.to_string())?;
            let mut params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: INFINITE,
                geometricMask: ManuallyDrop::new(None),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: IDENTITY,
                opacity: 1.0,
                opacityBrush: ManuallyDrop::new(Some(brush.cast().map_err(|e| e.to_string())?)),
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            };
            self.context.PushLayer(&params, None);
            ManuallyDrop::drop(&mut params.opacityBrush);
        }
        Ok(())
    }

    /// Composite a group's `content` onto the page target with a blend mode: the target
    /// under the content's bounds is copied out as the backdrop, blended and copied back.
    unsafe fn blend_onto(
        &self,
        content: &ID2D1Image,
        mode: BlendMode,
        b: &Backdrop<'_>,
        ppp: f32,
        clips: &[ClipItem],
    ) -> Result<(), String> {
        let in_clip = !clips.is_empty();
        // SAFETY: the scratch bitmap and effect are owned COM objects; the clip pushed here is
        // popped before returning; the world transform is identity during the call.
        unsafe {
            let bounds = self
                .context
                .GetImageLocalBounds(content)
                .map_err(|e| e.to_string())?;
            let [bl, bt, bw, bh] = b.band;
            let clampx = |v: f32| v.clamp(bl as f32, (bl + bw) as f32) as u32;
            let clampy = |v: f32| v.clamp(bt as f32, (bt + bh) as f32) as u32;
            let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
            let (x0, x1) = (
                clampx(finite(bounds.left * ppp).floor()),
                clampx(finite(bounds.right * ppp).ceil()),
            );
            let (y0, y1) = (
                clampy(finite(bounds.top * ppp).floor()),
                clampy(finite(bounds.bottom * ppp).ceil()),
            );
            if x1 <= x0 || y1 <= y0 {
                return Ok(());
            }
            let scratch = {
                let mut slot = b.scratch.borrow_mut();
                if slot.is_none() {
                    let dpi = 96.0 * ppp;
                    let props = D2D1_BITMAP_PROPERTIES1 {
                        pixelFormat: b.target.GetPixelFormat(),
                        dpiX: dpi,
                        dpiY: dpi,
                        bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                        ..Default::default()
                    };
                    *slot = Some(
                        self.context
                            .CreateBitmap(
                                D2D_SIZE_U {
                                    width: b.size.0,
                                    height: b.size.1,
                                },
                                None,
                                0,
                                &props,
                            )
                            .map_err(|e| e.to_string())?,
                    );
                }
                slot.clone().ok_or("no backdrop bitmap")?
            };
            self.context.Flush(None, None).map_err(|e| e.to_string())?;
            scratch
                .CopyFromBitmap(
                    Some(&D2D_POINT_2U { x: x0, y: y0 }),
                    b.target,
                    Some(&D2D_RECT_U {
                        left: x0,
                        top: y0,
                        right: x1,
                        bottom: y1,
                    }),
                )
                .map_err(|e| e.to_string())?;
            let effect = self
                .context
                .CreateEffect(&CLSID_D2D1Blend)
                .map_err(|e| e.to_string())?;
            // Input 0 is the destination (backdrop), input 1 the source.
            effect.SetInput(
                0,
                &scratch.cast::<ID2D1Image>().map_err(|e| e.to_string())?,
                true,
            );
            effect.SetInput(1, content, true);
            effect
                .SetValue(
                    D2D1_BLEND_PROP_MODE.0 as u32,
                    D2D1_PROPERTY_TYPE_ENUM,
                    &(d2d_blend(mode).0 as u32).to_le_bytes(),
                )
                .map_err(|e| e.to_string())?;
            let output = effect.GetOutput().map_err(|e| e.to_string())?;
            // The backdrop was read with the clips closed; they are opened again so the result
            // lands inside them only.
            self.push_clips(clips);
            self.context.SetTransform(&IDENTITY);
            self.context.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: x0 as f32 / ppp,
                    top: y0 as f32 / ppp,
                    right: x1 as f32 / ppp,
                    bottom: y1 as f32 / ppp,
                },
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
            self.context.DrawImage(
                &output,
                None,
                None,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                // Inside a clip layer the result (backdrop blended with the group) is drawn
                // over the layer, which keeps it to the clip's shape.
                if in_clip {
                    D2D1_COMPOSITE_MODE_SOURCE_OVER
                } else {
                    D2D1_COMPOSITE_MODE_SOURCE_COPY
                },
            );
            self.context.PopAxisAlignedClip();
        }
        Ok(())
    }

    /// Draw `commands` with the current target; `backdrop` is the page target when blend
    /// modes can read what is below (not while recording a group or mask).
    #[allow(clippy::too_many_lines)]
    unsafe fn draw_list(
        &self,
        commands: &[Command],
        mut state: State,
        fonts: &FontManager,
        prep: &Prep<'_>,
        backdrop: Option<&Backdrop<'_>>,
        sans_on: &mut Option<bool>,
    ) -> Result<(), String> {
        let pixels_per_point = prep.ppp;
        // SAFETY: every layer and clip pushed here is popped here (also on error); glyph runs,
        // bitmaps and recorded images in `prep` outlive the draw.
        unsafe {
            let base = state.stack.len();
            let mut clip_items: Vec<ClipItem> = Vec::new();
            // Stack depths of the open knockout groups (ISO 32000-1 §11.4.6.2): inside one,
            // each object replaces what the group painted before (primitive blend Copy in a
            // layer of its own), then the layer composites over the backdrop.
            let mut knockouts: Vec<usize> = Vec::new();
            let drawn = (|| -> Result<(), String> {
                let mut index = 0;
                while index < commands.len() {
                    let command = &commands[index];
                    index += 1;
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
                                clip_items.push(ClipItem {
                                    rect,
                                    geometry: None,
                                    transform: state.transform,
                                });
                            } else {
                                let geometry: ID2D1Geometry = self
                                    .factory
                                    .CreateRectangleGeometry(&rect)
                                    .map_err(|e| e.to_string())?
                                    .cast()
                                    .map_err(|e| e.to_string())?;
                                self.push_mask(geometry.clone());
                                clip_items.push(ClipItem {
                                    rect,
                                    geometry: Some(geometry),
                                    transform: state.transform,
                                });
                            }
                        }
                        Command::PushClipPath { segs } => {
                            let geometry: ID2D1Geometry =
                                self.geometry(segs)?.cast().map_err(|e| e.to_string())?;
                            self.push_mask(geometry.clone());
                            clip_items.push(ClipItem {
                                rect: D2D_RECT_F::default(),
                                geometry: Some(geometry),
                                transform: state.transform,
                            });
                        }
                        Command::PopClip => {
                            clip_items.pop();
                            match state.stack.last() {
                                Some(Saved::AxisClip) => self.context.PopAxisAlignedClip(),
                                _ => self.context.PopLayer(),
                            }
                        }
                        Command::PushGroup {
                            knockout, isolated, ..
                        } => {
                            state.apply(command)?;
                            if *isolated && !*knockout {
                                // ISO 32000-1 §11.4.5: a layer of its own, so the blended
                                // objects inside do not read the page below (they compose
                                // as Normal in it: no backdrop to read in a layer).
                                self.push_plain_layer();
                                if let Some(last) = state.stack.last_mut() {
                                    *last = Saved::Group(true);
                                }
                            }
                            if *knockout {
                                self.push_plain_layer();
                                if let Some(last) = state.stack.last_mut() {
                                    *last = Saved::Group(true);
                                }
                                knockouts.push(state.stack.len());
                                self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                            }
                            let Some(g) = prep.groups.get(&key(command)) else {
                                continue;
                            };
                            if let Some(mask) = &g.mask {
                                self.context.SetTransform(&IDENTITY);
                                let pushed = self.push_opacity_mask(mask, prep.page);
                                self.context.SetTransform(&state.transform);
                                pushed?;
                                if let Some(last) = state.stack.last_mut() {
                                    *last = Saved::Group(true);
                                }
                            }
                            if let (Some(content), Some(mode)) = (&g.content, g.blend) {
                                let end =
                                    group_end(commands, index - 1).ok_or("unbalanced group")?;
                                // The backdrop is read from the page target: inside a soft
                                // mask layer it is not there yet; inside a clip layer it is
                                // (ISO 32000-1 §11.4.7: the clip only limits the result).
                                let in_mask =
                                    state.stack.iter().any(|s| matches!(s, Saved::Group(true)));
                                let in_clip =
                                    state.stack.iter().any(|s| matches!(s, Saved::LayerClip));
                                self.context.SetTransform(&IDENTITY);
                                let done = match backdrop {
                                    Some(b) if !in_mask => {
                                        // A layer hides the target from the copy: close the
                                        // clips (their content is composited), blend, open
                                        // them again.
                                        let clips: &[ClipItem] =
                                            if in_clip { &clip_items } else { &[] };
                                        self.pop_clips(clips);
                                        self.context.SetTransform(&IDENTITY);
                                        self.blend_onto(content, mode, b, pixels_per_point, clips)
                                    }
                                    // No backdrop to read: the group draws as Normal.
                                    _ => {
                                        self.context.DrawImage(
                                            content,
                                            None,
                                            None,
                                            D2D1_INTERPOLATION_MODE_LINEAR,
                                            D2D1_COMPOSITE_MODE_SOURCE_OVER,
                                        );
                                        Ok(())
                                    }
                                };
                                self.context.SetTransform(&state.transform);
                                done?;
                                // Continue at the matching `PopGroup`.
                                index = end;
                            }
                            continue;
                        }
                        Command::PopGroup => {
                            if matches!(state.stack.last(), Some(Saved::Group(true))) {
                                self.context
                                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
                                self.context.PopLayer();
                                if knockouts.last() == Some(&state.stack.len()) {
                                    knockouts.pop();
                                }
                                if !knockouts.is_empty() {
                                    self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                                }
                            }
                        }
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
                        Command::Glyphs {
                            color: c,
                            x,
                            y,
                            face,
                            size,
                            ..
                        } => {
                            let Some(run) = prep.runs.get(&key(command)) else {
                                continue;
                            };
                            // Serif and sans-serif faces each with their own setting.
                            if let Some((serif_p, sans_p)) = self.text_params.borrow().as_ref() {
                                let want = fonts.info(FaceId(*face)).sans
                                    && *size * pixels_per_point < SANS_SMALL_PX;
                                if *sans_on != Some(want) {
                                    self.context.SetTextRenderingParams(if want {
                                        sans_p
                                    } else {
                                        serif_p
                                    });
                                    *sans_on = Some(want);
                                }
                            }
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
                            let Some(image) = prep.blurs.get(&key(command)) else {
                                continue;
                            };
                            // The blur was recorded in page space: draw without the transform.
                            self.context.SetTransform(&prep.base);
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
                            match prep.bitmaps.get(image.as_str()).and_then(Option::as_ref) {
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
                                    // PDF images without /Interpolate (id ending in `#px`,
                                    // bo-pdfread) enlarged twice or more show their pixels.
                                    let m = &local;
                                    let device_w = w * m.M11.hypot(m.M12) * pixels_per_point;
                                    let pixelated = image.ends_with("#px")
                                        && device_w >= 2.0 * (source.right - source.left);
                                    self.context.DrawBitmap(
                                        bitmap,
                                        Some(&dest),
                                        alpha,
                                        if pixelated {
                                            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
                                        } else {
                                            D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC
                                        },
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
            if !knockouts.is_empty() {
                self.context
                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            }
            while state.stack.len() > base {
                match state.stack.pop() {
                    Some(Saved::AxisClip) => self.context.PopAxisAlignedClip(),
                    Some(Saved::LayerClip | Saved::Group(true)) => self.context.PopLayer(),
                    _ => {}
                }
            }
            drawn
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
            // A separate setting for sans-serif faces (default grayscale only).
            let mut sans = None;
            self.text_params.replace(None);
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
                    if tuned.is_none() {
                        sans = Some(SANS_GRAY_PARAMS);
                    }
                    if let Ok(v) = std::env::var("BO_GRAY_MODE") {
                        sans = None;
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
                TextRendering::GrayscaleTuned {
                    gamma,
                    contrast,
                    grid_fit: g,
                } => {
                    (symmetric, grid_fit) = (true, g);
                    mode1 = Some(DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC);
                    (D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, (gamma, contrast, 0.0))
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
                let fit = if grid_fit {
                    DWRITE_GRID_FIT_MODE_ENABLED
                } else {
                    DWRITE_GRID_FIT_MODE_DISABLED
                };
                let make = |gamma: f32, contrast: f32| {
                    f3.CreateCustomRenderingParams(
                        gamma.clamp(1.0, 3.0),
                        contrast.clamp(0.0, 4.0),
                        contrast.clamp(0.0, 4.0),
                        level.clamp(0.0, 1.0),
                        geometry,
                        m,
                        fit,
                    )
                    .map_err(|e| e.to_string())
                };
                let params = make(gamma, contrast)?;
                self.context.SetTextAntialiasMode(antialias);
                self.context.SetTextRenderingParams(&params);
                if let Some((g, c)) = sans {
                    let sans_params = make(g, c)?;
                    self.text_params.replace(Some((
                        params.cast().map_err(|e| e.to_string())?,
                        sans_params.cast().map_err(|e| e.to_string())?,
                    )));
                }
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
    /// Close the clips of `items`, innermost first (their content is composited).
    unsafe fn pop_clips(&self, items: &[ClipItem]) {
        // SAFETY: each item was pushed by `draw_list` and is pushed again by `push_clips`.
        unsafe {
            for c in items.iter().rev() {
                if c.geometry.is_some() {
                    self.context.PopLayer();
                } else {
                    self.context.PopAxisAlignedClip();
                }
            }
        }
    }

    /// Open the clips of `items` again, outermost first, each with its own transform.
    unsafe fn push_clips(&self, items: &[ClipItem]) {
        // SAFETY: as `pop_clips`; the caller sets the transform it needs afterwards.
        unsafe {
            for c in items {
                self.context.SetTransform(&c.transform);
                match &c.geometry {
                    Some(g) => self.push_mask(g.clone()),
                    None => self
                        .context
                        .PushAxisAlignedClip(&c.rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE),
                }
            }
        }
    }

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
