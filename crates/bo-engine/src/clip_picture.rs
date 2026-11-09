//! Picture of the selected document content for "paste as picture"
//! (CORE-PASTE-001): the laid-out page cropped to the selection, rendered to PNG
//! by the raster path used for printing previews.

use crate::{nav, Engine};

/// Pixels per point of the copied picture.
const SCALE: f32 = 2.0;
/// Largest picture side in pixels.
const MAX_SIDE: f32 = 4096.0;
/// Margin around the selection (points).
const PAD: f32 = 2.0;

impl Engine {
    /// PNG (base64) of the main-text selection as it is laid out, cropped to its
    /// highlight rectangles on the first page they touch; `None` for a caret,
    /// while a header/footer or note is edited, or when it would be too large.
    pub(crate) fn selection_picture(&self) -> Option<String> {
        use base64::Engine as _;
        if self.hf.is_some() || self.note.is_some() {
            return None;
        }
        let sel = self.session.as_ref()?.selection();
        if sel.is_collapsed() {
            return None;
        }
        let rects = nav::selection_rects(self.layout.document(), sel);
        let page = rects.first()?.page;
        let on_page = rects.iter().filter(|r| r.page == page && r.w > 0.0);
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for r in on_page {
            x0 = x0.min(r.x);
            y0 = y0.min(r.y);
            x1 = x1.max(r.x + r.w);
            y1 = y1.max(r.y + r.h);
        }
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let list = self.page(page).ok()?;
        let (x0, y0) = ((x0 - PAD).max(0.0), (y0 - PAD).max(0.0));
        let (x1, y1) = ((x1 + PAD).min(list.width), (y1 + PAD).min(list.height));
        let (w, h) = (x1 - x0, y1 - y0);
        if w <= 0.0 || h <= 0.0 || w * SCALE > MAX_SIDE || h * SCALE > MAX_SIDE {
            return None;
        }
        let mut commands = Vec::with_capacity(list.commands.len() + 2);
        commands.push(bo_render::Command::PushTransform {
            m: [1.0, 0.0, 0.0, 1.0, -x0, -y0],
        });
        commands.extend(list.commands.iter().cloned());
        commands.push(bo_render::Command::PopTransform);
        let cropped = bo_render::PageList {
            width: w,
            height: h,
            commands,
        };
        let png =
            bo_raster::render_page_png_with(&cropped, &self.fonts, SCALE, &self.image_source())
                .ok()?;
        Some(base64::engine::general_purpose::STANDARD.encode(png))
    }
}
