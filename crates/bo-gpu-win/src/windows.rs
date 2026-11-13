use crate::Driver;
use std::{marker::PhantomData, rc::Rc};
use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};

/// A Direct2D context backed by a Direct3D device. It is not yet connected to a surface.
pub struct Device {
    // Retain all parent COM objects for the lifetime of the context.
    _d3d: ID3D11Device,
    _factory: ID2D1Factory1,
    _device: ID2D1Device,
    _context: ID2D1DeviceContext,
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
                _factory: factory,
                _device: device,
                _context: context,
                driver,
                _thread_bound: PhantomData,
            })
        }
    }
}
