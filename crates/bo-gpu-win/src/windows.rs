use crate::Driver;
use std::{marker::PhantomData, rc::Rc};
use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1, ID2D1Image,
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_MAP_OPTIONS_READ,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};

mod page;

/// A Direct2D context backed by a Direct3D device. It is not yet connected to a surface.
pub struct Device {
    // Retain all parent COM objects for the lifetime of the context.
    _d3d: ID3D11Device,
    factory: ID2D1Factory1,
    _device: ID2D1Device,
    context: ID2D1DeviceContext,
    driver: Driver,
    // A single-threaded Direct2D factory and its resources must remain on their creating thread.
    _thread_bound: PhantomData<Rc<()>>,
}

impl Device {
    /// Create a hardware device, falling back to WARP if the hardware driver is unavailable.
    pub fn new() -> Result<Self, String> {
        Self::create(D3D_DRIVER_TYPE_HARDWARE, Driver::Hardware)
            .or_else(|_| Self::create(D3D_DRIVER_TYPE_WARP, Driver::Software))
    }

    /// Returns the driver chosen during device creation.
    pub fn driver(&self) -> Driver {
        self.driver
    }

    /// Draw a DirectWrite glyph run to a small offscreen surface and count non-white pixels.
    /// This diagnostic does not draw a document or connect the surface to a window.
    pub fn verify_text(&self, font: &[u8], glyphs: &[bo_dwrite::Glyph]) -> Result<usize, String> {
        // SAFETY: COM objects own both bitmaps; the glyph-run callback holds its font and glyph
        // arrays throughout DrawGlyphRun. Mapped memory is read only while mapped and unmapped.
        unsafe {
            let size = D2D_SIZE_U {
                width: 64,
                height: 64,
            };
            let pixel = D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            };
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let target = self
                .context
                .CreateBitmap(size, None, 0, &props)
                .map_err(|e| e.to_string())?;
            let white = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let black = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            };
            let brush = self
                .context
                .CreateSolidColorBrush(&black, None)
                .map_err(|e| e.to_string())?;
            self.context.SetTarget(&target);
            self.context.BeginDraw();
            self.context.Clear(Some(&white));
            let drawing = bo_dwrite::with_glyph_run(font, 0, glyphs, 20.0, |run| {
                self.context.DrawGlyphRun(
                    windows_numerics::Vector2 { X: 8.0, Y: 40.0 },
                    run,
                    None,
                    &brush,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                Ok(())
            });
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            drawing?;
            ended.map_err(|e| e.to_string())?;
            let read_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let read = self
                .context
                .CreateBitmap(size, None, 0, &read_props)
                .map_err(|e| e.to_string())?;
            read.CopyFromBitmap(None, &target, None)
                .map_err(|e| e.to_string())?;
            let mapped = read.Map(D2D1_MAP_OPTIONS_READ).map_err(|e| e.to_string())?;
            if mapped.bits.is_null() || mapped.pitch < 256 {
                read.Unmap().map_err(|e| e.to_string())?;
                return Err("invalid mapped Direct2D surface".into());
            }
            let mut ink = 0;
            for y in 0..64_usize {
                for x in 0..64_usize {
                    let ptr = mapped.bits.add(y * mapped.pitch as usize + x * 4);
                    if *ptr != 255 || *ptr.add(1) != 255 || *ptr.add(2) != 255 {
                        ink += 1;
                    }
                }
            }
            read.Unmap().map_err(|e| e.to_string())?;
            Ok(ink)
        }
    }

    /// Draw an opaque red rectangle on white offscreen and read two BGRA pixels for diagnostics.
    ///
    /// This verifies a small offscreen path only; it is not a document renderer or a window surface.
    pub fn verify_offscreen(&self) -> Result<([u8; 4], [u8; 4]), String> {
        // SAFETY: Direct2D owns both bitmaps. The mapped pointer is read only while mapped,
        // with dimensions and row pitch checked before indexing, and is unmapped before return.
        unsafe {
            let size = D2D_SIZE_U {
                width: 16,
                height: 16,
            };
            let pixel = D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            };
            let target_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let target = self
                .context
                .CreateBitmap(size, None, 0, &target_props)
                .map_err(|e| e.to_string())?;
            let white = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let red = D2D1_COLOR_F {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            };
            let brush = self
                .context
                .CreateSolidColorBrush(&red, None)
                .map_err(|e| e.to_string())?;
            self.context.SetTarget(&target);
            self.context.BeginDraw();
            self.context.Clear(Some(&white));
            self.context.FillRectangle(
                &D2D_RECT_F {
                    left: 4.0,
                    top: 4.0,
                    right: 12.0,
                    bottom: 12.0,
                },
                &brush,
            );
            let ended = self.context.EndDraw(None, None);
            self.context.SetTarget(None::<&ID2D1Image>);
            ended.map_err(|e| e.to_string())?;
            let read_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel,
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let read = self
                .context
                .CreateBitmap(size, None, 0, &read_props)
                .map_err(|e| e.to_string())?;
            read.CopyFromBitmap(None, &target, None)
                .map_err(|e| e.to_string())?;
            let mapped = read.Map(D2D1_MAP_OPTIONS_READ).map_err(|e| e.to_string())?;
            if mapped.bits.is_null() || mapped.pitch < 64 {
                read.Unmap().map_err(|e| e.to_string())?;
                return Err("invalid mapped Direct2D surface".into());
            }
            let sample = |x: usize, y: usize| -> [u8; 4] {
                let ptr = mapped.bits.add(y * mapped.pitch as usize + x * 4);
                [*ptr, *ptr.add(1), *ptr.add(2), *ptr.add(3)]
            };
            let result = (sample(0, 0), sample(8, 8));
            read.Unmap().map_err(|e| e.to_string())?;
            Ok(result)
        }
    }

    fn create(kind: D3D_DRIVER_TYPE, driver: Driver) -> Result<Self, String> {
        // SAFETY: all out pointers refer to live local COM option slots; the generated bindings
        // check HRESULTs and own each returned interface. Parent objects outlive the context.
        unsafe {
            let mut d3d = None;
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                kind,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
            .map_err(|e| e.to_string())?;
            let d3d = d3d.ok_or("Direct3D returned no device")?;
            let dxgi: IDXGIDevice = d3d.cast().map_err(|e| e.to_string())?;
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|e| e.to_string())?;
            let device = factory.CreateDevice(&dxgi).map_err(|e| e.to_string())?;
            let context = device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                .map_err(|e| e.to_string())?;
            Ok(Self {
                _d3d: d3d,
                factory,
                _device: device,
                context,
                driver,
                _thread_bound: PhantomData,
            })
        }
    }
}
