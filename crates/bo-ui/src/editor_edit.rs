//! BlueEditor's Edit Text / Edit Object / Add Text tools on the document view: hover and
//! selection boxes, dragging to move, corner handles to resize, typing into a paragraph in
//! place (a [`BlockEdit`] session: the page is analysed once, every keystroke lays the
//! paragraph out again and rewrites only its text), selecting text, character formatting,
//! Delete, Escape.
use crate::editor_window::EditTool;
use crate::reader_view::ReaderView;
use crate::{Key, Mods, Painter};
use bo_pdfread::{text_blocks, Align, BlockEdit, ObjectKind, PageObject, StyleChange, TextBlock};
use bo_render::{PathSeg, Rgba};

/// Something the tools select: a paragraph (several runs) or another page object.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    /// Page object indices.
    pub ids: Vec<usize>,
    /// Box in document page points.
    pub bbox: [f32; 4],
    pub kind: ObjectKind,
    pub block: Option<TextBlock>,
}

/// Text being typed into.
#[derive(Debug, Clone)]
pub(crate) struct Typing {
    pub page: usize,
    /// The paragraph session (none yet for Add Text before the first character).
    pub block: Option<BlockEdit>,
    pub caret: usize,
    /// Other end of the selected text.
    pub anchor: Option<usize>,
    /// A new text block not written yet: its baseline origin and size.
    pub new_at: Option<([f32; 2], f32)>,
    /// Box shown before the first character of a new block.
    pub bbox: [f32; 4],
    /// The pointer is selecting text (button down).
    pub selecting: bool,
    /// Undo / redo within the session (Ctrl+Z / Ctrl+Y while typing), newest last.
    pub undo: Vec<Snap>,
    pub redo: Vec<Snap>,
    /// Kind of the last change (typing, deleting, other) for grouping undo steps.
    pub last: Change,
}

/// The paragraph's text and styles at one point of a typing session.
#[derive(Debug, Clone)]
pub(crate) struct Snap {
    text: Vec<char>,
    style_of: Vec<usize>,
    styles: Vec<bo_pdfread::RunStyle>,
    align: Align,
    caret: usize,
}

/// What a change in a typing session did: consecutive typing (up to a space) or deleting
/// undo together, as in the reference editor and word processors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Change {
    #[default]
    None,
    Typing,
    Deleting,
    Other,
}

