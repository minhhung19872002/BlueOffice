//! Window presentation: a flip-model DXGI swap chain drawn by Direct2D, and GPU-resident page
//! bitmaps composed into it 1:1 (ADR-0018). Coordinates of a [`Frame`] are device pixels.
use super::Device;
use bo_render::{PageList, PathSeg, Rgba};
use windows::core::Interface;
use windows::core::HSTRING;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1Image, D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Brush, ID2D1SolidColorBrush, ID2D1StrokeStyle, D2D1_CAP_STYLE_ROUND,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_LINE_JOIN_ROUND, D2D1_TEXT_ANTIALIAS_MODE_DEFAULT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_RENDERING_MODE,
    DWRITE_RENDERING_MODE_GDI_CLASSIC, DWRITE_RENDERING_MODE_NATURAL, DWRITE_TEXT_METRICS,
    DWRITE_TEXT_RANGE, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1, IDXGISwapChain2, DXGI_PRESENT,
    DXGI_SCALING_NONE, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::System::Threading::WaitForSingleObjectEx;
use windows_numerics::Matrix3x2;

/// A part of a page bitmap given in its device pixels as the rectangle `DrawBitmap` takes:
/// in the bitmap's DIPs (page and layer bitmaps are made at 96 × pixels per point DPI, so a
/// part in pixels would pick a region `pixels per point` times too far and too large).
fn source_rect(page: &PageBitmap, src: [f32; 4]) -> D2D_RECT_F {
    // SAFETY: a plain getter on a live bitmap.
    let size = unsafe { page.bitmap.GetSize() };
    let k = if page.width > 0 && size.width > 0.0 {
        size.width / page.width as f32
    } else {
        1.0
    };
    D2D_RECT_F {
        left: src[0] * k,
        top: src[1] * k,
        right: (src[0] + src[2]) * k,
        bottom: (src[1] + src[3]) * k,
    }
}

const IDENTITY: Matrix3x2 = Matrix3x2 {
    M11: 1.0,
    M12: 0.0,
    M21: 0.0,
    M22: 1.0,
    M31: 0.0,
    M32: 0.0,
};

fn color(c: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(c.0) / 255.0,
        g: f32::from(c.1) / 255.0,
        b: f32::from(c.2) / 255.0,
        a: f32::from(c.3) / 255.0,
    }
}

/// A page kept on the GPU, `width` × `height` device pixels; see [`Device::draw_page`].
pub struct PageBitmap {
    pub(super) bitmap: ID2D1Bitmap1,
    /// Width in device pixels.
    pub width: u32,
    /// Height in device pixels.
    pub height: u32,
}

impl Device {
    /// An opaque bitmap of `width` × `height` device pixels holding a cached part of a window
    /// (drawn with [`Device::draw_layer`], shown with [`Frame::draw_page`]).
    pub fn create_layer(&self, width: u32, height: u32) -> Result<PageBitmap, String> {
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err("layer too large or empty".into());
        }
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            ..Default::default()
        };
        // SAFETY: the context owns the returned bitmap; no pointers are passed.
        let bitmap = unsafe {
            self.context
                .CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &props)
                .map_err(|e| e.to_string())?
        };
        Ok(PageBitmap {
            bitmap,
            width,
            height,
        })
    }

    /// Draw `paint` into `layer` in device pixels, with the interface text settings of a
    /// window frame (outside a window frame).
    pub fn draw_layer(
        &self,
        layer: &PageBitmap,
        paint: impl FnOnce(&Frame<'_>),
    ) -> Result<(), String> {
        // SAFETY: the context targets the layer only between BeginDraw and EndDraw and is
        // reset afterwards; the layer belongs to this device.
        unsafe {
            self.context.SetTarget(&layer.bitmap);
            self.context.SetDpi(96.0, 96.0);
            self.context.BeginDraw();
            self.context.SetTransform(&IDENTITY);
            self.apply_ui_text();
            paint(&Frame { device: self });
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            ended.map_err(|e| e.to_string())
        }
    }

    /// An opaque GPU bitmap for `page` at `pixels_per_point` (its pixels are undefined until drawn).
    pub fn create_page_bitmap(
        &self,
        page: &PageList,
        pixels_per_point: f32,
    ) -> Result<PageBitmap, String> {
        if !(pixels_per_point.is_finite() && pixels_per_point > 0.0) {
            return Err("invalid scale".into());
        }
        let width = (page.width * pixels_per_point).ceil() as u32;
        let height = (page.height * pixels_per_point).ceil() as u32;
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err("page too large or empty".into());
        }
        let dpi = 96.0 * pixels_per_point;
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: dpi,
            dpiY: dpi,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            ..Default::default()
        };
        // SAFETY: the context owns the returned bitmap; no pointers are passed.
        let bitmap = unsafe {
            self.context
                .CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &props)
                .map_err(|e| e.to_string())?
        };
        Ok(PageBitmap {
            bitmap,
            width,
            height,
        })
    }

    /// Copy rectangles of `from` into `to` on the GPU: each `(src, dst)` copies the
    /// `src` rectangle (left, top, width, height in device pixels) of `from` with its top-left
    /// corner at `dst` in `to`. Rectangles are clamped to both bitmaps. Scrolling a grid image
    /// moves the still-visible part this way and redraws only the newly exposed band.
    pub fn copy_page_rects(
        &self,
        from: &PageBitmap,
        to: &PageBitmap,
        rects: &[([u32; 4], [u32; 2])],
    ) -> Result<(), String> {
        use windows::Win32::Graphics::Direct2D::Common::{D2D_POINT_2U, D2D_RECT_U};
        for &([x, y, w, h], [dx, dy]) in rects {
            let w = w
                .min(from.width.saturating_sub(x))
                .min(to.width.saturating_sub(dx));
            let h = h
                .min(from.height.saturating_sub(y))
                .min(to.height.saturating_sub(dy));
            if w == 0 || h == 0 {
                continue;
            }
            let src = D2D_RECT_U {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            };
            let at = D2D_POINT_2U { x: dx, y: dy };
            // SAFETY: both bitmaps belong to this device's context; the rectangles are
            // clamped to them and the pointers live for the call.
            unsafe {
                to.bitmap
                    .CopyFromBitmap(Some(&raw const at), &from.bitmap, Some(&raw const src))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

impl Device {
    /// A GPU bitmap with a transparent background for `page` at `pixels_per_point`: a slide
    /// show layer drawn over the slide below it (document text is grayscale on it).
    pub fn create_layer_bitmap(
        &self,
        page: &PageList,
        pixels_per_point: f32,
    ) -> Result<PageBitmap, String> {
        if !(pixels_per_point.is_finite() && pixels_per_point > 0.0) {
            return Err("invalid scale".into());
        }
        let width = (page.width * pixels_per_point).ceil() as u32;
        let height = (page.height * pixels_per_point).ceil() as u32;
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err("page too large or empty".into());
        }
        let dpi = 96.0 * pixels_per_point;
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: dpi,
            dpiY: dpi,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            ..Default::default()
        };
        // SAFETY: the context owns the returned bitmap; no pointers are passed.
        let bitmap = unsafe {
            self.context
                .CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &props)
                .map_err(|e| e.to_string())?
        };
        Ok(PageBitmap {
            bitmap,
            width,
            height,
        })
    }
}

