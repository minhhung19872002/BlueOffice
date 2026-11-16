//! BlueEditor round 8: Backspace at the start of a paragraph joins it to the paragraph above
//! in its box, Delete at its end pulls the paragraph below up — as the reference editor does
//! (measured on a copy: the second paragraph's text runs on after the first one's last
//! character, no space added; at the start of the first paragraph Backspace does nothing).
use crate::editor_edit::{Change, Editing, Item, Typing};
use crate::reader_view::ReaderView;
use bo_pdfread::ObjectKind;

/// Whether `b` is the paragraph right above `a` in the same column (its bottom within about
/// two lines of `a`'s top, overlapping it across).
fn just_above(b: [f32; 4], a: [f32; 4], line: f32) -> bool {
    let gap = a[1] - b[3];
    gap > -line * 0.3 && gap < line * 2.2 && b[0] < a[2] && a[0] < b[2]
}

fn closest(items: &[Item], bbox: [f32; 4]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.kind == ObjectKind::Text && it.block.is_some())
        .min_by(|(_, x), (_, y)| {
            let d = |r: [f32; 4]| (r[0] - bbox[0]).abs() + (r[1] - bbox[1]).abs();
            d(x.bbox).total_cmp(&d(y.bbox))
        })
        .map(|(i, _)| i)
}

impl Editing {
    /// Join the typed paragraph with its neighbour: `forward` false (Backspace at its start)
    /// with the one above, true (Delete at its end) with the one below. True when handled
    /// (also when there is nothing to join, as the reference ignores the key then).
    pub(crate) fn join_paragraph(&mut self, v: &mut ReaderView, forward: bool) -> bool {
        let Some(t) = self.typing.as_ref() else {
            return false;
        };
        let Some(b) = t.block.as_ref() else {
            return false;
        };
        let page = t.page;
        let here = b.bbox;
        let line = b.pitch.max(1.0);
        self.finish_typing(Some(&mut *v));
        let list = self.items(v, page).to_vec();
        let Some(cur) = closest(&list, here) else {
            return true;
        };
        let cur_box = list[cur].bbox;
        // The neighbour: the nearest paragraph right above (or below) in the same column.
        let other = list
            .iter()
            .enumerate()
            .filter(|(i, it)| *i != cur && it.kind == ObjectKind::Text && it.block.is_some())
            .filter(|(_, it)| {
                if forward {
                    just_above(cur_box, it.bbox, line)
                } else {
                    just_above(it.bbox, cur_box, line)
                }
            })
            .min_by(|(_, x), (_, y)| {
                let d = |r: [f32; 4]| {
                    if forward {
                        r[1] - cur_box[3]
                    } else {
                        cur_box[1] - r[3]
                    }
                };
                d(x.bbox).abs().total_cmp(&d(y.bbox).abs())
            })
            .map(|(i, _)| i);
        let Some(other) = other else {
            // Nothing to join: keep typing where the caret was.
            self.reopen(v, page, &list[cur], forward);
            return true;
        };
        // The lower paragraph's text moves to the end of the upper one, which keeps its
        // place and styles; the lower one is deleted.
        let (upper, lower) = if forward { (cur, other) } else { (other, cur) };
        let (Some(ub), Some(lb)) = (list[upper].block.clone(), list[lower].block.clone()) else {
            return true;
        };
        let moved = lb.text.clone();
        let upper_box = ub.bbox;
        if let Err(e) = v.doc_mut().delete_objects(page, &lb.ids) {
            self.status = Some(e);
            return true;
        }
        v.invalidate_page(page);
        let list = self.items(v, page).to_vec();
        let Some(k) = closest(&list, upper_box) else {
            return true;
        };
        let Some(block) = list[k].block.clone() else {
            return true;
        };
        match v.doc_mut().begin_block_edit(page, &block) {
            Ok(mut blk) => {
                let mode = self.push_mode();
                if let Err(e) = v.doc_mut().block_edit_push(&mut blk, mode) {
                    self.status = Some(e);
                }
                let join = blk.text.len();
                let _ = blk.insert(join, &moved);
                self.typing = Some(Typing {
                    page,
                    block: Some(blk),
                    caret: join,
                    anchor: None,
                    new_at: None,
                    bbox: block.bbox,
                    selecting: false,
                    undo: Vec::new(),
                    redo: Vec::new(),
                    last: Change::Deleting,
                });
                self.selected = vec![k];
                self.write(v);
            }
            Err(e) => self.status = Some(e),
        }
        true
    }

    /// Start typing again in `item` with the caret at its start (or end).
    fn reopen(&mut self, v: &mut ReaderView, page: usize, item: &Item, end: bool) {
        let Some(block) = item.block.clone() else {
            return;
        };
        if let Ok(blk) = v.doc_mut().begin_block_edit(page, &block) {
            let caret = if end { blk.text.len() } else { 0 };
            self.typing = Some(Typing {
                page,
                block: Some(blk),
                caret,
                anchor: None,
                new_at: None,
                bbox: block.bbox,
                selecting: false,
                undo: Vec::new(),
                redo: Vec::new(),
                last: Change::None,
            });
        }
    }
}
