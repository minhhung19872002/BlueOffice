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

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(windows)]
pub use windows::Device;
