//! BlueDoc round 15: what the editor's heading collapse needs from the view — hiding the
//! blocks under collapsed headings (view state, `Engine::set_hidden_blocks`) and where the
//! paragraphs of the visible pages start. A child module of `doc_view`.
use super::DocView;
use bo_engine::edit::Command;
use bo_layout::LineSource;

impl DocView {
    /// Hide these top-level body blocks (section, block); `true` when that changed the
    /// layout (every page is drawn again).
    pub fn set_hidden_blocks(&mut self, blocks: Vec<(usize, usize)>) -> bool {
        if !self.engine.set_hidden_blocks(blocks) {
            return false;
        }
        if let Ok(r) = self.engine.command(Command::State) {
            self.apply(&r);
        }
        self.invalidate_page_images();
        true
    }

    /// Body paragraphs whose first line is on a visible page, with that line's left edge,
    /// top and height in window pixels.
    pub fn visible_para_starts(&self) -> Vec<(usize, [f32; 3])> {
        let Some(layout) = self.engine.layout() else {
            return Vec::new();
        };
        let s = self.scale();
        let mut out = Vec::new();
        for slot in self.visible() {
            let Some(page) = layout.pages.get(slot.index) else {
                continue;
            };
            for l in &page.lines {
                if let LineSource::Body(id) = l.source {
                    if l.start == 0 && l.rotation.is_none() {
                        let x = bo_engine::nav::caret_x(l, 0);
                        out.push((
                            id as usize,
                            [
                                slot.x as f32 + x * s,
                                slot.y as f32 + l.top * s,
                                (l.height * s).max(1.0),
                            ],
                        ));
                    }
                }
            }
        }
        out
    }
}

impl DocView {
    /// The view's frame in window pixels (x, y, width, height).
    pub fn frame_rect(&self) -> [f32; 4] {
        [
            self.origin.0,
            self.origin.1,
            self.viewport.0,
            self.viewport.1,
        ]
    }
}
