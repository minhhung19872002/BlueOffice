//! BlueEditor round 11: while Edit Text is on, a thin frame around every text box of the
//! visible pages, as the reference editor draws them (black box at 125 %: 1 px #C0C0C0, two
//! pixels outside the text on the left and right, one above and below; a box joins the
//! paragraphs of a column that start within two lines of the one before, the same rule as its
//! typing frame, `bo_pdfread::BlockEdit::frame`). The box being typed into keeps its blue frame
//! with handles instead.
use crate::editor_edit::Editing;
use crate::editor_window::EditTool;
use crate::reader_view::ReaderView;
use crate::Painter;
use bo_pdfread::TextBlock;
use bo_render::{PathSeg, Rgba};

/// The idle frame colour measured on the reference.
pub(crate) const FRAME: Rgba = Rgba(0xC0, 0xC0, 0xC0, 255);

/// Text boxes of a page: paragraphs joined down a column while each starts within two lines
/// (of the larger font size) of the box so far (page points, left, top, right, bottom).
pub(crate) fn text_boxes(blocks: &[TextBlock]) -> Vec<[f32; 4]> {
    let mut order: Vec<&TextBlock> = blocks
        .iter()
        .filter(|b| !b.text.trim().is_empty())
        .collect();
    order.sort_by(|a, b| a.bbox[1].total_cmp(&b.bbox[1]));
    // Each box: its rectangle and the largest font size in it.
    let mut boxes: Vec<([f32; 4], f32)> = Vec::new();
    for b in order {
        let size = b.font_size.max(1.0);
        let joined = boxes.iter_mut().rev().find(|(r, s)| {
            let overlap = b.bbox[0] < r[2] && b.bbox[2] > r[0];
            let gap = b.bbox[1] - r[3];
            overlap && gap >= -0.25 * size && gap <= 2.0 * s.max(size)
        });
        match joined {
            Some((r, s)) => {
                *r = [
                    r[0].min(b.bbox[0]),
                    r[1].min(b.bbox[1]),
                    r[2].max(b.bbox[2]),
                    r[3].max(b.bbox[3]),
                ];
                *s = s.max(size);
            }
            None => boxes.push((b.bbox, size)),
        }
    }
    boxes.into_iter().map(|b| b.0).collect()
}

impl Editing {
    /// Text boxes of page `i`, kept for the document's edit count.
    fn boxes11(&mut self, v: &mut ReaderView, i: usize) -> Vec<[f32; 4]> {
        let edits = v.doc().edit_count();
        if let Some((e, b)) = self.frames11.get(&i) {
            if *e == edits {
                return b.clone();
            }
        }
        let b = v
            .doc_mut()
            .page_objects(i)
            .map(|o| text_boxes(&bo_pdfread::text_blocks(&o)))
            .unwrap_or_default();
        if self.frames11.len() > 64 {
            self.frames11.clear();
        }
        self.frames11.insert(i, (edits, b.clone()));
        b
    }

    /// The idle frames of Edit Text on the visible pages (under the typing frame).
    pub(crate) fn paint_frames11(
        &mut self,
        p: &mut dyn Painter,
        v: &mut ReaderView,
        tool: EditTool,
    ) {
        if tool != EditTool::Text {
            return;
        }
        let typing = self
            .typing
            .as_ref()
            .map(|t| (t.page, t.block.as_ref().map_or(t.bbox, |b| b.bbox)));
        let frame = v.frame();
        p.push_clip(frame[0], frame[1], frame[2], frame[3]);
        for page in v.visible_pages() {
            for b in self.boxes11(v, page) {
                // The box being typed into has the blue frame.
                if typing.is_some_and(|(tp, tb)| {
                    tp == page && tb[0] < b[2] && tb[2] > b[0] && tb[1] < b[3] && tb[3] > b[1]
                }) {
                    continue;
                }
                let Some(r) = v.doc_rect_to_window(page, b) else {
                    continue;
                };
                let (x0, y0) = (r[0].round() - 2.5, r[1].round() - 1.5);
                let (x1, y1) = ((r[0] + r[2]).round() + 2.5, (r[1] + r[3]).round() + 1.5);
                p.path(
                    &[
                        PathSeg::M(x0, y0),
                        PathSeg::L(x1, y0),
                        PathSeg::L(x1, y1),
                        PathSeg::L(x0, y1),
                        PathSeg::Z,
                    ],
                    None,
                    Some((FRAME, 1.0)),
                );
            }
        }
        p.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(top: f32, bottom: f32, size: f32) -> TextBlock {
        TextBlock {
            ids: Vec::new(),
            bbox: [72.0, top, 400.0, bottom],
            text: "text".into(),
            char_boxes: Vec::new(),
            runs: Vec::new(),
            align: bo_pdfread::Align::Left,
            lines: 1,
            font_size: size,
            color: [0, 0, 0],
        }
    }

    #[test]
    fn close_paragraphs_share_a_box_and_a_far_one_has_its_own() {
        let blocks = [
            block(100.0, 136.0, 12.0),
            block(146.0, 170.0, 12.0),
            block(180.0, 204.0, 12.0),
            block(300.0, 324.0, 12.0),
        ];
        let boxes = text_boxes(&blocks);
        assert_eq!(
            boxes,
            vec![[72.0, 100.0, 400.0, 204.0], [72.0, 300.0, 400.0, 324.0]]
        );
    }

    #[test]
    fn columns_and_empty_blocks() {
        let mut right = block(100.0, 136.0, 12.0);
        right.bbox[0] = 420.0;
        right.bbox[2] = 600.0;
        let mut empty = block(140.0, 150.0, 12.0);
        empty.text = " ".into();
        let boxes = text_boxes(&[block(100.0, 136.0, 12.0), right, empty]);
        assert_eq!(boxes.len(), 2);
    }
}