/// Start of the word left of c (Ctrl+Left).
fn word_left(t: &[char], c: usize) -> usize {
    let mut i = c.min(t.len());
    while i > 0 && t[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !t[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// Start of the next word right of c (Ctrl+Right).
fn word_right(t: &[char], c: usize) -> usize {
    let mut i = c.min(t.len());
    while i < t.len() && !t[i].is_whitespace() {
        i += 1;
    }
    while i < t.len() && t[i].is_whitespace() {
        i += 1;
    }
    i
}

impl Typing {
    /// Selected characters (start, end), if any.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.caret).then(|| (a.min(self.caret), a.max(self.caret)))
    }

    fn bbox(&self) -> [f32; 4] {
        self.block.as_ref().map_or(self.bbox, |b| b.bbox)
    }

    /// The frame drawn while typing: the paragraph's box joined with the paragraphs after it
    /// (`BlockEdit::frame`), as the reference editor frames its text box.
    fn frame(&self) -> [f32; 4] {
        self.block
            .as_ref()
            .map_or(self.bbox, bo_pdfread::BlockEdit::frame)
    }

    fn snap(&self) -> Option<Snap> {
        let b = self.block.as_ref()?;
        Some(Snap {
            text: b.text.clone(),
            style_of: b.style_of.clone(),
            styles: b.styles.clone(),
            align: b.align,
            caret: self.caret,
        })
    }

    /// Remember the paragraph before a change. Round 9: inside the session every typed or
    /// deleted character is its own step, as the reference editor undoes them (black-box:
    /// "xyz abc" typed, each Undo takes one character off); the session as a whole is still
    /// one step of the document once it ends.
    fn checkpoint(&mut self, kind: Change, boundary: bool) {
        let _ = boundary;
        {
            if let Some(s) = self.snap() {
                self.undo.push(s);
                if self.undo.len() > 200 {
                    self.undo.remove(0);
                }
            }
        }
        self.redo.clear();
        self.last = kind;
    }

    fn restore(&mut self, s: Snap) {
        if let Some(b) = self.block.as_mut() {
            b.text = s.text;
            b.style_of = s.style_of;
            b.styles = s.styles;
            b.align = s.align;
        }
        self.caret = s.caret;
        self.anchor = None;
        self.last = Change::None;
    }

    /// The selected text.
    pub fn selected_text(&self) -> Option<String> {
        let (a, e) = self.selection()?;
        let b = self.block.as_ref()?;
        Some(b.text.get(a..e)?.iter().collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Drag {
    /// Moving the selection from a document point (`to` after constraint and snapping);
    /// `copy`: Ctrl held at the press, the originals stay.
    Move {
        from: (f32, f32),
        to: (f32, f32),
        copy: bool,
    },
    /// Resizing the single selected item by a corner (0 top-left … 3 bottom-right).
    Resize { corner: usize, to: (f32, f32) },
    /// Selecting the items a rectangle touches, from a document point.
    Marquee { from: (f32, f32), to: (f32, f32) },
    /// Dragging the right side of the paragraph's frame: its lines wrap at `to` (page x).
    Wrap { to: f32 },
}

/// Editing state of the active document.
#[derive(Debug, Default)]
pub(crate) struct Editing {
    /// Items of `page`, valid for `edits` (the document's edit count).
    cache: Option<(usize, u32, Vec<Item>)>,
    /// Edit Object picks text by line, as the reference (round 9); Edit Text by paragraph.
    pub object_lines: bool,
    /// The mode the cache was built for.
    cache_lines: bool,
    pub page: Option<usize>,
    pub selected: Vec<usize>,
    pub hover: Option<usize>,
    pub typing: Option<Typing>,
    pub(crate) drag: Option<Drag>,
    pub status: Option<String>,
    /// Shift / Ctrl held with the last mouse message (Ctrl+drag copies, Shift constrains).
    pub mods: Mods,
    /// Snapping guides of the current move, page points (x0, y0, x1, y1).
    pub guides: Vec<[f32; 4]>,
    /// Reflow Editing is on: everything below a paragraph moves with it while typing
    /// (otherwise only the paragraphs after it in its box, like the reference editor).
    pub reflow: bool,
    /// Round 10: Edit Object ▾ picks only this kind (All: `None`).
    pub filter10: Option<ObjectKind>,
    /// Round 11: text boxes framed while Edit Text is on, per page (edit count, boxes).
    pub frames11: std::collections::HashMap<usize, (u32, Vec<[f32; 4]>)>,
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]
}

fn items(objs: &[PageObject], lines: bool) -> Vec<Item> {
    let blocks = text_blocks(objs);
    let mut out: Vec<Item> = if lines {
        // Each text line is an object of its own; it keeps its paragraph for a double click
        // (editing the text).
        bo_pdfread::text_lines(objs)
            .into_iter()
            .map(|l| Item {
                block: blocks
                    .iter()
                    .find(|b| l.ids.iter().all(|i| b.ids.contains(i)))
                    .cloned(),
                ids: l.ids,
                bbox: l.bbox,
                kind: ObjectKind::Text,
            })
            .collect()
    } else {
        blocks
            .into_iter()
            .map(|b| Item {
                ids: b.ids.clone(),
                bbox: b.bbox,
                kind: ObjectKind::Text,
                block: Some(b),
            })
            .collect()
    };
    for (i, o) in objs.iter().enumerate() {
        if o.kind != ObjectKind::Text {
            out.push(Item {
                ids: vec![i],
                bbox: o.bbox,
                kind: o.kind,
                block: None,
            });
        }
    }
    out
}

impl Editing {
    /// What moves with a paragraph that gains or loses lines while typing.
    pub(crate) fn push_mode(&self) -> bo_pdfread::PushMode {
        if self.reflow {
            bo_pdfread::PushMode::Below
        } else {
            bo_pdfread::PushMode::Box
        }
    }

    /// Forget everything (another document, tool off).
    pub fn reset(&mut self, v: Option<&mut ReaderView>) {
        self.finish_typing(v);
        self.cache = None;
        self.page = None;
        self.selected.clear();
        self.hover = None;
        self.drag = None;
        self.guides.clear();
    }

    /// Items of page `i` (analysed again after every edit; kept while typing, the typed
    /// paragraph draws from its session).
    pub fn items(&mut self, v: &mut ReaderView, i: usize) -> &[Item] {
        let edits = v.doc().edit_count();
        let fresh = matches!(&self.cache, Some((p, e, _)) if *p == i && *e == edits)
            && self.cache_lines == self.object_lines;
        if !fresh {
            let list = v
                .doc_mut()
                .page_objects(i)
                .map(|o| items(&o, self.object_lines))
                .unwrap_or_default();
            self.cache = Some((i, edits, list));
            self.cache_lines = self.object_lines;
        }
        self.cache.as_ref().map_or(&[], |c| c.2.as_slice())
    }

    fn item_at(
        &mut self,
        v: &mut ReaderView,
        page: usize,
        x: f32,
        y: f32,
        tool: EditTool,
    ) -> Option<usize> {
        let filter = self.filter10;
        let list = self.items(v, page);
        // Text first for the text tool; the smallest box under the point otherwise.
        let mut best: Option<(usize, f32)> = None;
        for (k, it) in list.iter().enumerate() {
            if tool == EditTool::Text && it.kind != ObjectKind::Text {
                continue;
            }
            if tool == EditTool::Object && filter.is_some_and(|k| it.kind != k) {
                continue;
            }
            let pad = if it.kind == ObjectKind::Text {
                1.5
            } else {
                0.5
            };
            let b = [
                it.bbox[0] - pad,
                it.bbox[1] - pad,
                it.bbox[2] + pad,
                it.bbox[3] + pad,
            ];
            if inside(b, x, y) {
                let area = (b[2] - b[0]) * (b[3] - b[1]);
                if best.is_none_or(|(_, a)| area < a) {
                    best = Some((k, area));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// Pointer moved over the document (no button).
    pub fn hover(&mut self, v: &mut ReaderView, x: f32, y: f32, tool: EditTool) -> bool {
        let h = v.window_to_doc(x, y).and_then(|(p, px, py)| {
            let k = self.item_at(v, p, px, py, tool)?;
            Some((p, k))
        });
        let new = h
            .filter(|(p, _)| Some(*p) == self.page || self.page.is_none())
            .map(|h| h.1);
        if let Some((p, _)) = h {
            if self.page != Some(p) && self.selected.is_empty() && self.typing.is_none() {
                self.page = Some(p);
            }
        }
        let changed = new != self.hover;
        self.hover = new;
        changed
    }

    /// Press on the document with `tool`.
    pub fn press(
        &mut self,
        v: &mut ReaderView,
        x: f32,
        y: f32,
        count: u8,
        shift: bool,
        tool: EditTool,
    ) {
        let Some((page, px, py)) = v.window_to_doc(x, y) else {
            self.finish_typing(Some(v));
            self.selected.clear();
            return;
        };
        if self.page != Some(page) {
            self.finish_typing(Some(v));
            self.selected.clear();
            self.page = Some(page);
        }
        // A resize handle of the single selected object.
        if tool == EditTool::Object && self.selected.len() == 1 {
            if let Some(corner) = self.handle_at(v, page, x, y) {
                self.drag = Some(Drag::Resize {
                    corner,
                    to: (px, py),
                });
                return;
            }
        }
        // The right side of the frame (its handles): the lines wrap where it is dropped.
        if let Some(t) = self
            .typing
            .as_ref()
            .filter(|t| t.page == page && t.block.is_some())
        {
            let f = t.frame();
            let tol = 5.0 / v.scale().max(0.01);
            if (px - f[2]).abs() <= tol && py >= f[1] - tol && py <= f[3] + tol {
                self.drag = Some(Drag::Wrap { to: px });
                return;
            }
        }
        // A press inside the paragraph being typed into moves the caret or selects.
        if let Some(t) = self.typing.as_mut() {
            let b = t.bbox();
            let pad = 3.0;
            if t.page == page
                && inside([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad], px, py)
                && tool != EditTool::Object
            {
                if let Some(blk) = &t.block {
                    let c = blk.hit(px, py);
                    match count {
                        2 => {
                            let (a, e) = blk.word(c);
                            t.anchor = Some(a);
                            t.caret = e;
                        }
                        n if n >= 3 => {
                            t.anchor = Some(0);
                            t.caret = blk.text.len();
                        }
                        _ => {
                            if shift {
                                t.anchor.get_or_insert(t.caret);
                            } else {
                                t.anchor = None;
                            }
                            t.caret = c;
                            t.selecting = true;
                        }
                    }
                }
                return;
            }
            self.finish_typing(Some(v));
        }
        if tool == EditTool::AddText {
            let size = 12.0;
            self.typing = Some(Typing {
                page,
                block: None,
                caret: 0,
                anchor: None,
                new_at: Some(([px, py + size * 0.9], size)),
                bbox: [px, py, px + 2.0, py + size * 1.2],
                selecting: false,
                undo: Vec::new(),
                redo: Vec::new(),
                last: Change::None,
            });
            return;
        }
        let hit = self.item_at(v, page, px, py, tool);
        let Some(k) = hit else {
            if !shift {
                self.selected.clear();
            }
            // Edit Object: a rectangle dragged over empty space selects what it touches.
            if tool == EditTool::Object {
                self.drag = Some(Drag::Marquee {
                    from: (px, py),
                    to: (px, py),
                });
            }
            return;
        };
        let item = self.items(v, page).get(k).cloned();
        let Some(item) = item else { return };
        if tool == EditTool::Text || (count >= 2 && item.kind == ObjectKind::Text) {
            if let Some(b) = item.block {
                let push = self.push_mode();
                match v.doc_mut().begin_block_edit(page, &b).and_then(|mut blk| {
                    v.doc_mut().block_edit_push(&mut blk, push)?;
                    Ok(blk)
                }) {
                    Ok(blk) => {
                        let caret = blk.hit(px, py);
                        let n = blk.text.len();
                        let anchor = match count {
                            2 => {
                                let (a, e) = blk.word(caret);
                                Some((a, e))
                            }
                            n2 if n2 >= 3 => Some((0, n)),
                            _ => None,
                        };
                        self.typing = Some(Typing {
                            page,
                            block: Some(blk),
                            caret: anchor.map_or(caret, |a| a.1),
                            anchor: anchor.map(|a| a.0),
                            new_at: None,
                            bbox: b.bbox,
                            selecting: anchor.is_none(),
                            undo: Vec::new(),
                            redo: Vec::new(),
                            last: Change::None,
                        });
                        self.selected = vec![k];
                    }
                    Err(e) => self.status = Some(e),
                }
            }
            return;
        }
        if shift {
            if let Some(p) = self.selected.iter().position(|&s| s == k) {
                self.selected.remove(p);
            } else {
                self.selected.push(k);
            }
        } else if !self.selected.contains(&k) {
            self.selected = vec![k];
        }
        if self.selected.contains(&k) {
            self.drag = Some(Drag::Move {
                from: (px, py),
                to: (px, py),
                copy: self.mods.ctrl,
            });
        }
    }

    /// Handle of the single selected item under a window point: 0–3 corners (top left, top
    /// right, bottom left, bottom right), 4–7 side middles (top, bottom, left, right).
    fn handle_at(&mut self, v: &mut ReaderView, page: usize, x: f32, y: f32) -> Option<usize> {
        let k = *self.selected.first()?;
        let b = self.items(v, page).get(k)?.bbox;
        let r = v.doc_rect_to_window(page, b)?;
        handle_points(r)
            .into_iter()
            .find(|&(_, (cx, cy))| (x - cx).abs() <= 5.0 && (y - cy).abs() <= 5.0)
            .map(|(i, _)| i)
    }

    /// Button down and moving.
    pub fn drag(&mut self, v: &ReaderView, x: f32, y: f32) -> bool {
        if let Some(Drag::Wrap { to }) = self.drag.as_mut() {
            if let Some(pr) = self.page.and_then(|p| v.page_rect(p)) {
                *to = (x - pr[0]) / v.scale().max(0.01);
            }
            return true;
        }
        if let Some(t) = self.typing.as_mut().filter(|t| t.selecting) {
            let Some(pr) = v.page_rect(t.page) else {
                return false;
            };
            let s = v.scale();
            let r = v.unrotate_rect(t.page, [(x - pr[0]) / s, (y - pr[1]) / s, 0.0, 0.0]);
            if let Some(b) = &t.block {
                let c = b.hit(r[0], r[1]);
                if c != t.caret {
                    t.anchor.get_or_insert(t.caret);
                    t.caret = c;
                }
            }
            return true;
        }
        let Some(page) = self.page else { return false };
        let Some(pr) = v.page_rect(page) else {
            return false;
        };
        let s = v.scale();
        // Keep following the pointer outside the page.
        let (px, py) = ((x - pr[0]) / s, (y - pr[1]) / s);
        let r = v.unrotate_rect(page, [px, py, px, py]);
        let pointer = (r[0], r[1]);
        let tol = 6.0 / s.max(0.01);
        let page_size = v
            .doc()
            .page_info(page)
            .map_or_else(|| v.page_size(page), |p| (p.width, p.height));
        match self.drag {
            Some(Drag::Move { from, copy, .. }) => {
                let mut d = (pointer.0 - from.0, pointer.1 - from.1);
                // Shift keeps the move horizontal or vertical.
                let across = d.0.abs() >= d.1.abs();
                if self.mods.shift {
                    if across {
                        d.1 = 0.0;
                    } else {
                        d.0 = 0.0;
                    }
                }
                let (boxes, others) = self.split_boxes();
                let moved = crate::editor_arrange::union_box(&boxes)
                    .map(|b| [b[0] + d.0, b[1] + d.1, b[2] + d.0, b[3] + d.1]);
                let (mut sx, mut sy, mut guides) =
                    crate::editor_arrange::snap_move(moved, &others, page_size, tol);
                // Snapping does not undo the constraint: only the free axis snaps.
                if self.mods.shift {
                    if across {
                        sy = 0.0;
                        guides.retain(|g| (g[0] - g[2]).abs() < 1e-3);
                    } else {
                        sx = 0.0;
                        guides.retain(|g| (g[1] - g[3]).abs() < 1e-3);
                    }
                }
                d.0 += sx;
                d.1 += sy;
                self.guides = guides;
                self.drag = Some(Drag::Move {
                    from,
                    to: (from.0 + d.0, from.1 + d.1),
                    copy,
                });
                true
            }
            Some(Drag::Resize { corner, .. }) => {
                self.drag = Some(Drag::Resize {
                    corner,
                    to: pointer,
                });
                true
            }
            Some(Drag::Marquee { from, .. }) => {
                self.drag = Some(Drag::Marquee { from, to: pointer });
                true
            }
            Some(Drag::Wrap { .. }) => true,
            None => false,
        }
    }

    /// Items of the page from the last analysis (empty before one).
    pub(crate) fn cached_items(&self) -> &[Item] {
        self.cache.as_ref().map_or(&[][..], |c| c.2.as_slice())
    }

    /// Boxes of the selected items and of the other items of the page (from the cache).
    fn split_boxes(&self) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
        let list = self.cache.as_ref().map_or(&[][..], |c| c.2.as_slice());
        let mut sel = Vec::new();
        let mut others = Vec::new();
        for (k, it) in list.iter().enumerate() {
            if self.selected.contains(&k) {
                sel.push(it.bbox);
            } else {
                others.push(it.bbox);
            }
        }
        (sel, others)
    }

    /// The resize box of the single selected item for a corner dragged to `to`; Shift keeps
    /// its proportions.
    fn resized(&self, b: [f32; 4], corner: usize, to: (f32, f32)) -> ([f32; 4], f32, f32) {
        if corner >= 4 {
            return side_resized(b, corner, to);
        }
        let (ax, ay) = match corner {
            0 => (b[2], b[3]),
            1 => (b[0], b[3]),
            2 => (b[2], b[1]),
            _ => (b[0], b[1]),
        };
        let (w0, h0) = ((b[2] - b[0]).max(0.01), (b[3] - b[1]).max(0.01));
        let (mut sx, mut sy) = ((to.0 - ax).abs() / w0, (to.1 - ay).abs() / h0);
        if self.mods.shift {
            let k = sx.max(sy);
            sx = k;
            sy = k;
        }
        let (w1, h1) = (w0 * sx, h0 * sy);
        let x0 = if to.0 < ax { ax - w1 } else { ax };
        let y0 = if to.1 < ay { ay - h1 } else { ay };
        ([x0, y0, x0 + w1, y0 + h1], sx, sy)
    }

    /// Button released: apply a move or resize.
    pub fn release(&mut self, v: &mut ReaderView) -> bool {
        if let Some(t) = self.typing.as_mut() {
            t.selecting = false;
        }
        let Some(page) = self.page else { return false };
        self.guides.clear();
        match self.drag.take() {
            Some(Drag::Wrap { to }) => {
                if let Some(b) = self.typing.as_mut().and_then(|t| t.block.as_mut()) {
                    let left = b.bbox[0];
                    b.set_wrap_width(to - left);
                }
                self.write(v)
            }
            Some(Drag::Move { from, to, copy }) => {
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                    return false;
                }
                let m = [1.0, 0.0, 0.0, 1.0, dx, dy];
                let expect = self.expected(v, page, m);
                let ids = self.selected_ids(v, page);
                let r = if copy {
                    v.doc_mut().copy_objects(page, &ids, m)
                } else {
                    v.doc_mut().transform_objects(page, &ids, m)
                };
                self.after_edit(v, page, r.map(|()| ids.len()));
                // The moved items are new objects: select them again by position.
                self.reselect(v, page, &expect);
                true
            }
            Some(Drag::Resize { corner, to }) => {
                let Some(&k) = self.selected.first() else {
                    return false;
                };
                let Some(item) = self.items(v, page).get(k).cloned() else {
                    return false;
                };
                let b = item.bbox;
                let (nb, sx, sy) = self.resized(b, corner, to);
                if b[2] - b[0] < 0.1
                    || b[3] - b[1] < 0.1
                    || nb[2] - nb[0] < 2.0
                    || nb[3] - nb[1] < 2.0
                {
                    return false;
                }
                // The opposite corner stays.
                let m = [sx, 0.0, 0.0, sy, nb[0] - b[0] * sx, nb[1] - b[1] * sy];
                let r = v.doc_mut().transform_objects(page, &item.ids, m);
                self.after_edit(v, page, r.map(|()| 1));
                self.reselect(v, page, &[(nb, item.kind)]);
                true
            }
            Some(Drag::Marquee { from, to }) => {
                let r = [
                    from.0.min(to.0),
                    from.1.min(to.1),
                    from.0.max(to.0),
                    from.1.max(to.1),
                ];
                if r[2] - r[0] < 1.0 && r[3] - r[1] < 1.0 {
                    return true;
                }
                let hits: Vec<usize> = self
                    .items(v, page)
                    .iter()
                    .enumerate()
                    .filter(|(_, it)| {
                        let b = it.bbox;
                        b[0] <= r[2] && b[2] >= r[0] && b[1] <= r[3] && b[3] >= r[1]
                    })
                    .map(|(k, _)| k)
                    .collect();
                for k in hits {
                    if !self.selected.contains(&k) {
                        self.selected.push(k);
                    }
                }
                true
            }
            None => false,
        }
    }

    /// Boxes and kinds of the selected items after transforming them by `m` (page points).
    pub(crate) fn expected(
        &mut self,
        v: &mut ReaderView,
        page: usize,
        m: [f32; 6],
    ) -> Vec<([f32; 4], ObjectKind)> {
        let sel = self.selected.clone();
        let list = self.items(v, page);
        sel.iter()
            .filter_map(|&k| list.get(k))
            .map(|it| (crate::editor_arrange::transform_box(it.bbox, m), it.kind))
            .collect()
    }

    /// Select the items of `page` closest to the boxes `expect` (after an edit gave the
    /// objects new indices).
    pub(crate) fn reselect(
        &mut self,
        v: &mut ReaderView,
        page: usize,
        expect: &[([f32; 4], ObjectKind)],
    ) {
        let list = self.items(v, page).to_vec();
        let mut sel = Vec::new();
        for (b, kind) in expect {
            let best = list
                .iter()
                .enumerate()
                .filter(|(k, it)| it.kind == *kind && !sel.contains(k))
                .map(|(k, it)| {
                    let d: f32 = (0..4).map(|i| (it.bbox[i] - b[i]).abs()).sum();
                    (k, d)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((k, d)) = best {
                if d < 4.0 + 0.05 * ((b[2] - b[0]) + (b[3] - b[1])) {
                    sel.push(k);
                }
            }
        }
        self.selected = sel;
    }

    fn selected_ids(&mut self, v: &mut ReaderView, page: usize) -> Vec<usize> {
        let sel = self.selected.clone();
        let list = self.items(v, page);
        let mut ids: Vec<usize> = sel
            .iter()
            .filter_map(|&k| list.get(k))
            .flat_map(|it| it.ids.iter().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn after_edit(&mut self, v: &mut ReaderView, page: usize, r: Result<usize, String>) {
        match r {
            Ok(_) => {
                v.invalidate_page(page);
                self.selected.clear();
                self.cache = None;
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// Delete the selection.
    pub fn delete(&mut self, v: &mut ReaderView) -> bool {
        let Some(page) = self.page else { return false };
        if self.selected.is_empty() {
            return false;
        }
        let ids = self.selected_ids(v, page);
        let r = v.doc_mut().delete_objects(page, &ids);
        self.after_edit(v, page, r.map(|()| 0));
        true
    }

    /// The single selected image (to replace it).
    pub fn selected_image(&mut self, v: &mut ReaderView) -> Option<(usize, usize)> {
        let page = self.page?;
        let k = *self.selected.first()?;
        let one = self.selected.len() == 1;
        let it = self.items(v, page).get(k)?;
        (one && it.kind == ObjectKind::Image).then(|| (page, it.ids[0]))
    }

    /// End typing (the edits so far stay, as one undo step).
    pub fn finish_typing(&mut self, v: Option<&mut ReaderView>) {
        if let Some(t) = self.typing.take() {
            if let (Some(v), Some(b)) = (v, &t.block) {
                v.doc_mut().end_block_edit(b);
                if b.dirty {
                    v.invalidate_page(t.page);
                }
            }
            self.cache = None;
            self.selected.clear();
        }
    }

    /// Typed text (replaces the selected text).
    pub fn type_text(&mut self, v: &mut ReaderView, s: &str) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        if t.block.is_none() {
            return self.start_new_text(v, &s);
        }
        let sel = t.selection();
        let boundary = sel.is_some() || s.chars().any(char::is_whitespace) || s.chars().count() > 1;
        t.checkpoint(Change::Typing, boundary);
        if let Some(b) = t.block.as_mut() {
            if let Some((a, e)) = sel {
                b.remove(a, e);
                t.caret = a;
            }
            t.caret = b.insert(t.caret, &s);
        }
        t.anchor = None;
        self.write(v)
    }

    /// Pasted plain text: line breaks become paragraph breaks of the block (as Enter).
    pub fn paste_text(&mut self, v: &mut ReaderView, s: &str) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let text = s.replace("\r\n", "\n").replace('\r', "\n");
        let text: String = text
            .chars()
            .map(|c| if c == '\t' { ' ' } else { c })
            .filter(|c| *c == '\n' || !c.is_control())
            .collect();
        if text.is_empty() || t.block.is_none() {
            return t.block.is_none() && self.type_text(v, &text.replace('\n', " "));
        }
        t.checkpoint(Change::Other, true);
        let sel = t.selection();
        if let Some(b) = t.block.as_mut() {
            if let Some((a, e)) = sel {
                b.remove(a, e);
                t.caret = a;
            }
            t.caret = b.insert(t.caret, &text);
        }
        t.anchor = None;
        self.write(v)
    }

    /// Cut: remove the selected text (the caller copied it).
    pub fn cut(&mut self, v: &mut ReaderView) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let Some((a, e)) = t.selection() else {
            return false;
        };
        t.checkpoint(Change::Other, true);
        if let Some(b) = t.block.as_mut() {
            b.remove(a, e);
        }
        t.caret = a;
        t.anchor = None;
        self.write(v)
    }

    /// First characters of an Add Text block: write them, then edit the new paragraph.
    fn start_new_text(&mut self, v: &mut ReaderView, s: &str) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let Some((at, size)) = t.new_at else {
            return false;
        };
        let page = t.page;
        let push = self.push_mode();
        v.doc_mut().begin_group("add text");
        let r = v
            .doc_mut()
            .add_text(page, at, s, size, [0, 0, 0])
            .and_then(|id| {
                let objs = v.doc_mut().page_objects(page)?;
                let blocks = text_blocks(&objs);
                let b = blocks
                    .into_iter()
                    .find(|b| b.ids.contains(&id))
                    .ok_or("no text")?;
                let caret = b
                    .runs
                    .iter()
                    .rposition(|&r| r == id)
                    .map_or(b.text.chars().count(), |i| i + 1);
                let mut blk = v.doc_mut().begin_block_edit(page, &b)?;
                v.doc_mut().block_edit_push(&mut blk, push)?;
                Ok((blk, caret))
            });
        v.invalidate_page(page);
        // The block's session joins the open group: ending it closes one undo step.
        match r {
            Ok((blk, caret)) => {
                if let Some(t) = self.typing.as_mut() {
                    t.block = Some(blk);
                    t.caret = caret;
                    t.new_at = None;
                }
                true
            }
            Err(e) => {
                v.doc_mut().end_group();
                self.status = Some(e);
                self.typing = None;
                true
            }
        }
    }

    /// A key while typing; false when it is not for the text.
    #[allow(clippy::too_many_lines)]
    pub fn key(&mut self, v: &mut ReaderView, k: Key, mods: Mods) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let Some(b) = t.block.as_ref() else {
            if matches!(k, Key::Escape | Key::Enter) {
                self.typing = None;
                return true;
            }
            return false;
        };
        let n = b.text.len();
        let sel = t.selection();
        // Moving the caret: Shift extends the selection.
        let moved = match (k, mods.ctrl) {
            (Key::Left, true) => Some(word_left(&b.text, t.caret)),
            (Key::Right, true) => Some(word_right(&b.text, t.caret)),
            (Key::Left, _) => Some(match (sel, mods.shift) {
                (Some((a, _)), false) => a,
                _ => t.caret.saturating_sub(1),
            }),
            (Key::Right, _) => Some(match (sel, mods.shift) {
                (Some((_, e)), false) => e,
                _ => (t.caret + 1).min(n),
            }),
            (Key::Up, _) => Some(b.vertical(t.caret, false)),
            (Key::Down, _) => Some(b.vertical(t.caret, true)),
            (Key::Home, true) => Some(0),
            (Key::End, true) => Some(n),
            (Key::Home, false) => Some(b.home_end(t.caret).0),
            (Key::End, false) => Some(b.home_end(t.caret).1),
            _ => None,
        };
        if let Some(c) = moved {
            if mods.shift {
                t.anchor.get_or_insert(t.caret);
            } else {
                t.anchor = None;
            }
            t.caret = c;
            return true;
        }
        // Undo and redo inside the session; past its first change the document's undo
        // takes over (it ends the session).
        match (k, mods.ctrl, mods.shift) {
            (Key::Letter('z'), true, false) => {
                let Some(s) = t.undo.pop() else {
                    return false;
                };
                if let Some(cur) = t.snap() {
                    t.redo.push(cur);
                }
                t.restore(s);
                self.write(v);
                return true;
            }
            (Key::Letter('y'), true, false) | (Key::Letter('z'), true, true) => {
                let Some(s) = t.redo.pop() else {
                    return false;
                };
                if let Some(cur) = t.snap() {
                    t.undo.push(cur);
                }
                t.restore(s);
                self.write(v);
                return true;
            }
            _ => {}
        }
        let kind = match k {
            Key::Backspace | Key::Delete => Change::Deleting,
            Key::Enter => Change::Other,
            _ => Change::None,
        };
        if kind != Change::None {
            t.checkpoint(kind, sel.is_some() || mods.ctrl);
        }
        let changed = match (k, mods.ctrl) {
            (Key::Backspace, true) if sel.is_none() && t.caret > 0 => {
                let e = t.caret;
                let a = t.block.as_ref().map_or(e, |b| word_left(&b.text, e));
                if let Some(b) = t.block.as_mut() {
                    b.remove(a, e);
                }
                t.caret = a;
                true
            }
            (Key::Delete, true) if sel.is_none() && t.caret < n => {
                let a = t.caret;
                let e = t.block.as_ref().map_or(a, |b| word_right(&b.text, a));
                if let Some(b) = t.block.as_mut() {
                    b.remove(a, e);
                }
                true
            }
            (Key::Letter('a'), true) => {
                t.anchor = Some(0);
                t.caret = n;
                return true;
            }
            (Key::Letter('b'), true) => {
                let on = !t
                    .block
                    .as_ref()
                    .and_then(|b| b.style_at(t.caret))
                    .is_some_and(|s| s.bold);
                return self.style(v, &StyleChange::Bold(on));
            }
            (Key::Letter('i'), true) => {
                let on = !t
                    .block
                    .as_ref()
                    .and_then(|b| b.style_at(t.caret))
                    .is_some_and(|s| s.italic);
                return self.style(v, &StyleChange::Italic(on));
            }
            (Key::Backspace | Key::Delete, _) if sel.is_some() => {
                if let (Some((a, e)), Some(b)) = (sel, t.block.as_mut()) {
                    b.remove(a, e);
                    t.caret = a;
                }
                true
            }
            // Round 8: at a paragraph's start / end the neighbour in its box joins it.
            (Key::Backspace, false) if sel.is_none() && t.caret == 0 => {
                return self.join_paragraph(v, false);
            }
            (Key::Delete, false) if sel.is_none() && t.caret >= n => {
                return self.join_paragraph(v, true);
            }
            (Key::Backspace, _) if t.caret > 0 => {
                t.caret -= 1;
                if let Some(b) = t.block.as_mut() {
                    b.remove(t.caret, t.caret + 1);
                }
                true
            }
            (Key::Delete, _) if t.caret < n => {
                if let Some(b) = t.block.as_mut() {
                    b.remove(t.caret, t.caret + 1);
                }
                true
            }
            (Key::Backspace | Key::Delete, _) => false,
            (Key::Enter, _) => {
                if let (Some((a, e)), Some(b)) = (sel, t.block.as_mut()) {
                    b.remove(a, e);
                    t.caret = a;
                }
                if let Some(b) = t.block.as_mut() {
                    t.caret = b.insert(t.caret, "\n");
                }
                true
            }
            (Key::Escape, _) => {
                self.finish_typing(Some(v));
                return true;
            }
            _ => return false,
        };
        if let Some(t) = self.typing.as_mut() {
            t.anchor = None;
        }
        if changed {
            self.write(v);
        }
        true
    }

    /// Change the style of the selected text (all of the paragraph when nothing is
    /// selected).
    pub fn style(&mut self, v: &mut ReaderView, change: &StyleChange) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let range = t.selection().map_or(0..0, |(a, e)| a..e);
        let page = t.page;
        let Some(b) = t.block.as_mut() else {
            return false;
        };
        match v.doc_mut().block_edit_style(b, range, change) {
            Ok(()) => v.invalidate_page(page),
            Err(e) => self.status = Some(e),
        }
        true
    }

    /// Change the paragraph being typed into with `f`, then lay it out again.
    pub fn with_block(
        &mut self,
        v: &mut ReaderView,
        f: impl FnOnce(&mut BlockEdit, &mut usize),
    ) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let Some(b) = t.block.as_mut() else {
            return false;
        };
        f(b, &mut t.caret);
        t.anchor = None;
        self.write(v)
    }

    /// The paragraph being typed into.
    pub fn block(&self) -> Option<&BlockEdit> {
        self.typing.as_ref()?.block.as_ref()
    }

    /// Align the paragraph being typed into.
    pub fn align(&mut self, v: &mut ReaderView, align: Align) -> bool {
        let Some(b) = self.typing.as_mut().and_then(|t| t.block.as_mut()) else {
            return false;
        };
        b.align = align;
        self.write(v)
    }

    /// Style at the caret of the paragraph being typed into.
    pub fn current_style(&self) -> Option<(bo_pdfread::RunStyle, Align)> {
        let t = self.typing.as_ref()?;
        let b = t.block.as_ref()?;
        let at = t.selection().map_or(t.caret, |s| s.0 + 1);
        Some((b.style_at(at)?.clone(), b.align))
    }

    /// Lay the paragraph out again and write it into the page.
    pub(crate) fn write(&mut self, v: &mut ReaderView) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let page = t.page;
        let Some(b) = t.block.as_mut() else {
            return false;
        };
        let before = b.bbox;
        let shift = b.follower_shift();
        let old_boxes = b.char_boxes.clone();
        let caret = t.caret;
        match v.doc_mut().block_edit_update(b) {
            Ok(()) => {
                // Only the lines whose characters moved change (round 12: a 500-line
                // paragraph redraws one or two lines per key): draw that band of the page
                // again; down to the page's foot when what follows the paragraph moved.
                let (w, h) = v.page_size(page);
                let moved = (b.follower_shift() - shift).abs() > 1e-3;
                let (top, bottom) = changed_band(&old_boxes, &b.char_boxes, caret)
                    .unwrap_or((before[1].min(b.bbox[1]), before[3].max(b.bbox[3])));
                let r = [0.0, top - 4.0, w, if moved { h } else { bottom + 4.0 }];
                v.invalidate_band(page, r);
            }
            Err(e) => self.status = Some(e),
        }
        true
    }

    /// Caret rectangle in window pixels.
    pub fn caret(&self, v: &ReaderView) -> Option<[f32; 4]> {
        let t = self.typing.as_ref()?;
        let c = match &t.block {
            Some(b) => b.caret_box(t.caret),
            None => [t.bbox[0], t.bbox[1], t.bbox[0], t.bbox[3]],
        };
        let r = v.doc_rect_to_window(t.page, c)?;
        Some([
            r[0].round(),
            r[1],
            1.0_f32.max((v.scale() / 1.33).round()),
            r[3].max(8.0),
        ])
    }

    /// Paint hover, selection, drag preview, handles and the caret.
    pub fn paint(
        &mut self,
        p: &mut dyn Painter,
        v: &mut ReaderView,
        accent: Rgba,
        tool: EditTool,
        caret_on: bool,
    ) {
        self.paint_frames11(p, v, tool);
        let Some(page) = self.page else { return };
        // Selections are drawn in the reference's selection blue, not the app accent.
        let _ = accent;
        let accent = crate::editor_rot8::SEL_BLUE;
        let list = self.items(v, page).to_vec();
        let frame = v.frame();
        p.push_clip(frame[0], frame[1], frame[2], frame[3]);
        let outline = |p: &mut dyn Painter, r: [f32; 4], c: Rgba, w: f32| {
            let (x, y) = (r[0].round() - 0.5, r[1].round() - 0.5);
            let (x1, y1) = ((r[0] + r[2]).round() + 0.5, (r[1] + r[3]).round() + 0.5);
            p.path(
                &[
                    PathSeg::M(x, y),
                    PathSeg::L(x1, y),
                    PathSeg::L(x1, y1),
                    PathSeg::L(x, y1),
                    PathSeg::Z,
                ],
                None,
                Some((c, w)),
            );
        };
        let soft = Rgba(accent.0, accent.1, accent.2, 140);
        let typing = self.typing.is_some();
        if let Some(h) = self.hover.filter(|h| !self.selected.contains(h) && !typing) {
            if let Some(r) = list
                .get(h)
                .and_then(|it| v.doc_rect_to_window(page, it.bbox))
            {
                outline(p, r, soft, 1.0);
            }
        }
        let offset = match self.drag {
            Some(Drag::Move { from, to, copy }) => {
                // A copy: the originals keep a thin box where they are.
                if copy {
                    for &k in &self.selected {
                        if let Some(r) = list
                            .get(k)
                            .and_then(|it| v.doc_rect_to_window(page, it.bbox))
                        {
                            outline(p, r, soft, 1.0);
                        }
                    }
                }
                (to.0 - from.0, to.1 - from.1)
            }
            _ => (0.0, 0.0),
        };
        // Snapping guides across the page.
        for g in &self.guides {
            if let Some(r) = v.doc_rect_to_window(page, *g) {
                let c = Rgba(0xE0, 0x1E, 0x8C, 255);
                if r[2] < 1.0 {
                    p.fill_rect(r[0].round(), r[1], 1.0, r[3].max(1.0), c);
                } else {
                    p.fill_rect(r[0], r[1].round(), r[2].max(1.0), 1.0, c);
                }
            }
        }
        if let Some(Drag::Marquee { from, to }) = self.drag {
            let b = [
                from.0.min(to.0),
                from.1.min(to.1),
                from.0.max(to.0),
                from.1.max(to.1),
            ];
            if let Some(r) = v.doc_rect_to_window(page, b) {
                p.fill_rect(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    Rgba(accent.0, accent.1, accent.2, 30),
                );
                outline(p, r, accent, 1.0);
            }
        }
        for &k in self.selected.iter().filter(|_| !typing) {
            let Some(it) = list.get(k) else { continue };
            let b = it.bbox;
            let moved = [
                b[0] + offset.0,
                b[1] + offset.1,
                b[2] + offset.0,
                b[3] + offset.1,
            ];
            if let Some(r) = v.doc_rect_to_window(page, moved) {
                outline(p, r, accent, 1.0);
                if tool == EditTool::Object && self.selected.len() == 1 && self.typing.is_none() {
                    // The reference's handles: solid 6 px squares on the 1 px frame at the
                    // corners and side middles.
                    // The frame's left / top line is the pixel before the box, its right /
                    // bottom line the pixel after: each square has 3 px before that pixel.
                    let (x1, y1) = (r[0] + r[2], r[1] + r[3]);
                    for (_, (cx, cy)) in handle_points(r) {
                        let hx = if cx >= x1 {
                            cx.round() - 3.0
                        } else {
                            cx.round() - 4.0
                        };
                        let hy = if cy >= y1 {
                            cy.round() - 3.0
                        } else {
                            cy.round() - 4.0
                        };
                        p.fill_rect(hx, hy, 6.0, 6.0, accent);
                    }
                }
            }
        }
        if let Some(Drag::Resize { corner, to }) = self.drag {
            if let Some(it) = self.selected.first().and_then(|&k| list.get(k)) {
                let (r, _, _) = self.resized(it.bbox, corner, to);
                if let Some(w) = v.doc_rect_to_window(page, r) {
                    outline(p, w, accent, 1.0);
                }
            }
        }
        if let Some(t) = &self.typing {
            // The reference's text box frame: 1 px #99C1DA around the joined paragraphs with
            // eight square handles (white, #B3D1E4 edge) at the corners and side middles.
            let mut f = t.frame();
            if let Some(Drag::Wrap { to }) = self.drag {
                f[2] = to.max(f[0] + 12.0);
            }
            if let Some(r) = v.doc_rect_to_window(t.page, f) {
                let r = [r[0] - 2.0, r[1] - 1.0, r[2] + 4.0, r[3] + 2.0];
                let edge = Rgba(0x99, 0xC1, 0xDA, 255);
                outline(p, r, edge, 1.0);
                if t.block.is_some() {
                    let (x0, y0) = (r[0].round(), r[1].round());
                    let (x1, y1) = ((r[0] + r[2]).round(), (r[1] + r[3]).round());
                    let (xm, ym) = (f32::midpoint(x0, x1).round(), f32::midpoint(y0, y1).round());
                    for (cx, cy) in [
                        (x0, y0),
                        (xm, y0),
                        (x1, y0),
                        (x0, ym),
                        (x1, ym),
                        (x0, y1),
                        (xm, y1),
                        (x1, y1),
                    ] {
                        p.fill_rect(cx - 4.0, cy - 4.0, 8.0, 8.0, Rgba(0xB3, 0xD1, 0xE4, 255));
                        p.fill_rect(cx - 3.0, cy - 3.0, 6.0, 6.0, Rgba(255, 255, 255, 255));
                    }
                }
            }
            // Round 9: the selected text is drawn into the page image (`editor_selhi9`).
            if caret_on {
                if let Some(c) = self.caret(v) {
                    p.fill_rect(c[0], c[1], c[2], c[3], Rgba(0, 0, 0, 255));
                }
            }
        }
        p.pop_clip();
    }
}

/// Window points of the handles of a selection box `[x, y, w, h]` with their index: corners
/// 0–3 (top left, top right, bottom left, bottom right), side middles 4–7 (top, bottom, left,
/// right). A box under 16 px either way has no side handles (they would cover a thin
/// object such as a line).
pub(crate) fn handle_points(r: [f32; 4]) -> Vec<(usize, (f32, f32))> {
    let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    let (xm, ym) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
    let all = [
        (x0, y0),
        (x1, y0),
        (x0, y1),
        (x1, y1),
        (xm, y0),
        (xm, y1),
        (x0, ym),
        (x1, ym),
    ];
    all.into_iter()
        .enumerate()
        .filter(|&(i, _)| i < 4 || (r[2] >= 16.0 && r[3] >= 16.0))
        .collect()
}

/// A box stretched by a side handle (4 top, 5 bottom, 6 left, 7 right): the opposite side
/// stays, the other direction keeps its size.
fn side_resized(b: [f32; 4], side: usize, to: (f32, f32)) -> ([f32; 4], f32, f32) {
    let (w0, h0) = ((b[2] - b[0]).max(0.01), (b[3] - b[1]).max(0.01));
    if side < 6 {
        let ay = if side == 4 { b[3] } else { b[1] };
        let sy = (to.1 - ay).abs() / h0;
        let y0 = if to.1 < ay { ay - h0 * sy } else { ay };
        ([b[0], y0, b[2], y0 + h0 * sy], 1.0, sy)
    } else {
        let ax = if side == 6 { b[2] } else { b[0] };
        let sx = (to.0 - ax).abs() / w0;
        let x0 = if to.0 < ax { ax - w0 * sx } else { ax };
        ([x0, b[1], x0 + w0 * sx, b[3]], sx, 1.0)
    }
}

/// Vertical extent (page points) of the characters whose boxes differ between two layouts of
/// a paragraph after one contiguous edit, plus the caret's neighbours: the common prefix and
/// suffix kept their places. `None` when nothing is known (empty layouts).
fn changed_band(old: &[[f32; 4]], new: &[[f32; 4]], caret: usize) -> Option<(f32, f32)> {
    let same = |a: &[f32; 4], b: &[f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
    let a = old.iter().zip(new).take_while(|(x, y)| same(x, y)).count();
    let room = old.len().min(new.len()) - a;
    let k = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(x, y)| same(x, y))
        .count();
    let mut span: Option<(f32, f32)> = None;
    let mut add = |r: &[f32; 4]| {
        let (t, b) = (r[1].min(r[3]), r[1].max(r[3]));
        span = Some(span.map_or((t, b), |(t0, b0)| (t0.min(t), b0.max(b))));
    };
    old[a..old.len() - k].iter().for_each(&mut add);
    new[a..new.len() - k].iter().for_each(&mut add);
    for i in [caret.saturating_sub(1), caret] {
        if let Some(r) = new.get(i) {
            add(r);
        }
    }
    span
}

#[cfg(test)]
mod band_tests {
    /// Typing on line 2 of a 4-line paragraph changes only that line's band.
    #[test]
    fn changed_band_covers_the_edited_line_only() {
        let line = |l: usize, n: usize| -> Vec<[f32; 4]> {
            (0..n)
                .map(|i| {
                    let x = i as f32 * 5.0;
                    let y = l as f32 * 20.0;
                    [x, y, x + 5.0, y + 10.0]
                })
                .collect()
        };
        let old: Vec<[f32; 4]> = (0..4).flat_map(|l| line(l, 10)).collect();
        let mut new = old.clone();
        // A character inserted at index 15 (line 1): the rest of line 1 moves right.
        new.insert(15, [75.0, 20.0, 80.0, 30.0]);
        for r in &mut new[16..21] {
            r[0] += 5.0;
            r[2] += 5.0;
        }
        let (top, bottom) = super::changed_band(&old, &new, 16).expect("band");
        assert!(
            (top - 20.0).abs() < 1e-3 && (bottom - 30.0).abs() < 1e-3,
            "{top} {bottom}"
        );
    }
}
