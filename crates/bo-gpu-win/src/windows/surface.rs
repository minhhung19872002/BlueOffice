//! Window presentation: a flip-model DXGI swap chain drawn by Direct2D, and GPU-resident page
//! bitmaps composed into it 1:1 (ADR-0018). Coordinates of a [`Frame`] are device pixels.
use super::Device;
use bo_render::{PageList, Rgba};
use windows::core::Interface;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1Image, D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
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
