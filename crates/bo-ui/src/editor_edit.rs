//! BlueEditor's Edit Text / Edit Object / Add Text tools on the document view: hover and
//! selection boxes, dragging to move, corner handles to resize, typing into a text line in
//! place (each keystroke rewrites the run and the page redraws), Delete, Escape.
use crate::editor_window::EditTool;
use crate::reader_view::ReaderView;
use crate::{Key, Mods, Painter};
use bo_pdfread::{text_lines, ObjectKind, PageObject, TextLine};
use bo_render::{PathSeg, Rgba};

/// Something the tools select: a text line (several runs) or another page object.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    /// Page object indices.
    pub ids: Vec<usize>,
    /// Box in document page points.
    pub bbox: [f32; 4],
    pub kind: ObjectKind,
    pub line: Option<TextLine>,
}

/// Text being typed into.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Typing {
    pub page: usize,
    /// Runs that hold the text now (one after the first keystroke).
    pub ids: Vec<usize>,
    pub text: Vec<char>,
    pub caret: usize,
    /// All text selected (the next key replaces it).
    pub all: bool,
    /// Character boxes in document points (one per character).
    pub boxes: Vec<[f32; 4]>,
    pub bbox: [f32; 4],
    /// A new text block not written yet: its baseline origin and size.
    pub new_at: Option<([f32; 2], f32)>,
    /// An undo group is open for this typing.
    pub grouped: bool,
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
    let mut out: Vec<Item> = text_lines(objs)
        .into_iter()
        .map(|l| Item {
            ids: l.ids.clone(),
            bbox: l.bbox,
            kind: ObjectKind::Text,
            line: Some(l),
        })
        .collect();
    for (i, o) in objs.iter().enumerate() {
        if o.kind != ObjectKind::Text {
            out.push(Item {
                ids: vec![i],
                bbox: o.bbox,
                kind: o.kind,
                line: None,
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

    /// Items of page `i` (analysed again after every edit).
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
        if tool == EditTool::AddText {
            self.finish_typing(Some(v));
            let size = 12.0;
            self.typing = Some(Typing {
                page,
                ids: Vec::new(),
                text: Vec::new(),
                caret: 0,
                all: false,
                boxes: Vec::new(),
                bbox: [px, py, px + 2.0, py + size * 1.2],
                new_at: Some(([px, py + size * 0.9], size)),
                grouped: false,
            });
            return;
        }
        let hit = self.item_at(v, page, px, py, tool);
        let Some(k) = hit else {
            self.finish_typing(Some(v));
            self.selected.clear();
            return;
        };
        let item = self.items(v, page).get(k).cloned();
        let Some(item) = item else { return };
        if tool == EditTool::Text || (count >= 2 && item.kind == ObjectKind::Text) {
            if let Some(t) = &self.typing {
                if t.page == page && t.ids == item.ids {
                    // Click inside the text being edited: move the caret.
                    let caret = caret_at(&t.boxes, px);
                    if let Some(t) = self.typing.as_mut() {
                        t.caret = caret;
                        t.all = count >= 3;
                    }
                    return;
                }
            }
            self.finish_typing(Some(v));
            if let Some(line) = item.line {
                let caret = caret_at(&line.char_boxes, px);
                v.doc_mut().begin_group("edit text");
                self.typing = Some(Typing {
                    page,
                    ids: line.ids.clone(),
                    text: line.text.chars().collect(),
                    caret,
                    all: count >= 2,
                    boxes: line.char_boxes.clone(),
                    bbox: line.bbox,
                    new_at: None,
                    grouped: true,
                });
                self.selected = vec![k];
            }
            return;
        }
        self.finish_typing(Some(v));
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
            if let (Some(v), true) = (v, t.grouped) {
                v.doc_mut().end_group();
            }
        }
    }

    /// Typed text.
    pub fn type_text(&mut self, v: &mut ReaderView, s: &str) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        if t.all {
            t.text.clear();
            t.caret = 0;
            t.all = false;
        }
        for c in s.chars().filter(|c| !c.is_control()) {
            t.text.insert(t.caret, c);
            t.caret += 1;
        }
        self.write(v)
    }

    /// A key while typing; false when it is not for the text.
    pub fn key(&mut self, v: &mut ReaderView, k: Key, mods: Mods) -> bool {
        let Some(t) = self.typing.as_mut() else {
            return false;
        };
        let n = t.text.len();
        let changed = match (k, mods.ctrl) {
            (Key::Left, _) => {
                t.caret = if t.all { 0 } else { t.caret.saturating_sub(1) };
                t.all = false;
                false
            }
            (Key::Right, _) => {
                t.caret = if t.all { n } else { (t.caret + 1).min(n) };
                t.all = false;
                false
            }
            (Key::Home, _) => {
                t.caret = 0;
                t.all = false;
                false
            }
            (Key::End, _) => {
                t.caret = n;
                t.all = false;
                false
            }
            (Key::Letter('a'), true) => {
                t.all = true;
                false
            }
            (Key::Backspace | Key::Delete, _) if t.all => {
                t.text.clear();
                t.caret = 0;
                t.all = false;
                true
            }
            (Key::Backspace, _) if t.caret > 0 => {
                t.caret -= 1;
                t.text.remove(t.caret);
                true
            }
            (Key::Delete, _) if t.caret < n => {
                t.text.remove(t.caret);
                true
            }
            (Key::Backspace | Key::Delete, _) => false,
            (Key::Enter | Key::Escape, _) => {
                self.finish_typing(Some(v));
                return true;
            }
            _ => return false,
        };
        if changed {
            self.write(v);
        }
        true
    }

    /// Write the typed text into the page.
    fn write(&mut self, v: &mut ReaderView) -> bool {
        let Some(t) = self.typing.clone() else {
            return false;
        };
        let text: String = t.text.iter().collect();
        let r = match t.new_at {
            Some((at, size)) if t.ids.is_empty() => {
                if text.is_empty() {
                    return true;
                }
                if !t.grouped {
                    v.doc_mut().begin_group("add text");
                    if let Some(t) = self.typing.as_mut() {
                        t.grouped = true;
                    }
                }
                v.doc_mut().add_text(t.page, at, &text, size, [0, 0, 0])
            }
            _ => v.doc_mut().replace_text_runs(t.page, &t.ids, &text),
        };
        match r {
            Ok(id) => {
                v.invalidate_page(t.page);
                self.cache = None;
                let obj = v
                    .doc_mut()
                    .page_objects(t.page)
                    .ok()
                    .and_then(|o| o.get(id).cloned());
                if let Some(t) = self.typing.as_mut() {
                    t.ids = if text.is_empty() {
                        Vec::new()
                    } else {
                        vec![id]
                    };
                    if let Some(o) = obj.filter(|_| !text.is_empty()) {
                        t.boxes = o.char_boxes.clone();
                        t.boxes
                            .resize(t.text.len(), [o.bbox[2], o.bbox[1], o.bbox[2], o.bbox[3]]);
                        t.bbox = o.bbox;
                    } else {
                        t.boxes.clear();
                        t.bbox = [t.bbox[0], t.bbox[1], t.bbox[0] + 2.0, t.bbox[3]];
                    }
                    if text.is_empty() && t.new_at.is_none() {
                        // The line is gone; typing again writes a new run where it was.
                        t.new_at = Some((
                            [t.bbox[0], t.bbox[3] - (t.bbox[3] - t.bbox[1]) * 0.2],
                            (t.bbox[3] - t.bbox[1]) / 1.2,
                        ));
                    }
                }
                true
            }
            Err(e) => {
                self.status = Some(e);
                true
            }
        }
    }

    /// Caret rectangle in window pixels.
    pub fn caret(&self, v: &ReaderView) -> Option<[f32; 4]> {
        let t = self.typing.as_ref()?;
        let b = t.bbox;
        let x = if t.caret == 0 {
            t.boxes.first().map_or(b[0], |c| c[0])
        } else {
            t.boxes.get(t.caret - 1).map_or(b[2], |c| c[2])
        };
        let r = v.doc_rect_to_window(t.page, [x, b[1], x, b[3]])?;
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
        if let Some(h) = self.hover.filter(|h| !self.selected.contains(h)) {
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
        for &k in &self.selected {
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
            if let Some(r) = v.doc_rect_to_window(t.page, t.bbox) {
                outline(
                    p,
                    [r[0] - 2.0, r[1] - 1.0, r[2] + 4.0, r[3] + 2.0],
                    accent,
                    1.0,
                );
            }
            if t.all {
                if let Some(r) = v.doc_rect_to_window(t.page, t.bbox) {
                    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0, 120, 215, 70));
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

/// Caret index nearest to `x` among character boxes.
fn caret_at(boxes: &[[f32; 4]], x: f32) -> usize {
    boxes
        .iter()
        .position(|b| x < f32::midpoint(b[0], b[2]))
        .unwrap_or(boxes.len())
}
