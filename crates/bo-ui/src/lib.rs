//! Self-drawn user interface of BlueOffice, shared by every platform (ADR-0018).
//!
//! This crate holds the interface state and logic only. A platform host turns OS events into
//! [`Input`], keeps page images in a [`PageStore`] (GPU bitmaps on Windows, canvases on the web)
//! and paints frames through a [`Painter`]. Editing goes straight to the in-process
//! [`bo_engine::Engine`]; nothing is serialised.
#![forbid(unsafe_code)]

mod doc_view;
mod input;

pub use doc_view::{DocView, PageSlot, CANVAS_BG};
pub use input::{Input, Key, Mods};

use bo_render::{PageList, Rgba};

/// Page images kept by a backend, one per page index.
pub trait PageStore {
    /// Bring page `index` up to date with `list` at `scale` device pixels per point (fonts and
    /// pictures come from `engine`). `band`
    /// (left, top, width, height in device pixels) limits the redraw to the part that changed
    /// since the image was last drawn at the same scale; `None` redraws the whole page.
    fn update(
        &mut self,
        index: usize,
        list: &PageList,
        engine: &bo_engine::Engine,
        scale: f32,
        band: Option<[u32; 4]>,
    ) -> Result<(), String>;

    /// Forget the image of page `index`.
    fn remove(&mut self, index: usize);
}

/// Drawing of one frame, in device pixels.
pub trait Painter {
    /// Fill the whole frame.
    fn clear(&mut self, color: Rgba);
    /// Fill a rectangle.
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba);
    /// Draw the stored image of page `index` 1:1 with its top-left corner at (`x`, `y`).
    fn page(&mut self, index: usize, x: i32, y: i32);
}
