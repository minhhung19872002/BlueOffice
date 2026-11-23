//! Pictures for the interface (BlueDoc round 18: Insert › Screenshot's Available Windows
//! thumbnails): a GPU bitmap made from RGBA pixels, drawn with [`super::Frame::draw_page_part`].
use super::Device;
use super::PageBitmap;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_PIXEL_FORMAT, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

impl Device {
    /// An opaque bitmap of `width` × `height` pixels from `rgba` (R, G, B, A rows, top-down;
    /// alpha is ignored).
    pub fn create_image(&self, width: u32, height: u32, rgba: &[u8]) -> Result<PageBitmap, String> {
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err("picture too large or empty".into());
        }
        if rgba.len() < (width * height * 4) as usize {
            return Err("picture data too short".into());
        }
        let bgra: Vec<u8> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[2], p[1], p[0], 255])
            .collect();
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        // SAFETY: the pixel buffer holds `height` rows of `width * 4` bytes and lives for the
        // call; the context copies it into the new bitmap.
        let bitmap = unsafe {
            self.context
                .CreateBitmap(
                    D2D_SIZE_U { width, height },
                    Some(bgra.as_ptr().cast()),
                    width * 4,
                    &props,
                )
                .map_err(|e| e.to_string())?
        };
        Ok(PageBitmap {
            bitmap,
            width,
            height,
        })
    }
}
