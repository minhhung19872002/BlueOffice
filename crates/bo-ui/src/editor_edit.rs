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
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    /// Moving the selection from a document point.
    Move { from: (f32, f32), to: (f32, f32) },
    /// Resizing the single selected item by a corner (0 top-left … 3 bottom-right).
    Resize { corner: usize, to: (f32, f32) },
}

/// Editing state of the active document.
#[derive(Debug, Default)]
pub(crate) struct Editing {
    /// Items of `page`, valid for `edits` (the document's edit count).
    cache: Option<(usize, u32, Vec<Item>)>,
    pub page: Option<usize>,
    pub selected: Vec<usize>,
    pub hover: Option<usize>,
    pub typing: Option<Typing>,
    drag: Option<Drag>,
    pub status: Option<String>,
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]
}

fn items(objs: &[PageObject]) -> Vec<Item> {
    let mut out: Vec<Item> = text_blocks(objs)
        .into_iter()
        .map(|b| Item {
            ids: b.ids.clone(),
            bbox: b.bbox,
            kind: ObjectKind::Text,
            block: Some(b),
        })
        .collect();
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
    /// Forget everything (another document, tool off).
    pub fn reset(&mut self, v: Option<&mut ReaderView>) {
        self.finish_typing(v);
        self.cache = None;
        self.page = None;
        self.selected.clear();
        self.hover = None;
        self.drag = None;
    }

    /// Items of page `i` (analysed again after every edit; kept while typing, the typed
    /// paragraph draws from its session).
    pub fn items(&mut self, v: &mut ReaderView, i: usize) -> &[Item] {
        let edits = v.doc().edit_count();
        let fresh = matches!(&self.cache, Some((p, e, _)) if *p == i && *e == edits);
        if !fresh {
            let list = v
                .doc_mut()
                .page_objects(i)
                .map(|o| items(&o))
                .unwrap_or_default();
            self.cache = Some((i, edits, list));
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
        let list = self.items(v, page);
        // Text first for the text tool; the smallest box under the point otherwise.
        let mut best: Option<(usize, f32)> = None;
        for (k, it) in list.iter().enumerate() {
            if tool == EditTool::Text && it.kind != ObjectKind::Text {
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
            });
            return;
        }
        let hit = self.item_at(v, page, px, py, tool);
        let Some(k) = hit else {
            self.selected.clear();
            return;
        };
        let item = self.items(v, page).get(k).cloned();
        let Some(item) = item else { return };
        if tool == EditTool::Text || (count >= 2 && item.kind == ObjectKind::Text) {
            if let Some(b) = item.block {
                match v.doc_mut().begin_block_edit(page, &b) {
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
        self.drag = Some(Drag::Move {
            from: (px, py),
            to: (px, py),
        });
    }

    /// Corner handle of the single selected item under a window point.
    fn handle_at(&mut self, v: &mut ReaderView, page: usize, x: f32, y: f32) -> Option<usize> {
        let k = *self.selected.first()?;
        let b = self.items(v, page).get(k)?.bbox;
        let r = v.doc_rect_to_window(page, b)?;
        let corners = [
            (r[0], r[1]),
            (r[0] + r[2], r[1]),
            (r[0], r[1] + r[3]),
            (r[0] + r[2], r[1] + r[3]),
        ];
        corners
            .iter()
            .position(|&(cx, cy)| (x - cx).abs() <= 5.0 && (y - cy).abs() <= 5.0)
    }

    /// Button down and moving.
    pub fn drag(&mut self, v: &ReaderView, x: f32, y: f32) -> bool {
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
        match self.drag.as_mut() {
            Some(Drag::Move { to, .. } | Drag::Resize { to, .. }) => {
                *to = (r[0], r[1]);
                true
            }
            None => false,
        }
    }

    /// Button released: apply a move or resize.
    pub fn release(&mut self, v: &mut ReaderView) -> bool {
        if let Some(t) = self.typing.as_mut() {
            t.selecting = false;
        }
        let Some(page) = self.page else { return false };
        match self.drag.take() {
            Some(Drag::Move { from, to }) => {
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                    return false;
                }
                let ids = self.selected_ids(v, page);
                let r = v
                    .doc_mut()
                    .transform_objects(page, &ids, [1.0, 0.0, 0.0, 1.0, dx, dy]);
                self.after_edit(v, page, r.map(|()| ids.len()));
                // The moved items are new objects: select them again by position.
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
                // The opposite corner stays.
                let (ax, ay) = match corner {
                    0 => (b[2], b[3]),
                    1 => (b[0], b[3]),
                    2 => (b[2], b[1]),
                    _ => (b[0], b[1]),
                };
                let (w0, h0) = (b[2] - b[0], b[3] - b[1]);
                let (w1, h1) = ((to.0 - ax).abs(), (to.1 - ay).abs());
                if w0 < 0.1 || h0 < 0.1 || w1 < 2.0 || h1 < 2.0 {
                    return false;
                }
                let (sx, sy) = (w1 / w0, h1 / h0);
                let m = [sx, 0.0, 0.0, sy, ax - ax * sx, ay - ay * sy];
                let r = v.doc_mut().transform_objects(page, &item.ids, m);
                self.after_edit(v, page, r.map(|()| 1));
                true
            }
            None => false,
        }
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

    /// First characters of an Add Text block: write them, then edit the new paragraph.
    fn start_new_text(&mut self, v: &mut ReaderView, s: &str) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let Some((at, size)) = t.new_at else {
            return false;
        };
        let page = t.page;
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
                let blk = v.doc_mut().begin_block_edit(page, &b)?;
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
        let changed = match (k, mods.ctrl) {
            (Key::Letter('a'), true) => {
                t.anchor = Some(0);
                t.caret = n;
                return true;
            }
            (Key::Letter('b'), true) => {
                let on = !b.style_at(t.caret).is_some_and(|s| s.bold);
                return self.style(v, &StyleChange::Bold(on));
            }
            (Key::Letter('i'), true) => {
                let on = !b.style_at(t.caret).is_some_and(|s| s.italic);
                return self.style(v, &StyleChange::Italic(on));
            }
            (Key::Backspace | Key::Delete, _) if sel.is_some() => {
                if let (Some((a, e)), Some(b)) = (sel, t.block.as_mut()) {
                    b.remove(a, e);
                    t.caret = a;
                }
                true
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
    fn write(&mut self, v: &mut ReaderView) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let page = t.page;
        let Some(b) = t.block.as_mut() else {
            return false;
        };
        match v.doc_mut().block_edit_update(b) {
            Ok(()) => v.invalidate_page(page),
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
        let Some(page) = self.page else { return };
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
            Some(Drag::Move { from, to }) => (to.0 - from.0, to.1 - from.1),
            _ => (0.0, 0.0),
        };
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
                    for (cx, cy) in [
                        (r[0], r[1]),
                        (r[0] + r[2], r[1]),
                        (r[0], r[1] + r[3]),
                        (r[0] + r[2], r[1] + r[3]),
                    ] {
                        p.fill_rect(cx.round() - 3.0, cy.round() - 3.0, 7.0, 7.0, accent);
                        p.fill_rect(
                            cx.round() - 2.0,
                            cy.round() - 2.0,
                            5.0,
                            5.0,
                            Rgba(255, 255, 255, 255),
                        );
                    }
                }
            }
        }
        if let Some(Drag::Resize { corner, to }) = self.drag {
            if let Some(it) = self.selected.first().and_then(|&k| list.get(k)) {
                let b = it.bbox;
                let (ax, ay) = match corner {
                    0 => (b[2], b[3]),
                    1 => (b[0], b[3]),
                    2 => (b[2], b[1]),
                    _ => (b[0], b[1]),
                };
                let r = [ax.min(to.0), ay.min(to.1), ax.max(to.0), ay.max(to.1)];
                if let Some(w) = v.doc_rect_to_window(page, r) {
                    outline(p, w, accent, 1.0);
                }
            }
        }
        if let Some(t) = &self.typing {
            if let Some(r) = v.doc_rect_to_window(t.page, t.bbox()) {
                outline(
                    p,
                    [r[0] - 2.0, r[1] - 1.0, r[2] + 4.0, r[3] + 2.0],
                    accent,
                    1.0,
                );
            }
            if let (Some((a, e)), Some(b)) = (t.selection(), &t.block) {
                for s in b.selection_rects(a, e) {
                    if let Some(r) = v.doc_rect_to_window(t.page, s) {
                        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0, 120, 215, 70));
                    }
                }
            }
            if caret_on {
                if let Some(c) = self.caret(v) {
                    p.fill_rect(c[0], c[1], c[2], c[3], Rgba(0, 0, 0, 255));
                }
            }
        }
        p.pop_clip();
    }
}