/// Interface font: the system Segoe UI, like the platform's own controls (ADR-0016 / 0018).
const UI_FONT: &str = "Segoe UI";

/// A font for interface text: family, size in device pixels, weight (400 regular, 600
/// semibold, 700 bold), italic and underline.
#[derive(Debug, Clone, Copy)]
pub struct FontSpec<'a> {
    /// Family name (system font collection).
    pub family: &'a str,
    /// Size in device pixels.
    pub size: f32,
    /// OpenType weight.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
}

impl FontSpec<'_> {
    /// The interface font (Segoe UI), semibold when `bold`.
    pub fn ui(size: f32, bold: bool) -> FontSpec<'static> {
        FontSpec {
            family: UI_FONT,
            size,
            weight: if bold { 600 } else { 400 },
            italic: false,
            underline: false,
        }
    }
}

impl Device {
    fn ui_format(&self, f: &FontSpec<'_>) -> Result<IDWriteTextFormat, String> {
        let key = (f.family.to_owned(), f.size.to_bits(), f.weight, f.italic);
        if let Some(format) = self.ui_formats.borrow().get(&key) {
            return Ok(format.clone());
        }
        // SAFETY: the shared factory and the format are owned COM references.
        let format = unsafe {
            let factory: IDWriteFactory =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| e.to_string())?;
            let format = factory
                .CreateTextFormat(
                    &HSTRING::from(f.family),
                    None,
                    DWRITE_FONT_WEIGHT(i32::from(f.weight)),
                    if f.italic {
                        DWRITE_FONT_STYLE_ITALIC
                    } else {
                        DWRITE_FONT_STYLE_NORMAL
                    },
                    DWRITE_FONT_STRETCH_NORMAL,
                    f.size.max(1.0),
                    &HSTRING::from("vi-VN"),
                )
                .map_err(|e| e.to_string())?;
            format
                .SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)
                .map_err(|e| e.to_string())?;
            format
        };
        // Bound the cache: previews of many families must not grow it without limit.
        let mut cache = self.ui_formats.borrow_mut();
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(key, format.clone());
        Ok(format)
    }

    /// The interface brush set to `c` (one brush reused by every interface draw).
    fn ui_solid(&self, c: Rgba) -> Option<ID2D1SolidColorBrush> {
        let mut slot = self.ui_brush.borrow_mut();
        // SAFETY: the brush belongs to this device's context; colours are plain values.
        unsafe {
            if let Some(b) = slot.as_ref() {
                b.SetColor(&color(c));
                return Some(b.clone());
            }
            let b = self.context.CreateSolidColorBrush(&color(c), None).ok()?;
            *slot = Some(b.clone());
            Some(b)
        }
    }

    /// Round caps and joins for interface paths, created once.
    fn ui_stroke_style(&self) -> Option<ID2D1StrokeStyle> {
        let mut slot = self.ui_stroke.borrow_mut();
        if let Some(s) = slot.as_ref() {
            return Some(s.clone());
        }
        let s = self
            .stroke_style(1.0, &[], D2D1_CAP_STYLE_ROUND, D2D1_LINE_JOIN_ROUND)
            .ok()?;
        *slot = Some(s.clone());
        Some(s)
    }

    fn ui_layout(&self, text: &str, f: &FontSpec<'_>) -> Result<IDWriteTextLayout, String> {
        // Interface labels repeat every frame: reuse their layouts (bounded cache).
        let key = (
            text.to_owned(),
            f.family.to_owned(),
            f.size.to_bits(),
            f.weight,
            f.italic,
            f.underline,
        );
        if let Some(l) = self.ui_layouts.borrow().get(&key) {
            return Ok(l.clone());
        }
        let layout = self.new_ui_layout(text, f)?;
        let mut cache = self.ui_layouts.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(key, layout.clone());
        Ok(layout)
    }

    /// Path geometry of interface drawing `segs`, reused across frames (bounded cache).
    fn ui_geometry(
        &self,
        segs: &[PathSeg],
    ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1PathGeometry1, String> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for s in segs {
            match *s {
                PathSeg::M(a, b) => (0u8, a.to_bits(), b.to_bits()).hash(&mut h),
                PathSeg::L(a, b) => (1u8, a.to_bits(), b.to_bits()).hash(&mut h),
                PathSeg::Q(a, b, c, d) => {
                    (2u8, a.to_bits(), b.to_bits(), c.to_bits(), d.to_bits()).hash(&mut h);
                }
                PathSeg::C(a, b, c, d, e, f) => {
                    (3u8, [a, b, c, d, e, f].map(f32::to_bits)).hash(&mut h)
                }
                PathSeg::Z => 4u8.hash(&mut h),
            }
        }
        let key = h.finish();
        if let Some(g) = self.ui_geoms.borrow().get(&key) {
            return Ok(g.clone());
        }
        let g = self.geometry(segs)?;
        let mut cache = self.ui_geoms.borrow_mut();
        if cache.len() > 8192 {
            cache.clear();
        }
        cache.insert(key, g.clone());
        Ok(g)
    }

    fn new_ui_layout(&self, text: &str, f: &FontSpec<'_>) -> Result<IDWriteTextLayout, String> {
        let format = self.ui_format(f)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: `wide` outlives the call (DirectWrite copies it); the layout is owned.
        // GDI-compatible (classic) measuring keeps small interface text on whole pixels like
        // the system's own controls, which is crisper than natural layout at 9–11 pt.
        unsafe {
            let factory: IDWriteFactory =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(|e| e.to_string())?;
            let layout = factory
                .CreateGdiCompatibleTextLayout(&wide, &format, 10_000.0, 1_000.0, 1.0, None, false)
                .map_err(|e| e.to_string())?;
            if f.underline {
                let _ = layout.SetUnderline(
                    true,
                    DWRITE_TEXT_RANGE {
                        startPosition: 0,
                        length: wide.len() as u32,
                    },
                );
            }
            Ok(layout)
        }
    }

    /// Text antialiasing for interface text: the system's gamma, contrast, ClearType level and
    /// pixel geometry, rendered in GDI classic mode (stems and baselines on whole pixels, as
    /// the platform's own controls draw 9–11 pt text). `BLUEOFFICE_UI_TEXT=natural|system`
    /// selects DirectWrite's natural mode or the unmodified defaults for comparison.
    pub(crate) fn apply_ui_text(&self) {
        // SAFETY: called between BeginDraw and EndDraw; parameters are owned COM references.
        unsafe {
            self.context
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_DEFAULT);
            let mode: Option<DWRITE_RENDERING_MODE> =
                match std::env::var("BLUEOFFICE_UI_TEXT").as_deref() {
                    Ok("system") => None,
                    Ok("natural") => Some(DWRITE_RENDERING_MODE_NATURAL),
                    _ => Some(DWRITE_RENDERING_MODE_GDI_CLASSIC),
                };
            let params = mode.and_then(|mode| {
                if let Some(p) = self.ui_params.borrow().as_ref() {
                    return Some(p.clone());
                }
                let factory: IDWriteFactory =
                    DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
                let base = factory.CreateRenderingParams().ok()?;
                let geometry = self
                    .pixel_geometry
                    .get()
                    .unwrap_or_else(|| base.GetPixelGeometry());
                let p = factory
                    .CreateCustomRenderingParams(
                        base.GetGamma(),
                        base.GetEnhancedContrast(),
                        base.GetClearTypeLevel(),
                        geometry,
                        mode,
                    )
                    .ok()?;
                *self.ui_params.borrow_mut() = Some(p.clone());
                Some(p)
            });
            self.context.SetTextRenderingParams(params.as_ref());
        }
    }

    /// Width and height in device pixels of interface text at `size` pixels.
    pub fn measure_text(&self, text: &str, size: f32, bold: bool) -> (f32, f32) {
        self.measure_font(text, &FontSpec::ui(size, bold))
    }

    /// Width and height in device pixels of `text` in font `f`.
    pub fn measure_font(&self, text: &str, f: &FontSpec<'_>) -> (f32, f32) {
        let Ok(layout) = self.ui_layout(text, f) else {
            return (0.0, f.size);
        };
        let mut m = DWRITE_TEXT_METRICS::default();
        // SAFETY: the out pointer refers to a live metrics struct.
        unsafe {
            if layout.GetMetrics(&mut m).is_err() {
                return (0.0, f.size);
            }
        }
        (m.widthIncludingTrailingWhitespace, m.height)
    }
}

