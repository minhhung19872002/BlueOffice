//! Caret geometry, hit testing and caret movement over a laid-out document.
//!
//! Positions are `bo_edit::Pos` (body paragraph id + index-text offset); layout
//! lines carry the same ids (`LineSource::Body`) and caret stops.

use bo_edit::{Pos, Selection};
use bo_layout::{LayoutDocument, Line, LineSource};
use serde::Serialize;

/// Caret box on a page (points).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CaretRect {
    /// 0-based page index.
    pub page: usize,
    /// Left of the caret in points from the page's left edge.
    pub x: f32,
    /// Top of the caret in points from the page's top edge.
    pub y: f32,
    /// Caret height in points.
    pub h: f32,
}

/// Selection highlight rectangle on a page (points).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SelRect {
    /// 0-based page index.
    pub page: usize,
    /// Left edge in points from the page's left edge.
    pub x: f32,
    /// Top edge in points from the page's top edge.
    pub y: f32,
    /// Width in points.
    pub w: f32,
    /// Height in points.
    pub h: f32,
}

/// Width shown for a selected paragraph mark.
const MARK_W: f32 = 4.0;

/// Text story positions refer to: the body, or one footnote/endnote whose
/// paragraphs are numbered in document order inside the note (§17.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Story {
    /// The main document body.
    #[default]
    Body,
    /// One footnote or endnote.
    Note {
        /// `true` = endnote, `false` = footnote.
        endnote: bool,
        /// Note id (`w:id`).
        id: i32,
    },
    /// The text box at index offset `at` of body paragraph `para` (§17.17.1).
    TextBox {
        /// Body paragraph id of the anchor.
        para: u32,
        /// Index-text offset of the anchor in that paragraph.
        at: u32,
    },
}

thread_local! {
    /// Story the navigation functions work on (set with [`with_story`]).
    static STORY: std::cell::Cell<Story> = const { std::cell::Cell::new(Story::Body) };
}

/// Run `f` with the navigation functions working on the lines of `story`.
pub fn with_story<R>(story: Story, f: impl FnOnce() -> R) -> R {
    let saved = STORY.with(|s| s.replace(story));
    let r = f();
    STORY.with(|s| s.set(saved));
    r
}

fn body_id(l: &Line) -> Option<usize> {
    match (l.source, STORY.with(std::cell::Cell::get)) {
        (LineSource::Body(id), Story::Body) => Some(id as usize),
        (LineSource::Note { endnote, id, para }, Story::Note { endnote: e, id: i })
            if endnote == e && id == i =>
        {
            Some(para as usize)
        }
        (LineSource::TextBox { para, at, sub }, Story::TextBox { para: p, at: a })
            if para == p && at == a =>
        {
            Some(sub as usize)
        }
        _ => None,
    }
}

/// Note shown by a line, if any.
pub fn note_of(l: &Line) -> Option<(bool, i32)> {
    match l.source {
        LineSource::Note { endnote, id, .. } => Some((endnote, id)),
        _ => None,
    }
}

/// All lines of the current story in placement order with their page.
pub fn body_lines(layout: &LayoutDocument) -> Vec<(usize, &Line)> {
    layout
        .pages
        .iter()
        .enumerate()
        .flat_map(|(pi, p)| {
            p.lines
                .iter()
                .filter(|l| body_id(l).is_some())
                .map(move |l| (pi, l))
        })
        .collect()
}

/// x of the caret stop at `offset` in a line (nearest stop at or before it).
pub fn caret_x(line: &Line, offset: usize) -> f32 {
    let off = offset as u32;
    let mut x = line.carets.first().map_or(0.0, |c| c.1);
    for &(o, cx) in &line.carets {
        if o <= off {
            x = cx;
        } else {
            break;
        }
    }
    x
}

fn right_x(line: &Line) -> f32 {
    line.carets.last().map_or(0.0, |c| c.1)
}

/// The line showing `pos` (a position at a soft line end belongs to the next line).
fn line_of<'l>(lines: &[(usize, &'l Line)], pos: Pos) -> Option<(usize, usize, &'l Line)> {
    let mut best: Option<(usize, usize, &Line)> = None;
    for (i, &(pi, l)) in lines.iter().enumerate() {
        if body_id(l) != Some(pos.para) {
            continue;
        }
        let (s, e) = (l.start as usize, l.end as usize);
        if pos.offset >= s && pos.offset < e {
            return Some((i, pi, l));
        }
        if pos.offset >= s || best.is_none() {
            best = Some((i, pi, l));
        }
    }
    best
}

/// Caret rectangle for a position.
pub fn caret_rect(layout: &LayoutDocument, pos: Pos) -> Option<CaretRect> {
    let lines = body_lines(layout);
    let (_, page, l) = line_of(&lines, pos)?;
    let x = caret_x(l, pos.offset);
    // Vertical text (§17.4.72 textDirection): a short bar where the caret turns to.
    if let Some(r) = l.rotation {
        let (cx, cy) = r.apply(x, l.top + l.height / 2.0);
        return Some(CaretRect {
            page,
            x: cx,
            y: cy - 2.0,
            h: 4.0,
        });
    }
    Some(CaretRect {
        page,
        x,
        y: l.top,
        h: l.height,
    })
}

