//! Window presentation: a flip-model DXGI swap chain drawn by Direct2D, and GPU-resident page
//! bitmaps composed into it 1:1 (ADR-0018). Coordinates of a [`Frame`] are device pixels.
use super::Device;
use bo_render::{PageList, PathSeg, Rgba};
use windows::core::Interface;
use windows::core::HSTRING;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1Image, D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Brush, D2D1_CAP_STYLE_ROUND, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_LINE_JOIN_ROUND,
    D2D1_TEXT_ANTIALIAS_MODE_DEFAULT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteRenderingParams, IDWriteTextFormat,
    IDWriteTextLayout, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_TEXT_METRICS,
    DWRITE_TEXT_RANGE, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1, IDXGISwapChain2, DXGI_PRESENT,
    DXGI_SCALING_NONE, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::System::Threading::WaitForSingleObjectEx;
use windows_numerics::Matrix3x2;

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

    fn ui_layout(&self, text: &str, f: &FontSpec<'_>) -> Result<IDWriteTextLayout, String> {
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
            };
            surface.bind(device)?;
            Ok(surface)
        }
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
            // Interface text uses the system's ClearType settings; pages carry their own pixels.
            device
                .context
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_DEFAULT);
            device
                .context
                .SetTextRenderingParams(None::<&IDWriteRenderingParams>);
            paint(&Frame { device });
            let ended = device.context.EndDraw(None, None);
            device.context.SetTarget(None::<&ID2D1Image>);
            ended.map_err(|e| e.to_string())?;
            self.swap
                .Present(1, DXGI_PRESENT(0))
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
            if let Ok(brush) = self.device.context.CreateSolidColorBrush(&color(c), None) {
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
            if let Ok(brush) = self.device.context.CreateSolidColorBrush(&color(c), None) {
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
        let Ok(geometry) = self.device.geometry(segs) else {
            return;
        };
        // SAFETY: geometry, brushes and stroke style are owned; drawing happens in the frame.
        unsafe {
            if let Some(c) = fill {
                if let Ok(brush) = self.device.solid(c, 1.0) {
                    self.device
                        .context
                        .FillGeometry(&geometry, &brush, None::<&ID2D1Brush>);
                }
            }
            if let Some((c, width)) = stroke {
                if let (Ok(brush), Ok(style)) = (
                    self.device.solid(c, 1.0),
                    self.device.stroke_style(
                        width,
                        &[],
                        D2D1_CAP_STYLE_ROUND,
                        D2D1_LINE_JOIN_ROUND,
                    ),
                ) {
                    self.device
                        .context
                        .DrawGeometry(&geometry, &brush, width, &style);
                }
            }
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
