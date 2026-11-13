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
        pub(super) fn get(
            &mut self,
            engine: &bo_engine::Engine,
            id: &str,
        ) -> Option<bo_gpu_win::Bitmap> {
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

    /// Display lists the WebView currently shows, by render id, to diff the next render with.
    #[derive(Default)]
    pub(super) struct Shown {
        next: u32,
        lists: std::collections::VecDeque<(u32, f32, bo_render::PageList)>,
    }

    // Visible pages plus overscan; older ids repaint in full.
    const SHOWN: usize = 16;

    impl Shown {
        pub(super) fn take(&mut self, id: u32, scale: f32) -> Option<bo_render::PageList> {
            let at = self
                .lists
                .iter()
                .position(|(i, s, _)| *i == id && (*s - scale).abs() < 1e-6)?;
            self.lists.remove(at).map(|(_, _, list)| list)
        }

        pub(super) fn keep(&mut self, scale: f32, list: bo_render::PageList) -> u32 {
            self.next = self.next.wrapping_add(1).max(1);
            if self.lists.len() >= SHOWN {
                self.lists.pop_front();
            }
            self.lists.push_back((self.next, scale, list));
            self.next
        }
    }

    thread_local! {
        // Direct2D's single-threaded factory and device stay on their creating thread.
        pub(super) static GPU: RefCell<Option<bo_gpu_win::Device>> = const { RefCell::new(None) };
        pub(super) static PICTURES: RefCell<Pictures> = RefCell::new(Pictures::default());
        pub(super) static SHOWN_LISTS: RefCell<Shown> = RefCell::new(Shown::default());
    }
}

/// Render one page display list (as shown by the editor) at `scale` pixels per point.
///
/// `base` is the render id the WebView's canvas shows now: only the area that changed since
/// that render is drawn and returned (typing repaints a few lines instead of the page).
/// Response: seven u32 little-endian (`id`, page width, page height, band left, top, width,
/// height) followed by the band's opaque RGBA pixels; an empty band means nothing changed.
#[tauri::command]
pub(crate) fn engine_gpu_page(
    state: State<'_, EngineState>,
    json: String,
    scale: f32,
    base: Option<u32>,
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
                let previous = base.and_then(|id| {
                    imp::SHOWN_LISTS.with(|shown| shown.borrow_mut().take(id, scale))
                });
                // Only the changed band (padded) is drawn; no base or a state change: everything.
                let region = previous
                    .and_then(|old| bo_gpu_win::changed_region(&old, &page).device_band(scale));
                let image = imp::PICTURES.with(|pictures| {
                    let pictures = RefCell::new(pictures.borrow_mut());
                    let resolve = |id: &str| pictures.borrow_mut().get(&engine, id);
                    device.render_page_region(&page, engine.fonts(), scale, &resolve, region)
                })?;
                let id = imp::SHOWN_LISTS.with(|shown| shown.borrow_mut().keep(scale, page));
                let mut bytes = Vec::with_capacity(28 + image.pixels.len());
                for v in [
                    id,
                    image.page_width,
                    image.page_height,
                    image.left,
                    image.top,
                    image.width,
                    image.height,
                ] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
                bytes.extend_from_slice(&image.pixels);
                Ok(Response::new(bytes))
            })
        })
    }
    #[cfg(not(all(windows, feature = "gpu-preview")))]
    {
        let _ = (state, json, scale, base);
        Err("Direct2D renderer unavailable".into())
    }
}

#[cfg(all(windows, feature = "gpu-preview"))]
use std::cell::RefCell;
