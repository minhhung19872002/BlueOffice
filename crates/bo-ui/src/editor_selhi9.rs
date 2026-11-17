//! BlueEditor round 9: selected text while editing a paragraph, as the reference editor shows
//! it (black-box at 125 %: an opaque `#003371` band behind the selected characters, which are
//! drawn white), instead of a translucent tint over the page image.
//!
//! The band and the white characters go into the page's display list
//! ([`ReaderView::set_text_mark9`]), so they come out of the same page image as the text.
use crate::editor_edit::Typing;
use crate::editor_window::EditorWindow;
use crate::reader_view::ReaderView;
use bo_render::{Command, PageList, Rgba};

/// The reference's selection band behind edited text.
pub(crate) const MARK_BG: Rgba = Rgba(0x00, 0x33, 0x71, 255);
/// Selected characters.
pub(crate) const MARK_FG: Rgba = Rgba(255, 255, 255, 255);

/// Selected text of the paragraph being typed: page and boxes `[x0, y0, x1, y1]` (page points).
pub(crate) fn typing_marks(t: &Typing) -> Option<(usize, Vec<[f32; 4]>)> {
    let (a, e) = t.selection()?;
    let rects = t.block.as_ref()?.selection_rects(a, e);
    (!rects.is_empty()).then_some((t.page, rects))
}

/// `list` with the selection boxes `rects` (page points) filled with [`MARK_BG`] and the text
/// inside them drawn again in [`MARK_FG`]: per box, the list's clips, transforms, groups and
/// glyph runs (pictures, paths and fills left out) replayed inside the box.
pub(crate) fn mark_list(mut list: PageList, rects: &[[f32; 4]]) -> PageList {
    let text: Vec<Command> = list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Glyphs {
                face,
                size,
                x,
                y,
                text,
                glyphs,
                ..
            } => Some(Command::Glyphs {
                face: *face,
                size: *size,
                x: *x,
                y: *y,
                color: MARK_FG,
                text: text.clone(),
                glyphs: glyphs.clone(),
            }),
            Command::FillRect { .. }
            | Command::Line { .. }
            | Command::Image { .. }
            | Command::Path { .. }
            | Command::BlurPath { .. } => None,
            other => Some(other.clone()),
        })
        .collect();
    for r in rects {
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            continue;
        }
        list.commands.push(Command::FillRect {
            x: r[0],
            y: r[1],
            w,
            h,
            color: MARK_BG,
        });
        list.commands.push(Command::PushClip {
            x: r[0],
            y: r[1],
            w,
            h,
        });
        list.commands.extend(text.iter().cloned());
        list.commands.push(Command::PopClip);
    }
    list
}

impl EditorWindow {
    /// Hand the selected text of the paragraph being typed to the page view (before pages are
    /// drawn): the view redraws the changed band.
    pub(crate) fn sync_text_mark9(&mut self) {
        let want = self.edit.typing.as_ref().and_then(typing_marks);
        if let Some(v) = self.r.view_mut() {
            v.set_text_mark9(want);
        }
    }
}

impl ReaderView {
    /// Selected text drawn into page images (BlueEditor's Edit Text): page and boxes in page
    /// points, or none. The pages concerned are read again in the changed band.
    pub(crate) fn set_text_mark9(&mut self, want: Option<(usize, Vec<[f32; 4]>)>) {
        if self.text_mark9 == want {
            return;
        }
        let old = std::mem::replace(&mut self.text_mark9, want.clone());
        let touch = |m: Option<(usize, Vec<[f32; 4]>)>, v: &mut Self| {
            if let Some((page, rects)) = m {
                let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for r in &rects {
                    b = [
                        b[0].min(r[0]),
                        b[1].min(r[1]),
                        b[2].max(r[2]),
                        b[3].max(r[3]),
                    ];
                }
                if b[0] <= b[2] {
                    v.invalidate_band(page, [b[0] - 1.0, b[1] - 1.0, b[2] + 1.0, b[3] + 1.0]);
                }
            }
        };
        touch(old, self);
        touch(want, self);
    }

    /// The display list of page `i` with BlueEditor's text selection drawn in, if any.
    pub(crate) fn with_text_mark9(&self, i: usize, list: PageList) -> PageList {
        match &self.text_mark9 {
            Some((page, rects)) if *page == i => mark_list(list, rects),
            _ => list,
        }
    }
}