/// A window's swap chain. Presenting waits on the frame-latency object first, so a frame is
/// drawn with the newest input and shown on the next vertical blank (one frame of latency).
pub struct WindowSurface {
    swap: IDXGISwapChain1,
    waitable: HANDLE,
    target: Option<ID2D1Bitmap1>,
    width: u32,
    height: u32,
    /// `Present` sync interval: 1 (default) shows a frame on the vertical blank after it is
    /// presented; 0 hands it to the compositor at once (one refresh sooner on screen; the
    /// frame-latency wait still paces drawing).
    interval: u32,
    /// DirectComposition device, target and visual holding the swap chain
    /// ([`WindowSurface::new_composition`]); `None` for a swap chain of the window itself.
    comp: Option<(
        windows::Win32::Graphics::DirectComposition::IDCompositionDevice,
        windows::Win32::Graphics::DirectComposition::IDCompositionTarget,
        windows::Win32::Graphics::DirectComposition::IDCompositionVisual,
    )>,
}

const FLAGS: DXGI_SWAP_CHAIN_FLAG = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;

impl WindowSurface {
    /// Create a swap chain for the window `hwnd` (its raw handle) of `width` × `height` pixels.
    pub fn new(device: &Device, hwnd: isize, width: u32, height: u32) -> Result<Self, String> {
        let (width, height) = (width.max(1), height.max(1));
        // SAFETY: COM interfaces are owned and released on drop; `hwnd` is a live window of the
        // caller's thread. The waitable handle is owned by the swap chain.
        unsafe {
            let dxgi: IDXGIDevice = device.d3d.cast().map_err(|e| e.to_string())?;
            let adapter = dxgi.GetAdapter().map_err(|e| e.to_string())?;
            let factory: IDXGIFactory2 = adapter.GetParent().map_err(|e| e.to_string())?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_NONE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: FLAGS.0 as u32,
                ..Default::default()
            };
            let swap = factory
                .CreateSwapChainForHwnd(&device.d3d, HWND(hwnd as _), &desc, None, None)
                .map_err(|e| e.to_string())?;
            let swap2: IDXGISwapChain2 = swap.cast().map_err(|e| e.to_string())?;
            swap2.SetMaximumFrameLatency(1).map_err(|e| e.to_string())?;
            let waitable = swap2.GetFrameLatencyWaitableObject();
            let mut surface = Self {
                swap,
                waitable,
                target: None,
                width,
                height,
                interval: 1,
                comp: None,
            };
            surface.bind(device)?;
            Ok(surface)
        }
    }

    /// [`WindowSurface::new`] with the swap chain made for composition and shown through a
    /// DirectComposition visual on the window (topmost), instead of a swap chain of the
    /// window: the compositor takes the frame without the window's redirection surface.
    pub fn new_composition(
        device: &Device,
        hwnd: isize,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        use windows::Win32::Graphics::DirectComposition::{
            DCompositionCreateDevice, IDCompositionDevice,
        };
        let (width, height) = (width.max(1), height.max(1));
        // SAFETY: as in `new`; the composition objects are owned by the surface and released
        // on drop, after the swap chain they show.
        unsafe {
            let dxgi: IDXGIDevice = device.d3d.cast().map_err(|e| e.to_string())?;
            let adapter = dxgi.GetAdapter().map_err(|e| e.to_string())?;
            let factory: IDXGIFactory2 = adapter.GetParent().map_err(|e| e.to_string())?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: FLAGS.0 as u32,
                ..Default::default()
            };
            let swap = factory
                .CreateSwapChainForComposition(&device.d3d, &desc, None)
                .map_err(|e| e.to_string())?;
            let swap2: IDXGISwapChain2 = swap.cast().map_err(|e| e.to_string())?;
            swap2.SetMaximumFrameLatency(1).map_err(|e| e.to_string())?;
            let waitable = swap2.GetFrameLatencyWaitableObject();
            let dcomp: IDCompositionDevice =
                DCompositionCreateDevice(&dxgi).map_err(|e| e.to_string())?;
            let target = dcomp
                .CreateTargetForHwnd(HWND(hwnd as _), true)
                .map_err(|e| e.to_string())?;
            let visual = dcomp.CreateVisual().map_err(|e| e.to_string())?;
            visual.SetContent(&swap).map_err(|e| e.to_string())?;
            target.SetRoot(&visual).map_err(|e| e.to_string())?;
            dcomp.Commit().map_err(|e| e.to_string())?;
            let mut surface = Self {
                swap,
                waitable,
                target: None,
                width,
                height,
                interval: 1,
                comp: Some((dcomp, target, visual)),
            };
            surface.bind(device)?;
            Ok(surface)
        }
    }

    /// Whether the swap chain is shown through DirectComposition.
    pub fn is_composition(&self) -> bool {
        self.comp.is_some()
    }

    /// Present with sync interval `interval` (0 or 1; see the field).
    pub fn set_sync_interval(&mut self, interval: u32) {
        self.interval = interval.min(1);
    }

    /// Size of the back buffer in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn bind(&mut self, device: &Device) -> Result<(), String> {
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };
        // SAFETY: the back buffer surface is owned; the bitmap keeps its own reference.
        unsafe {
            let buffer: IDXGISurface = self.swap.GetBuffer(0).map_err(|e| e.to_string())?;
            self.target = Some(
                device
                    .context
                    .CreateBitmapFromDxgiSurface(&buffer, Some(&props))
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }

    /// Resize the back buffers to the window's new client size.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) -> Result<(), String> {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.width, self.height) {
            return Ok(());
        }
        // Every reference to the back buffer must be released before ResizeBuffers.
        self.target = None;
        // SAFETY: the context no longer targets the back buffer; the swap chain is owned.
        unsafe {
            device.context.SetTarget(None::<&ID2D1Image>);
            self.swap
                .ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, FLAGS)
                .map_err(|e| e.to_string())?;
        }
        (self.width, self.height) = (width, height);
        self.bind(device)
    }

    /// Draw one frame with `paint` and present it.
    pub fn frame(&mut self, device: &Device, paint: impl FnOnce(&Frame<'_>)) -> Result<(), String> {
        let target = self.target.as_ref().ok_or("no back buffer")?;
        // SAFETY: the waitable handle belongs to the live swap chain; the context targets the
        // back buffer only between BeginDraw and EndDraw and is reset afterwards.
        unsafe {
            // Wait until the swap chain can take a frame (at most a few frames' time).
            let _ = WaitForSingleObjectEx(self.waitable, 100, true);
            device.context.SetTarget(target);
            device.context.SetDpi(96.0, 96.0);
            device.context.BeginDraw();
            device.context.SetTransform(&IDENTITY);
            // Interface text: system ClearType settings, drawn like the platform's own controls;
            // pages carry their own pixels.
            device.apply_ui_text();
            paint(&Frame { device });
            let ended = device.context.EndDraw(None, None);
            device.context.SetTarget(None::<&ID2D1Image>);
            ended.map_err(|e| e.to_string())?;
            self.swap
                .Present(self.interval, DXGI_PRESENT(0))
                .ok()
                .map_err(|e| e.to_string())
        }
    }
}

