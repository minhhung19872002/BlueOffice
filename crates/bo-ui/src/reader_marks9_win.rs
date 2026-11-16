//! BlueReader round 9 (a child of `reader_window`): editing bookmarks and sizing thumbnails.
//!
//! Bookmarks panel: add (after the selected bookmark, titled with the selected text, pointing at
//! the current page), rename in place (F2, Enter / Esc), delete, promote / demote, drag a
//! bookmark onto another to move it before that one; each change rewrites `/Outlines`
//! (`PdfDocument::set_outline`) and is saved as an incremental update by File › Save.
//! Pages panel: a tool row and a context menu like the reference reader's (enlarge / reduce
//! thumbnails, print pages, properties — page changes are not offered by a reader), Ctrl + wheel
//! over the panel sizes the thumbnails, and the current thumbnail shows the visible area.
use super::{inside, Ink, LinkTarget, Panel, PanelRow, ReaderWindow, TEXT};
use crate::popup::{Choice, MenuItem};
use crate::reader_icons;
use crate::{Input, Key, Painter, TextMeasure};
use bo_pdfread::OutlineItem;
use bo_render::Rgba;

/// Thumbnail sizes (fractions of the measured width), the default at index 2.
const THUMB_STEPS: [f32; 7] = [0.6, 0.8, 1.0, 1.25, 1.5, 2.0, 2.5];
/// Pages panel: list top under the panel top (logical px, the reference's 95 device px at 125 %).
pub(crate) const THUMB_TOP: f32 = 76.0;
/// Visible area mark on the current thumbnail (the reference's red).
const VIEW_MARK: Rgba = Rgba(226, 0, 26, 255);

/// Round 9 state of the Bookmarks and Pages panels.
#[derive(Debug, Default)]
pub(crate) struct Marks9 {
    /// Bookmark being renamed and its new title.
    pub editing: Option<(Vec<usize>, String)>,
    /// A press on a bookmark row (path, y) and whether it became a drag; drop target row.
    press: Option<(Vec<usize>, f32)>,
    dragging: bool,
    drop: Option<Vec<usize>>,
    /// The last press was in the navigation panel (F2 / Delete act on bookmarks).
    pub panel_focus: bool,
    /// Index into [`THUMB_STEPS`] offset from the default.
    pub thumb_step: i32,
    /// Pages panel tool boxes.
    tools: Vec<(&'static str, [f32; 4])>,
}

/// The list holding the item at `path` and its index there.
fn parent_of<'a>(
    items: &'a mut Vec<OutlineItem>,
    path: &[usize],
) -> Option<(&'a mut Vec<OutlineItem>, usize)> {
    let (&last, head) = path.split_last()?;
    let mut list = items;
    for &k in head {
        list = &mut list.get_mut(k)?.children;
    }
    (last < list.len()).then_some((list, last))
}

/// Move the item at `from` to just before the item at `to` (same list as `to`); the new path.
fn move_before(items: &mut Vec<OutlineItem>, from: &[usize], to: &[usize]) -> Option<Vec<usize>> {
    if from == to || to.starts_with(from) {
        return None;
    }
    let item = {
        let (list, i) = parent_of(items, from)?;
        list.remove(i)
    };
    // Removing `from` shifts `to` when they share a parent and `from` came first.
    let mut to = to.to_vec();
    let depth = from.len() - 1;
    if to.len() > depth && to[..depth] == from[..depth] && to[depth] > from[depth] {
        to[depth] -= 1;
    }
    let (list, i) = parent_of(items, &to)?;
    list.insert(i, item);
    Some(to)
}

/// Promote: the item becomes the next sibling of its parent.
fn promote(items: &mut Vec<OutlineItem>, path: &[usize]) -> Option<Vec<usize>> {
    if path.len() < 2 {
        return None;
    }
    let item = {
        let (list, i) = parent_of(items, path)?;
        list.remove(i)
    };
    let parent = &path[..path.len() - 1];
    let (list, i) = parent_of(items, parent)?;
    list.insert(i + 1, item);
    let mut p = parent.to_vec();
    if let Some(l) = p.last_mut() {
        *l += 1;
    }
    Some(p)
}