/// Vertical extent of the paragraph holding `pos` on the page of its caret:
/// `x` is the left edge of its lines, `y`/`h` span from the top of its first
/// line to the bottom of its last line on that page (presence bars).
pub fn paragraph_band(layout: &LayoutDocument, pos: Pos) -> Option<CaretRect> {
    let lines = body_lines(layout);
    let (_, page, _) = line_of(&lines, pos)?;
    let (mut x, mut top, mut bottom) = (f32::MAX, f32::MAX, f32::MIN);
    for &(pi, l) in &lines {
        if pi != page || body_id(l) != Some(pos.para) || l.rotation.is_some() {
            continue;
        }
        x = x.min(l.carets.first().map_or(0.0, |c| c.1));
        top = top.min(l.top);
        bottom = bottom.max(l.top + l.height);
    }
    (top <= bottom).then_some(CaretRect {
        page,
        x,
        y: top,
        h: bottom - top,
    })
}

/// Axis-aligned box of a rectangle of a rotated line.
fn rotated_rect(
    r: &bo_layout::LineRotation,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> (f32, f32, f32, f32) {
    let (ax, ay) = r.apply(x, y);
    let (bx, by) = r.apply(x + w, y + h);
    (ax.min(bx), ay.min(by), (ax - bx).abs(), (ay - by).abs())
}

/// Highlight rectangles of a selection.
pub fn selection_rects(layout: &LayoutDocument, sel: Selection) -> Vec<SelRect> {
    if sel.is_collapsed() {
        return Vec::new();
    }
    let (a, b) = sel.ordered();
    let mut out = Vec::new();
    for (page, l) in body_lines(layout) {
        let Some(id) = body_id(l) else { continue };
        if id < a.para || id > b.para {
            continue;
        }
        let (ls, le) = (l.start as usize, l.end as usize);
        let s = if id == a.para { a.offset.max(ls) } else { ls };
        let e = if id == b.para { b.offset.min(le) } else { le };
        let continues = id < b.para && le >= s;
        if e < s || (e == s && !continues) {
            continue;
        }
        let x0 = caret_x(l, s);
        let mut x1 = caret_x(l, e);
        // Selection running past this paragraph's last line includes the paragraph mark.
        if continues && e == le {
            x1 = x1.max(right_x(l)) + MARK_W;
        }
        let (x, y, w, h) = (x0, l.top, (x1 - x0).max(0.5), l.height);
        let (x, y, w, h) = match &l.rotation {
            Some(r) => rotated_rect(r, x, y, w, h),
            None => (x, y, w, h),
        };
        out.push(SelRect { page, x, y, w, h });
    }
    out
}

/// Nearest caret stop to `x` in a line.
fn offset_at(l: &Line, x: f32) -> usize {
    let mut best = (l.start, f32::INFINITY);
    for &(o, cx) in &l.carets {
        let d = (cx - x).abs();
        if d < best.1 {
            best = (o, d);
        }
    }
    best.0 as usize
}

/// Position under a point of a page (nearest line, then nearest caret stop).
pub fn hit_test(layout: &LayoutDocument, page: usize, x: f32, y: f32) -> Option<Pos> {
    let p = layout.pages.get(page)?;
    let mut best: Option<(&Line, f32, f32)> = None;
    for l in p.lines.iter().filter(|l| body_id(l).is_some()) {
        // Vertical text (§17.4.72): measure in the line's own, unrotated frame.
        let (x, y) = l.rotation.map_or((x, y), |r| r.invert(x, y));
        let dy = if y < l.top {
            l.top - y
        } else if y > l.top + l.height {
            y - l.top - l.height
        } else {
            0.0
        };
        let (lx0, lx1) = (l.carets.first().map_or(0.0, |c| c.1), right_x(l));
        let dx = if x < lx0 {
            lx0 - x
        } else if x > lx1 {
            x - lx1
        } else {
            0.0
        };
        // Vertical distance dominates; horizontal breaks ties (table cells side by side).
        let score = dy * 1000.0 + dx;
        if best.is_none_or(|b| score < b.1) {
            best = Some((l, score, x));
        }
    }
    let (l, _, lx) = best.or_else(|| {
        // Page without body text: snap to the nearest body line of other pages.
        let lines = body_lines(layout);
        let after = lines.iter().find(|(pi, _)| *pi >= page).or(lines.last())?;
        Some((after.1, 0.0, x))
    })?;
    Some(Pos::new(body_id(l)?, offset_at(l, lx)))
}

/// Caret movement kinds handled with layout information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
    /// One character left.
    Left,
    /// One character right.
    Right,
    /// To the start of the previous word.
    WordLeft,
    /// To the start of the next word.
    WordRight,
    /// One line up.
    Up,
    /// One line down.
    Down,
    /// To the start of the line (Home).
    LineStart,
    /// To the end of the line (End).
    LineEnd,
    /// One page up.
    PageUp,
    /// One page down.
    PageDown,
    /// To the start of the document (Ctrl+Home).
    DocStart,
    /// To the end of the document (Ctrl+End).
    DocEnd,
}