/// Drawing commands of one frame, in device pixels.
pub struct Frame<'a> {
    device: &'a Device,
}

impl Frame<'_> {
    /// Fill the whole frame.
    pub fn clear(&self, c: Rgba) {
        // SAFETY: called between BeginDraw and EndDraw on the frame's context.
        unsafe { self.device.context.Clear(Some(&color(c))) }
    }

    /// Fill a rectangle.
    pub fn fill_rect(&self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        // SAFETY: the brush is owned; drawing happens inside the frame's BeginDraw/EndDraw.
        unsafe {
            if let Some(brush) = self.device.ui_solid(c) {
                self.device.context.FillRectangle(
                    &D2D_RECT_F {
                        left: x,
                        top: y,
                        right: x + w,
                        bottom: y + h,
                    },
                    &brush,
                );
            }
        }
    }

    /// Restrict later drawing to a rectangle until [`Frame::pop_clip`].
    pub fn push_clip(&self, x: f32, y: f32, w: f32, h: f32) {
        // SAFETY: inside BeginDraw/EndDraw; the caller pairs it with `pop_clip`.
        unsafe {
            self.device.context.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + w,
                    bottom: y + h,
                },
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
        }
    }

    /// Undo the last [`Frame::push_clip`].
    pub fn pop_clip(&self) {
        // SAFETY: paired with a previous `push_clip` in the same frame.
        unsafe { self.device.context.PopAxisAlignedClip() }
    }

    /// Draw interface text with its layout box's top-left corner at (`x`, `y`).
    pub fn text(&self, text: &str, x: f32, y: f32, size: f32, bold: bool, c: Rgba) {
        self.text_font(text, x, y, &FontSpec::ui(size, bold), c);
    }

    /// Draw text in font `f` with its layout box's top-left corner at (`x`, `y`).
    pub fn text_font(&self, text: &str, x: f32, y: f32, f: &FontSpec<'_>, c: Rgba) {
        let Ok(layout) = self.device.ui_layout(text, f) else {
            return;
        };
        // SAFETY: the layout and brush are owned; drawing happens inside the frame.
        unsafe {
            if let Some(brush) = self.device.ui_solid(c) {
                self.device.context.DrawTextLayout(
                    windows_numerics::Vector2 {
                        X: x.round(),
                        Y: y.round(),
                    },
                    &layout,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
    }

    /// Fill and / or stroke (round caps and joins) a path in device pixels.
    pub fn path(&self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) {
        let Ok(geometry) = self.device.ui_geometry(segs) else {
            return;
        };
        // SAFETY: geometry, brushes and stroke style are owned; drawing happens in the frame.
        unsafe {
            if let Some(c) = fill {
                if let Some(brush) = self.device.ui_solid(c) {
                    self.device
                        .context
                        .FillGeometry(&geometry, &brush, None::<&ID2D1Brush>);
                }
            }
            if let Some((c, width)) = stroke {
                if let (Some(brush), Some(style)) =
                    (self.device.ui_solid(c), self.device.ui_stroke_style())
                {
                    self.device
                        .context
                        .DrawGeometry(&geometry, &brush, width, &style);
                }
            }
        }
    }

    /// Start interface layer `id` covering `rect` (x, y, width, height in device pixels) with
    /// content `key` (a hash of what it shows). `true`: the cached image already shows `key`
    /// and was drawn, skip painting. `false`: later drawing goes into the layer, in window
    /// coordinates, until [`Frame::layer_end`] puts it on the frame. Keystrokes redraw only
    /// the document this way, not the unchanged ribbon (most of an interface frame's time).
    pub fn layer_begin(&self, id: usize, rect: [i32; 4], key: u64) -> bool {
        let [x, y, w, h] = rect;
        if w <= 0 || h <= 0 || w > 16_384 || h > 16_384 {
            return false;
        }
        let mut layers = self.device.ui_layers.borrow_mut();
        if let Some((k, r, bitmap)) = layers.get(&id) {
            if *k == key && *r == rect {
                draw_bitmap_at(self.device, bitmap, x, y, w, h);
                return true;
            }
        }
        let reuse = layers
            .get(&id)
            .filter(|(_, r, _)| r[2] == w && r[3] == h)
            .map(|(_, _, b)| b.clone());
        let bitmap = match reuse {
            Some(b) => b,
            None => {
                let props = D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                    ..Default::default()
                };
                // SAFETY: the context owns the returned bitmap; no pointers are passed.
                let created = unsafe {
                    self.device.context.CreateBitmap(
                        D2D_SIZE_U {
                            width: w as u32,
                            height: h as u32,
                        },
                        None,
                        0,
                        &props,
                    )
                };
                match created {
                    Ok(b) => b,
                    Err(_) => return false,
                }
            }
        };
        // SAFETY: inside the frame's BeginDraw/EndDraw a device context may change its target;
        // the previous target is kept and restored by `layer_end`.
        unsafe {
            *self.device.ui_layer_target.borrow_mut() = self.device.context.GetTarget().ok();
            self.device.context.SetTarget(&bitmap);
            self.device
                .context
                .SetTransform(&Matrix3x2::translation(-x as f32, -y as f32));
        }
        layers.insert(id, (key, rect, bitmap));
        false
    }

    /// Finish the layer started by [`Frame::layer_begin`] and draw it on the frame.
    pub fn layer_end(&self, id: usize) {
        let prev = self.device.ui_layer_target.borrow_mut().take();
        // SAFETY: restores the frame's own target taken in `layer_begin`.
        unsafe {
            self.device.context.SetTarget(prev.as_ref());
            self.device.context.SetTransform(&IDENTITY);
        }
        if let Some((_, [x, y, w, h], bitmap)) = self.device.ui_layers.borrow().get(&id) {
            draw_bitmap_at(self.device, bitmap, *x, *y, *w, *h);
        }
    }

    /// Forget interface layer `id` (drawn again next time).
    pub fn layer_forget(&self, id: usize) {
        self.device.ui_layers.borrow_mut().remove(&id);
    }

    /// Draw the `src` part (device pixels of the bitmap) of a page bitmap into `dest` (device
    /// pixels; scaled smoothly when the sizes differ) at `opacity` 0–1 (slide show effects).
    pub fn draw_page_part(&self, page: &PageBitmap, src: [f32; 4], dest: [f32; 4], opacity: f32) {
        if dest[2] <= 0.0 || dest[3] <= 0.0 || src[2] <= 0.0 || src[3] <= 0.0 {
            return;
        }
        let d = D2D_RECT_F {
            left: dest[0],
            top: dest[1],
            right: dest[0] + dest[2],
            bottom: dest[1] + dest[3],
        };
        let s = source_rect(page, src);
        let same = (src[2] - dest[2]).abs() < 0.01 && (src[3] - dest[3]).abs() < 0.01;
        let mode = if same && dest[0].fract() == 0.0 && dest[1].fract() == 0.0 {
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
        } else {
            D2D1_INTERPOLATION_MODE_LINEAR
        };
        // SAFETY: the bitmap belongs to the same device; drawing happens inside the frame.
        unsafe {
            self.device.context.DrawBitmap(
                &page.bitmap,
                Some(&d),
                opacity.clamp(0.0, 1.0),
                mode,
                Some(&s),
                None,
            );
        }
    }

    /// Draw like [`Frame::draw_page_part`], turned `degrees` clockwise about the centre of
    /// `dest` (slide show spin effects, Morph).
    pub fn draw_page_part_rotated(
        &self,
        page: &PageBitmap,
        src: [f32; 4],
        dest: [f32; 4],
        opacity: f32,
        degrees: f32,
    ) {
        if degrees.abs() < 0.01 {
            self.draw_page_part(page, src, dest, opacity);
            return;
        }
        if dest[2] <= 0.0 || dest[3] <= 0.0 || src[2] <= 0.0 || src[3] <= 0.0 {
            return;
        }
        let (cx, cy) = (dest[0] + dest[2] / 2.0, dest[1] + dest[3] / 2.0);
        let (s, c) = degrees.to_radians().sin_cos();
        // Row-vector convention: x' = x·M11 + y·M21 + M31, y' = x·M12 + y·M22 + M32.
        let m = Matrix3x2 {
            M11: c,
            M12: s,
            M21: -s,
            M22: c,
            M31: cx - c * cx + s * cy,
            M32: cy - s * cx - c * cy,
        };
        let d = D2D_RECT_F {
            left: dest[0],
            top: dest[1],
            right: dest[0] + dest[2],
            bottom: dest[1] + dest[3],
        };
        let sr = source_rect(page, src);
        // SAFETY: the bitmap belongs to the same device; drawing happens inside the frame, and
        // the transform is put back to identity (what every other draw of the frame expects).
        unsafe {
            self.device.context.SetTransform(&m);
            self.device.context.DrawBitmap(
                &page.bitmap,
                Some(&d),
                opacity.clamp(0.0, 1.0),
                D2D1_INTERPOLATION_MODE_LINEAR,
                Some(&sr),
                None,
            );
            self.device.context.SetTransform(&IDENTITY);
        }
    }

    /// Draw the `src` rectangle (left, top, width, height in device pixels of the bitmap) of a
    /// page bitmap 1:1 with its top-left corner at device pixel (`x`, `y`): the pieces of a
    /// grid image kept as a ring.
    pub fn draw_page_rect(&self, page: &PageBitmap, src: [u32; 4], x: i32, y: i32) {
        let [l, t, w, h] = src;
        if w == 0 || h == 0 {
            return;
        }
        let dest = D2D_RECT_F {
            left: x as f32,
            top: y as f32,
            right: (x + w as i32) as f32,
            bottom: (y + h as i32) as f32,
        };
        // SAFETY: the bitmap belongs to the same device; drawing happens inside the frame.
        unsafe {
            // The source rectangle is in the bitmap's DIPs (page bitmaps carry their scale).
            let (mut dx, mut dy) = (96.0_f32, 96.0_f32);
            page.bitmap.GetDpi(&raw mut dx, &raw mut dy);
            let (kx, ky) = (96.0 / dx.max(1.0), 96.0 / dy.max(1.0));
            let s = D2D_RECT_F {
                left: l as f32 * kx,
                top: t as f32 * ky,
                right: (l + w) as f32 * kx,
                bottom: (t + h) as f32 * ky,
            };
            self.device.context.DrawBitmap(
                &page.bitmap,
                Some(&dest),
                1.0,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                Some(&s),
                None,
            );
        }
    }

    /// Draw a page bitmap 1:1 with its top-left corner at device pixel (`x`, `y`).
    pub fn draw_page(&self, page: &PageBitmap, x: i32, y: i32) {
        let dest = D2D_RECT_F {
            left: x as f32,
            top: y as f32,
            right: (x + page.width as i32) as f32,
            bottom: (y + page.height as i32) as f32,
        };
        // SAFETY: the bitmap belongs to the same device; drawing happens inside the frame.
        unsafe {
            self.device.context.DrawBitmap(
                &page.bitmap,
                Some(&dest),
                1.0,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
                None,
            );
        }
    }
}

/// Draw `bitmap` 1:1 at (`x`, `y`) with size `w` × `h` device pixels.
fn draw_bitmap_at(device: &Device, bitmap: &ID2D1Bitmap1, x: i32, y: i32, w: i32, h: i32) {
    let dest = D2D_RECT_F {
        left: x as f32,
        top: y as f32,
        right: (x + w) as f32,
        bottom: (y + h) as f32,
    };
    // SAFETY: the bitmap belongs to the same device; drawing happens inside the frame.
    unsafe {
        device.context.DrawBitmap(
            bitmap,
            Some(&dest),
            1.0,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            None,
            None,
        );
    }
}

#[cfg(test)]
mod visual_tests {
    use super::*;
    use bo_ui::{
        DocView, DocWindow, Font, Icons, Input, PageStore, Painter, Ribbon, Strings, TextMeasure,
        TextStyle, Theme,
    };
    use std::collections::HashMap;
    use windows::Win32::Graphics::Direct2D::{D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_MAP_OPTIONS_READ};

    struct Measure<'a>(&'a Device);
    impl TextMeasure for Measure<'_> {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            self.0.measure_text(text, style.size, style.bold)
        }
        fn measure_font(&self, text: &str, font: &Font<'_>) -> (f32, f32) {
            self.0.measure_font(
                text,
                &FontSpec {
                    family: font.family,
                    size: font.size,
                    weight: font.weight,
                    italic: font.italic,
                    underline: font.underline,
                },
            )
        }
    }
    struct UiPainter<'a, 'b>(&'a Frame<'b>, Option<&'a HashMap<usize, PageBitmap>>);
    impl Painter for UiPainter<'_, '_> {
        fn clear(&mut self, c: Rgba) {
            self.0.clear(c);
        }
        fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
            self.0.fill_rect(x, y, w, h, c);
        }
        fn page(&mut self, index: usize, x: i32, y: i32) {
            if let Some(page) = self.1.and_then(|pages| pages.get(&index)) {
                self.0.draw_page(page, x, y);
            }
        }
        fn text(&mut self, s: &str, x: f32, y: f32, style: TextStyle, c: Rgba) {
            self.0.text(s, x, y, style.size, style.bold, c);
        }
        fn text_font(&mut self, s: &str, x: f32, y: f32, f: &Font<'_>, c: Rgba) {
            self.0.text_font(
                s,
                x,
                y,
                &FontSpec {
                    family: f.family,
                    size: f.size,
                    weight: f.weight,
                    italic: f.italic,
                    underline: f.underline,
                },
                c,
            );
        }
        fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) {
            self.0.path(segs, fill, stroke);
        }
        fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
            self.0.push_clip(x, y, w, h);
        }
        fn pop_clip(&mut self) {
            self.0.pop_clip();
        }
    }

    struct UiPages<'a> {
        device: &'a Device,
        pages: HashMap<usize, PageBitmap>,
    }
    impl PageStore for UiPages<'_> {
        fn update(
            &mut self,
            index: usize,
            list: &PageList,
            engine: &bo_engine::Engine,
            scale: f32,
            _: Option<[u32; 4]>,
        ) -> Result<(), String> {
            let page = self.device.create_page_bitmap(list, scale)?;
            let pictures = |id: &str| {
                let bytes = engine.media_resolved(id)?;
                let (width, height, pixels) = bo_raster::decode_premultiplied(&bytes)?;
                Some(crate::Bitmap {
                    width,
                    height,
                    pixels,
                })
            };
            self.device
                .draw_page(list, engine.fonts(), scale, &pictures, None, &page)?;
            self.pages.insert(index, page);
            Ok(())
        }
        fn remove(&mut self, index: usize) {
            self.pages.remove(&index);
        }
    }

    /// Render a complete native window and document without a visible window.
    #[test]
    #[ignore = "writes an offscreen window image for visual review"]
    fn window_offscreen() {
        let device = Device::new().expect("Direct2D hardware or WARP");
        device.set_text_rendering(crate::TextRendering::document_default());
        let mut engine = bo_engine::Engine::with_bundled_fonts();
        let font_cache = std::env::var_os("LOCALAPPDATA").map(|dir| {
            std::path::PathBuf::from(dir)
                .join("BlueOffice")
                .join("system-fonts.txt")
        });
        engine.set_system_fonts(bo_engine::SystemFontIndex::scan_cached(
            &bo_engine::SystemFontIndex::default_dirs(),
            font_cache.as_deref(),
        ));
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/phase1-sample.docx"
        );
        engine
            .open(&std::fs::read(path).unwrap(), "phase1-sample.docx")
            .unwrap();
        let mut view = DocView::new(engine).unwrap();
        // BO_WINDOW_ZOOM=1.5 renders the document at another zoom.
        if let Some(z) = std::env::var("BO_WINDOW_ZOOM")
            .ok()
            .and_then(|z| z.parse().ok())
        {
            view.set_zoom(z);
        }
        // BO_WINDOW_COMMANDS='[{"cmd":"selectAll"}, …]' edits the document before rendering.
        if let Ok(json) = std::env::var("BO_WINDOW_COMMANDS") {
            let cmds: Vec<bo_engine::edit::Command> = serde_json::from_str(&json).unwrap();
            for cmd in cmds {
                assert!(view.run_command(cmd), "command failed");
            }
        }
        let mut window = DocWindow::new(view, "en-US", "phase1-sample.docx");
        let measure = Measure(&device);
        let context_test = std::env::var_os("BO_WINDOW_CONTEXT").is_some();
        let (width, height, scale) = if context_test {
            (800_u32, 600_u32, 1.0)
        } else {
            (1708, 1125, 1.25)
        };
        window.handle(Input::Scale(scale), &measure);
        window.handle(
            Input::Resize {
                width: width as f32,
                height: height as f32,
            },
            &measure,
        );
        // BO_WINDOW_WHEEL=-3000 scrolls the document (wheel delta units) before rendering.
        if let Some(dy) = std::env::var("BO_WINDOW_WHEEL")
            .ok()
            .and_then(|d| d.parse().ok())
        {
            window.handle(Input::Wheel(dy), &measure);
        }
        let mut pages = UiPages {
            device: &device,
            pages: HashMap::new(),
        };
        window.prepare(&mut pages).unwrap();
        if context_test {
            window.handle(
                Input::Context {
                    x: width as f32 - 20.0,
                    y: height as f32 - 60.0,
                },
                &measure,
            );
        }
        let size = D2D_SIZE_U { width, height };
        let pixel = D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_IGNORE,
        };
        let target_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: pixel,
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            ..Default::default()
        };
        let read_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: pixel,
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };
        // SAFETY: the bitmaps and context are owned and the mapped pixels are read before Unmap.
        unsafe {
            let target = device
                .context
                .CreateBitmap(size, None, 0, &target_props)
                .unwrap();
            device.context.SetTarget(&target);
            device.context.SetDpi(96.0, 96.0);
            device.context.BeginDraw();
            device.context.SetTransform(&IDENTITY);
            device.apply_ui_text();
            let frame = Frame { device: &device };
            window.paint(&mut UiPainter(&frame, Some(&pages.pages)));
            device.context.EndDraw(None, None).unwrap();
            device.context.SetTarget(None::<&ID2D1Image>);
            let read = device
                .context
                .CreateBitmap(size, None, 0, &read_props)
                .unwrap();
            read.CopyFromBitmap(None, &target, None).unwrap();
            let mapped = read.Map(D2D1_MAP_OPTIONS_READ).unwrap();
            assert!(!mapped.bits.is_null() && mapped.pitch as usize >= width as usize * 4);
            let mut ppm = format!("P6 {width} {height} 255\n").into_bytes();
            for y in 0..height as usize {
                let row = std::slice::from_raw_parts(
                    mapped.bits.add(y * mapped.pitch as usize),
                    width as usize * 4,
                );
                for bgra in row.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&[bgra[2], bgra[1], bgra[0]]);
                }
            }
            read.Unmap().unwrap();
            let output = std::env::var("BO_RIBBON_OUT").unwrap_or_else(|_| ".".into());
            let name = if context_test {
                "window-context"
            } else {
                "window-125"
            };
            std::fs::write(format!("{output}/{name}.ppm"), ppm).unwrap();
        }
    }

    /// Diagnostic image of the same Direct2D/DirectWrite ribbon used by the Windows host.
    #[test]
    #[ignore = "writes offscreen ribbon images for visual review"]
    fn ribbon_offscreen() {
        let device = Device::new().expect("Direct2D hardware or WARP");
        let mut engine = bo_engine::Engine::with_bundled_fonts();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/phase1-sample.docx"
        );
        engine
            .open(&std::fs::read(path).unwrap(), "sample.docx")
            .unwrap();
        let mut view = DocView::new(engine).unwrap();
        let styles = view.styles();
        let strings = Strings::load("vi-VN");
        let icons = Icons::load();
        let theme = Theme::light("doc");
        let output = std::env::var("BO_RIBBON_OUT").unwrap_or_else(|_| ".".into());
        let configs = [(720_u32, 1.0_f32, 0_usize), (1366, 1.0, 0)]
            .into_iter()
            .chain((0..8).map(|tab| (1708, 1.25, tab)));
        for (width, scale, tab) in configs {
            let mut ribbon = Ribbon::new();
            ribbon.layout(&Measure(&device), &strings, width as f32, scale, 0.0);
            if tab != 0 {
                let [x, y, w, h] = ribbon.rect_of(bo_ui::RibbonHit::Tab(tab)).unwrap();
                ribbon.press(x + w / 2.0, y + h / 2.0);
                ribbon.release(x + w / 2.0, y + h / 2.0);
                ribbon.layout(&Measure(&device), &strings, width as f32, scale, 0.0);
            }
            ribbon.measure_previews(&Measure(&device), &styles);
            for style in styles.iter().take(5) {
                let preview = &style.preview;
                let size = (preview.size * scale * 96.0 / 72.0)
                    .clamp((11.0 * scale).round(), (20.0 * scale).round());
                let font = FontSpec {
                    family: &preview.font,
                    size,
                    weight: if preview.bold { 700 } else { 400 },
                    italic: preview.italic,
                    underline: preview.underline,
                };
                let (text_width, _) = device.measure_font("AaĐđ", &font);
                let available = (78.0 * scale).round() - (8.0 * scale).round();
                assert!(
                    text_width <= available,
                    "style {} at {scale}: {text_width} > {available}",
                    style.name
                );
            }
            let height = ribbon.height() as u32;
            let size = D2D_SIZE_U { width, height };
            let pixel = D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            };
            let target_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let read_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            // SAFETY: both bitmaps and the context are owned; the mapping is read only until Unmap.
            unsafe {
                let target = device
                    .context
                    .CreateBitmap(size, None, 0, &target_props)
                    .unwrap();
                device.context.SetTarget(&target);
                device.context.SetDpi(96.0, 96.0);
                device.context.BeginDraw();
                device.context.SetTransform(&IDENTITY);
                device.apply_ui_text();
                let frame = Frame { device: &device };
                frame.clear(theme.surface);
                ribbon.paint(
                    &mut UiPainter(&frame, None),
                    &theme,
                    &icons,
                    &strings,
                    view.format(),
                    &styles,
                    &[],
                );
                device.context.EndDraw(None, None).unwrap();
                device.context.SetTarget(None::<&ID2D1Image>);
                let read = device
                    .context
                    .CreateBitmap(size, None, 0, &read_props)
                    .unwrap();
                read.CopyFromBitmap(None, &target, None).unwrap();
                let mapped = read.Map(D2D1_MAP_OPTIONS_READ).unwrap();
                assert!(!mapped.bits.is_null() && mapped.pitch as usize >= width as usize * 4);
                let mut ppm = format!("P6 {width} {height} 255\n").into_bytes();
                for y in 0..height as usize {
                    let row = std::slice::from_raw_parts(
                        mapped.bits.add(y * mapped.pitch as usize),
                        width as usize * 4,
                    );
                    for bgra in row.as_chunks::<4>().0 {
                        ppm.extend_from_slice(&[bgra[2], bgra[1], bgra[0]]);
                    }
                }
                read.Unmap().unwrap();
                std::fs::write(
                    format!("{output}/ribbon-{width}-{scale}-tab-{tab}.ppm"),
                    ppm,
                )
                .unwrap();
            }
        }
    }
}
