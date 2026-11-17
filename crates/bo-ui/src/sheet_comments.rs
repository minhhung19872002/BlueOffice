//! Comment cards of BlueSheet: hovering a cell with a comment or note shows its card, and
//! Show All Comments (Review tab) keeps every card on screen. Measured on the reference
//! (docs/ui/REPORT-sheet.md round 15): a 193 × 119 px box at 125 %, fill #FFFFE1, 1 px black
//! frame, 15 px right of / 8 px above the cell's top-right corner, text 6 px inside, a
//! black line from the cell's corner mark to the box; the author's name is bold.
use super::SheetWindow;
use crate::{Font, Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};

const CARD_FILL: Rgba = Rgba(0xff, 0xff, 0xe1, 255);
const CARD_EDGE: Rgba = Rgba(0, 0, 0, 255);

/// A laid-out card: cell, author, wrapped lines of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Card {
    pub cell: (u32, u32),
    pub author: String,
    pub lines: Vec<String>,
}

impl SheetWindow {
    fn card_font(&self, bold: bool) -> Font<'static> {
        Font {
            family: "Tahoma",
            size: (12.0 * self.scale).round(),
            weight: if bold { 700 } else { 400 },
            italic: false,
            underline: false,
        }
    }

    /// Author and text of the comment / note on cell (`r`, `c`).
    fn comment_of(&self, r: u32, c: u32) -> Option<(String, String)> {
        let wb = self.view.engine().workbook()?;
        let ws = wb.sheets.get(self.view.sheet())?;
        if let Some(n) = ws
            .comments
            .iter()
            .find(|n| (n.cell.row, n.cell.col) == (r, c))
        {
            let text: String = n.text.iter().map(|t| t.text.as_str()).collect();
            // A note's text starts with "Author:" when the author typed it; keep it once.
            let text = text
                .strip_prefix(&format!("{}:", n.author))
                .map_or(text.clone(), |t| t.trim_start().to_owned());
            return Some((n.author.clone(), text));
        }
        let mut thread = ws
            .threaded_comments
            .iter()
            .filter(|t| (t.cell.row, t.cell.col) == (r, c));
        let first = thread.next()?;
        let name = |id: &str| {
            wb.persons
                .iter()
                .find(|p| p.id == id)
                .map_or_else(String::new, |p| p.display_name.clone())
        };
        let mut text = first.text.clone();
        for reply in thread {
            text.push('\n');
            text.push_str(&format!("{}: {}", name(&reply.person_id), reply.text));
        }
        Some((name(&first.person_id), text))
    }

    /// Cards to show: the hovered cell's, or every one with Show All Comments; text wrapped
    /// to the card width.
    pub(super) fn layout_cards(&mut self, m: &dyn TextMeasure) {
        let cells: Vec<(u32, u32)> = if self.show_all_comments {
            self.commented_cells()
        } else {
            let mut v: Vec<(u32, u32)> = self.pinned_comments.clone();
            v.extend(self.hover_comment.filter(|h| !v.contains(h)));
            v
        };
        let font = self.card_font(false);
        let width = (193.0 - 12.0) * self.scale / 1.25;
        let mut cards = Vec::new();
        for (r, c) in cells {
            let Some((author, text)) = self.comment_of(r, c) else {
                continue;
            };
            let mut lines = Vec::new();
            for para in text.split('\n') {
                let mut line = String::new();
                for word in para.split(' ') {
                    let next = if line.is_empty() {
                        word.to_owned()
                    } else {
                        format!("{line} {word}")
                    };
                    if !line.is_empty() && m.measure_font(&next, &font).0 > width {
                        lines.push(std::mem::take(&mut line));
                        line = word.to_owned();
                    } else {
                        line = next;
                    }
                }
                lines.push(line);
            }
            cards.push(Card {
                cell: (r, c),
                author,
                lines,
            });
        }
        self.cards = cards;
    }

    /// The pointer moved to (x, y): the hovered comment cell; `true` when it changed.
    pub(super) fn hover_comments(&mut self, x: f32, y: f32) -> bool {
        let at = self
            .view
            .cell_under(x, y)
            .filter(|rc| self.commented_cells().contains(rc));
        std::mem::replace(&mut self.hover_comment, at) != at
    }

    /// Show or hide every comment card.
    pub(super) fn toggle_all_comments(&mut self) -> bool {
        self.show_all_comments = !self.show_all_comments;
        true
    }

    /// Paint the cards over the grid.
    pub(super) fn paint_cards(&self, p: &mut dyn Painter) {
        if self.cards.is_empty() {
            return;
        }
        let [fx, fy, fw, fh] = self.view.grid_frame();
        p.push_clip(fx, fy, fw, fh);
        let k = self.scale / 1.25;
        let (bold, plain) = (self.card_font(true), self.card_font(false));
        let line = (plain.size * 1.2).round();
        for card in &self.cards {
            let Some([cx, cy, cw, _]) = self.view.cell_rect(card.cell.0, card.cell.1) else {
                continue;
            };
            let corner = (cx + cw, cy);
            let (x, y) = (
                (corner.0 + 15.0 * k).round(),
                (corner.1 - 8.0 * k).round().max(fy),
            );
            let n = card.lines.len() + usize::from(!card.author.is_empty());
            let w = (193.0 * k).round();
            let h = ((n as f32 * line + 12.0 * k).max(119.0 * k)).round();
            p.path(
                &[
                    PathSeg::M(corner.0 - 1.0, corner.1 + 1.0),
                    PathSeg::L(x, y + 6.0 * k),
                ],
                None,
                Some((CARD_EDGE, 1.0)),
            );
            p.fill_rect(x, y, w, h, CARD_EDGE);
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, CARD_FILL);
            p.push_clip(x + 1.0, y + 1.0, w - 2.0, h - 2.0);
            let mut ty = y + (5.0 * k).round();
            let tx = x + (6.0 * k).round();
            if !card.author.is_empty() {
                p.text_font(&format!("{}:", card.author), tx, ty, &bold, CARD_EDGE);
                ty += line;
            }
            for l in &card.lines {
                p.text_font(l, tx, ty, &plain, CARD_EDGE);
                ty += line;
            }
            p.pop_clip();
        }
        p.pop_clip();
    }
}