/// Vertical/line motions; `goal_x` keeps the column across consecutive up/down moves.
pub fn vertical(
    layout: &LayoutDocument,
    from: Pos,
    motion: Motion,
    goal_x: Option<f32>,
) -> Option<(Pos, f32)> {
    let lines = body_lines(layout);
    let (i, page, l) = line_of(&lines, from)?;
    let x = goal_x.unwrap_or_else(|| caret_x(l, from.offset));
    let pick = |j: usize| -> Option<(Pos, f32)> {
        let (_, t) = lines.get(j)?;
        Some((Pos::new(body_id(t)?, offset_at(t, x)), x))
    };
    match motion {
        Motion::Up if i > 0 => pick(i - 1),
        Motion::Down => pick(i + 1),
        Motion::LineStart => Some((
            Pos::new(from.para, l.start as usize),
            l.carets.first().map_or(0.0, |c| c.1),
        )),
        Motion::LineEnd => {
            // Stop before a trailing space that wrapped the line.
            let end = l.end as usize;
            Some((Pos::new(from.para, end), right_x(l)))
        }
        Motion::PageUp | Motion::PageDown => {
            let target = if motion == Motion::PageDown {
                page + 1
            } else {
                page.checked_sub(1)?
            };
            if target >= layout.pages.len() {
                return lines
                    .last()
                    .and_then(|(_, t)| Some((Pos::new(body_id(t)?, t.end as usize), x)));
            }
            hit_test(layout, target, x, l.top + l.height / 2.0).map(|p| (p, x))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_layout::{Fragment, Page};

    fn line(id: u32, top: f32, start: u32, end: u32, x0: f32) -> Line {
        Line {
            top,
            height: 10.0,
            baseline: top + 8.0,
            fragments: Vec::<Fragment>::new(),
            source: LineSource::Body(id),
            start,
            end,
            carets: (start..=end)
                .map(|o| (o, x0 + (o - start) as f32 * 5.0))
                .collect(),
            rotation: None,
        }
    }

    fn layout() -> LayoutDocument {
        LayoutDocument {
            pages: vec![
                Page {
                    width: 100.0,
                    height: 100.0,
                    lines: vec![line(0, 10.0, 0, 4, 10.0), line(0, 20.0, 4, 8, 10.0)],
                    items: vec![],
                    number: 1,
                    section: 0,
                    decor: bo_layout::PageDecor::default(),
                },
                Page {
                    width: 100.0,
                    height: 100.0,
                    lines: vec![line(1, 10.0, 0, 3, 10.0)],
                    items: vec![],
                    number: 2,
                    section: 0,
                    decor: bo_layout::PageDecor::default(),
                },
            ],
        }
    }

    #[test]
    fn caret_and_hits() {
        let l = layout();
        let c = caret_rect(&l, Pos::new(0, 4)).unwrap();
        assert_eq!((c.page, c.x, c.y), (0, 10.0, 20.0)); // soft line end → next line
        let c = caret_rect(&l, Pos::new(0, 8)).unwrap();
        assert_eq!((c.x, c.y), (30.0, 20.0));
        assert_eq!(hit_test(&l, 0, 21.0, 12.0), Some(Pos::new(0, 2)));
        assert_eq!(hit_test(&l, 0, 99.0, 95.0), Some(Pos::new(0, 8)));
        assert_eq!(hit_test(&l, 1, 0.0, 0.0), Some(Pos::new(1, 0)));
        assert!(hit_test(&l, 9, 0.0, 0.0).is_none());
    }

    #[test]
    fn rects_and_motion() {
        let l = layout();
        let sel = Selection {
            anchor: Pos::new(0, 2),
            focus: Pos::new(1, 1),
        };
        let r = selection_rects(&l, sel);
        assert_eq!(r.len(), 3);
        assert_eq!(r[2].page, 1);
        let (p, x) = vertical(&l, Pos::new(0, 1), Motion::Down, None).unwrap();
        assert_eq!(p, Pos::new(0, 5));
        let (p, _) = vertical(&l, p, Motion::Down, Some(x)).unwrap();
        assert_eq!(p, Pos::new(1, 1));
        assert!(vertical(&l, Pos::new(0, 0), Motion::Up, None).is_none());
        let (p, _) = vertical(&l, Pos::new(0, 5), Motion::LineStart, None).unwrap();
        assert_eq!(p, Pos::new(0, 4));
        let (p, _) = vertical(&l, Pos::new(0, 1), Motion::PageDown, None).unwrap();
        assert_eq!(p.para, 1);
    }
}