/// Demote: the item becomes the last child of its previous sibling.
fn demote(items: &mut Vec<OutlineItem>, path: &[usize]) -> Option<Vec<usize>> {
    let (list, i) = parent_of(items, path)?;
    if i == 0 {
        return None;
    }
    let item = list.remove(i);
    let prev = &mut list[i - 1];
    prev.children.push(item);
    let mut p = path.to_vec();
    if let Some(l) = p.last_mut() {
        *l = i - 1;
    }
    p.push(prev.children.len() - 1);
    Some(p)
}

impl ReaderWindow {
    /// Thumbnail width (logical px).
    pub(crate) fn thumb_w9(&self) -> f32 {
        let k = (2 + self.marks9.thumb_step).clamp(0, THUMB_STEPS.len() as i32 - 1);
        #[allow(clippy::cast_sign_loss)]
        let f = THUMB_STEPS[k as usize];
        super::THUMB_W * f
    }

    fn step_thumbs9(&mut self, d: i32) {
        let lo = -2;
        let hi = THUMB_STEPS.len() as i32 - 3;
        self.marks9.thumb_step = (self.marks9.thumb_step + d).clamp(lo, hi);
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.panel_scroll = 0.0;
        }
    }

    fn outline9(&self) -> Vec<OutlineItem> {
        self.tabs
            .get(self.active)
            .map(|t| t.view.doc().outline())
            .unwrap_or_default()
    }

    /// Write `items` as the outline, keep the open branches, select `select`.
    fn write_outline9(&mut self, items: &[OutlineItem], select: Option<Vec<usize>>) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let Err(e) = t.view.doc_mut().set_outline(items) {
            self.status = e;
            return;
        }
        let expanded = t.expanded.clone();
        self.document_changed();
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.expanded = expanded;
            if let Some(s) = &select {
                for n in 1..s.len() {
                    t.expanded.insert(s[..n].to_vec());
                }
            }
        }
        self.selected_outline = select;
    }

    /// Bookmark and thumbnail commands; true when handled.
    pub(crate) fn command_marks9(&mut self, cmd: &str) -> bool {
        let sel = self.selected_outline.clone();
        match cmd {
            "bm9.add" => {
                let mut items = self.outline9();
                let (page, title) = self.view().map_or((0, String::new()), |v| {
                    let t = v
                        .selected_text()
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                        .map(|s| s.chars().take(80).collect::<String>());
                    (v.current_page(), t.unwrap_or_default())
                });
                let title = if title.is_empty() {
                    self.t("pdfMarks9.untitled")
                } else {
                    title
                };
                let item = OutlineItem {
                    title: title.clone(),
                    target: Some(LinkTarget::Page {
                        index: page,
                        top: None,
                    }),
                    open: false,
                    children: Vec::new(),
                };
                let path = if let Some((list, i)) =
                    sel.as_deref().and_then(|s| parent_of(&mut items, s))
                {
                    list.insert(i + 1, item);
                    let mut p = sel.clone().unwrap_or_default();
                    if let Some(l) = p.last_mut() {
                        *l = i + 1;
                    }
                    p
                } else {
                    items.push(item);
                    vec![items.len() - 1]
                };
                self.write_outline9(&items, Some(path.clone()));
                self.set_panel(Some(Panel::Bookmarks));
                self.marks9.editing = Some((path, title));
            }
            "bm9.rename" => {
                if let Some(p) = sel {
                    let mut items = self.outline9();
                    if let Some((list, i)) = parent_of(&mut items, &p) {
                        self.marks9.editing = Some((p, list[i].title.clone()));
                    }
                }
            }
            "bm9.delete" => {
                if let Some(p) = sel {
                    let mut items = self.outline9();
                    if let Some((list, i)) = parent_of(&mut items, &p) {
                        list.remove(i);
                        self.write_outline9(&items, None);
                    }
                }
            }
            "bm9.promote" | "bm9.demote" => {
                if let Some(p) = sel {
                    let mut items = self.outline9();
                    let moved = if cmd == "bm9.promote" {
                        promote(&mut items, &p)
                    } else {
                        demote(&mut items, &p)
                    };
                    if let Some(np) = moved {
                        self.write_outline9(&items, Some(np));
                    }
                }
            }
            "thumb9.larger" => self.step_thumbs9(1),
            "thumb9.smaller" => self.step_thumbs9(-1),
            _ => return false,
        }
        true
    }

    fn finish_rename9(&mut self, keep: bool) {
        let Some((path, title)) = self.marks9.editing.take() else {
            return;
        };
        let title = title.trim().to_owned();
        if !keep || title.is_empty() {
            return;
        }
        let mut items = self.outline9();
        if let Some((list, i)) = parent_of(&mut items, &path) {
            if list[i].title != title {
                list[i].title = title;
                self.write_outline9(&items, Some(path));
            }
        }
    }

    fn outline_row_at9(&self, x: f32, y: f32) -> Option<Vec<usize>> {
        if !inside(self.panel_rect, x, y) {
            return None;
        }
        self.panel_rows.iter().find_map(|(ry, rh, row)| match row {
            PanelRow::Outline(p, ..) if y >= *ry && y < ry + rh => Some(p.clone()),
            _ => None,
        })
    }

    fn bookmark_menu9(&mut self, x: f32, y: f32, m: &dyn TextMeasure) {
        let has = self.selected_outline.is_some();
        let item = |label: String, cmd: &str, dim: bool| MenuItem {
            label,
            choice: Choice::Command(cmd.to_owned()),
            checked: false,
            dim,
        };
        let items = vec![
            item(self.t("pdfMarks9.add"), "bm9.add", false),
            item(self.t("pdfMarks9.rename"), "bm9.rename", !has),
            item(self.t("pdfMarks9.delete"), "bm9.delete", !has),
            item(self.t("pdfMarks9.promote"), "bm9.promote", !has),
            item(self.t("pdfMarks9.demote"), "bm9.demote", !has),
            item(self.t("pdfBookmark.expandAll"), "bm.expandAll", false),
            item(self.t("pdfBookmark.collapseAll"), "bm.collapseAll", false),
        ];
        let menu = self
            .context_menu9(items, [x, y, 0.0, 0.0])
            .with_decor(Vec::new(), vec![1, 3, 5]);
        let menu = self.keyed_menu9(menu, &["Ctrl+B", "F2", "Del", "", "", "", ""], m);
        self.menu = Some((menu, "bookmarks9"));
    }

    fn thumb_menu9(&mut self, x: f32, y: f32, m: &dyn TextMeasure) {
        let item = |label: String, cmd: &str| MenuItem {
            label,
            choice: Choice::Command(cmd.to_owned()),
            checked: false,
            dim: false,
        };
        let items = vec![
            item(self.t("pdfMarks9.enlarge"), "thumb9.larger"),
            item(self.t("pdfMarks9.reduce"), "thumb9.smaller"),
            item(self.t("pdfMarks9.printPages"), "file.print"),
            item(self.t("pdfMarks9.properties"), "file.properties"),
        ];
        let menu = self
            .context_menu9(items, [x, y, 0.0, 0.0])
            .with_decor(Vec::new(), vec![2]);
        let menu = self.keyed_menu9(menu, &[], m);
        self.menu = Some((menu, "thumbs9"));
    }

    /// Input of the Bookmarks / Pages panel additions; `Some(redraw)` when taken.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn route_marks9(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if self.profile.is_some() || self.tabs.is_empty() {
            return None;
        }
        let panel = self.panel_open();
        // Renaming in place takes the keyboard.
        if self.marks9.editing.is_some() {
            match input {
                Input::Text(s) => {
                    if let Some((_, t)) = self.marks9.editing.as_mut() {
                        t.push_str(s);
                    }
                    let st = self.style(14.0, false);
                    let q = self
                        .marks9
                        .editing
                        .as_ref()
                        .map(|e| e.1.clone())
                        .unwrap_or_default();
                    self.measure(m, &q, st);
                    return Some(true);
                }
                Input::Key(Key::Backspace, _) => {
                    if let Some((_, t)) = self.marks9.editing.as_mut() {
                        t.pop();
                    }
                    return Some(true);
                }
                Input::Key(Key::Enter, _) => {
                    self.finish_rename9(true);
                    return Some(true);
                }
                Input::Key(Key::Escape, _) => {
                    self.finish_rename9(false);
                    return Some(true);
                }
                Input::Press { .. } => self.finish_rename9(true),
                _ => {}
            }
        }
        match input {
            Input::Key(Key::Letter('b'), mods) if mods.ctrl && !mods.shift => {
                self.command_marks9("bm9.add");
                Some(true)
            }
            Input::Key(Key::F(2), _)
                if panel == Some(Panel::Bookmarks) && self.selected_outline.is_some() =>
            {
                self.command_marks9("bm9.rename");
                Some(true)
            }
            Input::Key(Key::Delete, _)
                if panel == Some(Panel::Bookmarks)
                    && self.marks9.panel_focus
                    && self.selected_outline.is_some() =>
            {
                self.command_marks9("bm9.delete");
                Some(true)
            }
            Input::Zoom(n)
                if panel == Some(Panel::Pages)
                    && inside(self.panel_rect, self.pointer.0, self.pointer.1) =>
            {
                self.step_thumbs9(if *n > 0.0 { 1 } else { -1 });
                Some(true)
            }
            Input::Context { x, y }
                if panel == Some(Panel::Bookmarks) && inside(self.panel_rect, *x, *y) =>
            {
                if let Some(p) = self.outline_row_at9(*x, *y) {
                    self.selected_outline = Some(p);
                }
                self.bookmark_menu9(*x, *y, m);
                Some(true)
            }
            Input::Context { x, y }
                if panel == Some(Panel::Pages) && inside(self.panel_rect, *x, *y) =>
            {
                self.thumb_menu9(*x, *y, m);
                Some(true)
            }
            Input::Press { x, y, .. } => {
                self.marks9.panel_focus = inside(self.panel_rect, *x, *y);
                if panel == Some(Panel::Pages) {
                    if let Some(id) = self
                        .marks9
                        .tools
                        .iter()
                        .find(|t| inside(t.1, *x, *y))
                        .map(|t| t.0)
                    {
                        self.command_marks9(id);
                        return Some(true);
                    }
                }
                self.marks9.press = if panel == Some(Panel::Bookmarks) {
                    self.outline_row_at9(*x, *y).map(|p| (p, *y))
                } else {
                    None
                };
                self.marks9.dragging = false;
                self.marks9.drop = None;
                None
            }
            Input::Drag { x, y } if self.marks9.press.is_some() => {
                let y0 = self.marks9.press.as_ref().map_or(*y, |p| p.1);
                if !self.marks9.dragging && (y - y0).abs() < self.px(4.0) {
                    return None;
                }
                self.marks9.dragging = true;
                self.marks9.drop = self.outline_row_at9(*x, *y);
                Some(true)
            }
            Input::Release { .. } if self.marks9.press.is_some() => {
                let (from, _) = self.marks9.press.take()?;
                let drop = self.marks9.drop.take();
                if !std::mem::take(&mut self.marks9.dragging) {
                    return None;
                }
                if let Some(to) = drop {
                    let mut items = self.outline9();
                    if let Some(np) = move_before(&mut items, &from, &to) {
                        self.write_outline9(&items, Some(np));
                    }
                }
                Some(true)
            }
            _ => None,
        }
    }

    /// Pages panel tool row (enlarge / reduce); returns the list top under the panel top.
    pub(crate) fn paint_thumb_tools9(&mut self, p: &mut dyn Painter, r: [f32; 4]) -> f32 {
        let is = self.px(17.6);
        let cy = r[1] + self.px(41.6);
        self.marks9.tools.clear();
        for (k, (id, icon)) in [
            ("thumb9.larger", "thumbLarger"),
            ("thumb9.smaller", "thumbSmaller"),
        ]
        .into_iter()
        .enumerate()
        {
            #[allow(clippy::cast_precision_loss)]
            let cx = r[0] + self.px(15.2 + 24.8 * k as f32);
            let b = [
                (cx - is / 2.0 - 2.0).round(),
                (cy - is / 2.0 - 2.0).round(),
                is + 4.0,
                is + 4.0,
            ];
            if inside(b, self.pointer.0, self.pointer.1) {
                p.fill_rect(b[0], b[1], b[2], b[3], self.look.hover);
            }
            reader_icons::draw(
                p,
                icon,
                b[0] + 2.0,
                b[1] + 2.0,
                is,
                Ink {
                    ink: TEXT,
                    accent: TEXT,
                },
            );
            self.marks9.tools.push((id, b));
        }
        self.px(THUMB_TOP)
    }

    /// Over the panels: the rename box, the drop line, the visible area on the thumbnail.
    pub(crate) fn paint_marks9(&self, p: &mut dyn Painter) {
        let r = self.panel_rect;
        if r[2] <= 0.0 {
            return;
        }
        let panel = self.panel_open();
        if panel == Some(Panel::Bookmarks) {
            let row = |want: &Vec<usize>| {
                self.panel_rows.iter().find_map(|(ry, rh, row)| match row {
                    PanelRow::Outline(p, ..) if p == want => Some((*ry, *rh)),
                    _ => None,
                })
            };
            if let Some((path, text)) = &self.marks9.editing {
                if let Some((ry, rh)) = row(path) {
                    let x0 = r[0] + self.px(8.0) + self.px(22.0) * path.len() as f32;
                    let b = [
                        x0,
                        ry + 1.0,
                        (r[0] + r[2] - self.px(8.0) - x0).max(20.0),
                        rh - 2.0,
                    ];
                    p.push_clip(r[0], r[1], r[2], r[3]);
                    p.fill_rect(b[0], b[1], b[2], b[3], self.theme.accent);
                    p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, super::WHITE);
                    let st = self.style(14.0, false);
                    let ty = b[1] + ((b[3] - st.size * 1.33) / 2.0).round();
                    p.push_clip(b[0] + 1.0, b[1], b[2] - 2.0, b[3]);
                    p.text(text, b[0] + self.px(4.0), ty, st, TEXT);
                    let cx = b[0] + self.px(4.0) + self.width_of(text, st);
                    p.fill_rect(cx.round(), b[1] + 3.0, 1.0, b[3] - 6.0, TEXT);
                    p.pop_clip();
                    p.pop_clip();
                }
            }
            if let (true, Some(to)) = (self.marks9.dragging, &self.marks9.drop) {
                if let Some((ry, _)) = row(to) {
                    p.fill_rect(
                        r[0] + self.px(8.0),
                        ry - 1.0,
                        r[2] - self.px(16.0),
                        2.0,
                        self.theme.accent,
                    );
                }
            }
        }
        if panel == Some(Panel::Pages) {
            self.paint_view_mark9(p, r);
        }
    }

    /// The part of the current page in view, framed on its thumbnail.
    fn paint_view_mark9(&self, p: &mut dyn Painter, r: [f32; 4]) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let v = &t.view;
        let cur = v.current_page();
        let Some(pr) = v.page_rect(cur) else {
            return;
        };
        let Some((ry, _)) = self.panel_rows.iter().find_map(|(ry, rh, row)| match row {
            PanelRow::Page(i) if *i == cur => Some((*ry, *rh)),
            _ => None,
        }) else {
            return;
        };
        let d = self.doc_rect;
        let x0 = pr[0].max(d[0]);
        let y0 = pr[1].max(d[1]);
        let x1 = (pr[0] + pr[2]).min(d[0] + d[2]);
        let y1 = (pr[1] + pr[3]).min(d[1] + d[3]);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let tw = self.px(self.thumb_w9());
        let k = tw / pr[2].max(1.0);
        let tx = (r[0] + (r[2] - tw) / 2.0).round();
        let b = [
            tx + (x0 - pr[0]) * k,
            ry + (y0 - pr[1]) * k,
            (x1 - x0) * k,
            (y1 - y0) * k,
        ];
        let w = self.px(1.6).max(1.0);
        p.push_clip(
            r[0],
            r[1] + self.px(THUMB_TOP),
            r[2],
            r[3] - self.px(THUMB_TOP),
        );
        p.raw_colours(true);
        for e in [
            [b[0], b[1], b[2], w],
            [b[0], b[1] + b[3] - w, b[2], w],
            [b[0], b[1], w, b[3]],
            [b[0] + b[2] - w, b[1], w, b[3]],
        ] {
            p.fill_rect(e[0], e[1], e[2], e[3], VIEW_MARK);
        }
        p.raw_colours(false);
        p.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader_tests::Fixed;
    use crate::{Input, Mods};
    use bo_pdfread::PdfDocument;

    fn it(t: &str, children: Vec<OutlineItem>) -> OutlineItem {
        OutlineItem {
            title: t.into(),
            target: Some(LinkTarget::Page {
                index: 0,
                top: None,
            }),
            open: false,
            children,
        }
    }

    fn titles(items: &[OutlineItem]) -> String {
        items
            .iter()
            .map(|i| {
                if i.children.is_empty() {
                    i.title.clone()
                } else {
                    format!("{}({})", i.title, titles(&i.children))
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    #[test]
    fn tree_moves() {
        let base = vec![
            it("A", vec![it("A1", vec![]), it("A2", vec![])]),
            it("B", vec![]),
            it("C", vec![]),
        ];
        let mut v = base.clone();
        assert_eq!(promote(&mut v, &[0, 1]), Some(vec![1]));
        assert_eq!(titles(&v), "A(A1),A2,B,C");
        let mut v = base.clone();
        assert_eq!(demote(&mut v, &[1]), Some(vec![0, 2]));
        assert_eq!(titles(&v), "A(A1,A2,B),C");
        let mut v = base.clone();
        assert_eq!(move_before(&mut v, &[2], &[0]), Some(vec![0]));
        assert_eq!(titles(&v), "C,A(A1,A2),B");
        let mut v = base.clone();
        assert_eq!(move_before(&mut v, &[0], &[2]), Some(vec![1]));
        assert_eq!(titles(&v), "B,A(A1,A2),C");
        let mut v = base.clone();
        assert_eq!(move_before(&mut v, &[0], &[0, 1]), None);
        assert_eq!(move_before(&mut v, &[1], &[0, 1]), Some(vec![0, 1]));
        assert_eq!(titles(&v), "A(A1,B,A2),C");
    }

    fn window() -> ReaderWindow {
        let doc = PdfDocument::open(crate::reader_tests::pdf(3), bo_pdfread::standard_fonts())
            .expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "test.pdf", None);
        w
    }

    fn key(w: &mut ReaderWindow, k: Key) {
        w.handle(Input::Key(k, Mods::default()), &Fixed);
    }

    #[test]
    fn add_rename_delete_and_save_incrementally() {
        let mut w = window();
        let before = w.view().expect("view").doc().bytes().to_vec();
        w.handle(
            Input::Key(
                Key::Letter('b'),
                Mods {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
            ),
            &Fixed,
        );
        assert_eq!(w.panel_open(), Some(Panel::Bookmarks));
        assert!(w.marks9.editing.is_some());
        // Type a new name over "Untitled".
        for _ in 0..20 {
            key(&mut w, Key::Backspace);
        }
        w.handle(Input::Text("Chapter 1".into()), &Fixed);
        key(&mut w, Key::Enter);
        assert_eq!(titles(&w.outline9()), "Chapter 1");
        w.command_marks9("bm9.add");
        key(&mut w, Key::Escape);
        assert_eq!(titles(&w.outline9()), "Chapter 1,Untitled");
        // Demote the second under the first, then rename it with F2.
        w.command_marks9("bm9.demote");
        assert_eq!(titles(&w.outline9()), "Chapter 1(Untitled)");
        key(&mut w, Key::F(2));
        w.handle(Input::Text(" A".into()), &Fixed);
        key(&mut w, Key::Enter);
        assert_eq!(titles(&w.outline9()), "Chapter 1(Untitled A)");
        w.command_marks9("bm9.promote");
        w.command_marks9("bm9.delete");
        assert_eq!(titles(&w.outline9()), "Chapter 1");
        // Saved as an incremental update: the original bytes stay the file's start.
        let saved = w
            .tabs
            .get_mut(w.active)
            .expect("tab")
            .view
            .doc_mut()
            .save()
            .expect("save");
        assert!(saved.len() > before.len());
        assert_eq!(&saved[..before.len()], &before[..]);
        let again = PdfDocument::open(saved, bo_pdfread::standard_fonts()).expect("reopen");
        assert_eq!(titles(&again.outline()), "Chapter 1");
    }

    #[test]
    fn thumbnails_zoom_with_ctrl_wheel_and_menu() {
        let mut w = window();
        w.set_panel(Some(Panel::Pages));
        let r = w.panel_rect;
        w.handle(
            Input::Move {
                x: r[0] + 40.0,
                y: r[1] + 200.0,
            },
            &Fixed,
        );
        let w0 = w.thumb_w9();
        w.handle(Input::Zoom(1.0), &Fixed);
        assert!(w.thumb_w9() > w0);
        w.handle(Input::Zoom(-1.0), &Fixed);
        w.handle(Input::Zoom(-1.0), &Fixed);
        assert!(w.thumb_w9() < w0);
        w.handle(
            Input::Context {
                x: r[0] + 40.0,
                y: r[1] + 200.0,
            },
            &Fixed,
        );
        let labels = w.menu_labels9();
        assert_eq!(
            labels,
            [
                "Enlarge Page Thumbnails",
                "Reduce Page Thumbnails",
                "Print Pages...",
                "Properties..."
            ]
        );
    }
}
