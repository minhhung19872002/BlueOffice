//! Offscreen Direct2D / DirectWrite page rendering (feature `gpu-preview`).
//!
//! Pages are drawn by Direct2D with DirectWrite glyph runs and read back as RGBA for the
//! WebView canvas. This does not create a native window surface; any error makes the
//! WebView fall back to its own canvas renderer for that page.
use crate::EngineState;
use tauri::{ipc::Response, State};

#[tauri::command]
pub(crate) fn engine_gpu_available() -> bool {
    cfg!(all(windows, feature = "gpu-preview"))
}

#[cfg(all(windows, feature = "gpu-preview"))]
mod imp {
    use std::cell::RefCell;
    use std::collections::HashMap;

    // Decoded pictures kept between pages; dropped wholesale past this many bytes.
    const MAX_CACHED_PIXELS: usize = 128 * 1024 * 1024;

    #[derive(Default)]
    pub(super) struct Pictures {
        // Keyed by picture id and source byte length, so a replaced picture decodes again.
        decoded: HashMap<(String, usize), Option<bo_gpu_win::Bitmap>>,
        bytes: usize,
    }

    impl Pictures {
        pub(super) fn get(&mut self, engine: &bo_engine::Engine, id: &str) -> Option<bo_gpu_win::Bitmap> {
            let source = engine.media_resolved(id)?;
            let key = (id.to_owned(), source.len());
            if let Some(hit) = self.decoded.get(&key) {
                return hit.clone();
            }
            let bitmap = bo_raster::decode_premultiplied(&source).map(|(width, height, pixels)| {
                bo_gpu_win::Bitmap {
                    width,
                    height,
                    pixels,
                }
            });
            let size = bitmap.as_ref().map_or(0, |b| b.pixels.len());
            if self.bytes + size > MAX_CACHED_PIXELS {
                self.decoded.clear();
                self.bytes = 0;
            }
            self.bytes += size;
            self.decoded.insert(key, bitmap.clone());
            bitmap
        }
    }

    thread_local! {
        // Direct2D's single-threaded factory and device stay on their creating thread.
        pub(super) static GPU: RefCell<Option<bo_gpu_win::Device>> = const { RefCell::new(None) };
        pub(super) static PICTURES: RefCell<Pictures> = RefCell::new(Pictures::default());
    }
}

/// Render one page display list (as shown by the editor) at `scale` pixels per point.
/// Response: width and height (u32 little-endian) followed by opaque RGBA pixels.
#[tauri::command]
pub(crate) fn engine_gpu_page(
    state: State<'_, EngineState>,
    json: String,
    scale: f32,
) -> Result<Response, String> {
    #[cfg(all(windows, feature = "gpu-preview"))]
    {
        let page: bo_render::PageList = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let engine = state.0.lock().map_err(|e| e.to_string())?;
        crate::perf::timed("engine_gpu_page", || {
            imp::GPU.with(|slot| {
                let mut slot = slot
                    .try_borrow_mut()
                    .map_err(|_| "Direct2D renderer busy")?;
                if slot.is_none() {
                    let device = bo_gpu_win::Device::new()?;
                    device.set_text_rendering(bo_gpu_win::TextRendering::document_default());
                    *slot = Some(device);
                }
                let device = slot.as_ref().ok_or("Direct2D device unavailable")?;
                let image = imp::PICTURES.with(|pictures| {
                    let pictures = RefCell::new(pictures.borrow_mut());
                    let resolve = |id: &str| pictures.borrow_mut().get(&engine, id);
                    device.render_page(&page, engine.fonts(), scale, &resolve)
                })?;
                let mut bytes = Vec::with_capacity(8 + image.pixels.len());
                bytes.extend_from_slice(&image.width.to_le_bytes());
                bytes.extend_from_slice(&image.height.to_le_bytes());
                bytes.extend_from_slice(&image.pixels);
                Ok(Response::new(bytes))
            })
        })
    }
    #[cfg(not(all(windows, feature = "gpu-preview")))]
    {
        let _ = (state, json, scale);
        Err("Direct2D renderer unavailable".into())
    }
}

#[cfg(all(windows, feature = "gpu-preview"))]
use std::cell::RefCell;
