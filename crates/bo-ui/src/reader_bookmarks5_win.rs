//! BlueReader round 5 (a child of `reader_window`): the Bookmarks panel like the reference's —
//! a tool row (expand / collapse all, find the current bookmark, larger / smaller text), a
//! search box that filters the bookmarks, dotted tree connectors, the bookmark of the current
//! page marked in the accent colour, and the measured row metrics (125 %: rows 24 px apart,
//! 22 px indent per level, first row centre 134 px under the panel top).
use super::{
    flatten_outline, inside, Ink, LinkTarget, Panel, PanelRow, ReaderWindow, MUTED, TEXT, WHITE,
};
use crate::popup::{Choice, Menu, MenuItem};
use crate::reader_icons;
use crate::{Input, Key, Painter, TextMeasure};
use bo_pdfread::OutlineItem;
use bo_render::Rgba;
use std::collections::HashSet;

/// Grey of the dotted tree connectors.
const DOTS: Rgba = Rgba(160, 160, 160, 255);

type Row<'o> = (Vec<usize>, &'o OutlineItem, usize, bool, bool);

/// Every bookmark whose title contains `q` (lower case), as a flat list.
fn search_rows<'o>(
    items: &'o [OutlineItem],
    q: &str,
    path: &mut Vec<usize>,
    out: &mut Vec<Row<'o>>,
) {
    for (k, it) in items.iter().enumerate() {
        path.push(k);
        if it.title.to_lowercase().contains(q) {
            out.push((path.clone(), it, 0, false, false));
        }
        if path.len() < 16 {
            search_rows(&it.children, q, path, out);
        }
        path.pop();
    }
}

fn page_of(it: &OutlineItem) -> Option<usize> {
    match it.target {
        Some(LinkTarget::Page { index, .. }) => Some(index),
        _ => None,
    }
}

/// The bookmark of page `cur`: the first one on the latest page not after `cur`.
pub(crate) fn current_path(items: &[OutlineItem], cur: usize) -> Option<Vec<usize>> {
    fn walk(
        items: &[OutlineItem],
        cur: usize,
        path: &mut Vec<usize>,
        best: &mut Option<(usize, Vec<usize>)>,
    ) {
        for (k, it) in items.iter().enumerate() {
            path.push(k);
            if let Some(pg) = page_of(it) {
                if pg <= cur && best.as_ref().is_none_or(|(b, _)| pg > *b) {
                    *best = Some((pg, path.clone()));
                }
            }
            if path.len() < 16 {
                walk(&it.children, cur, path, best);
            }
            path.pop();
        }
    }
    let mut best = None;
    walk(items, cur, &mut Vec::new(), &mut best);
    best.map(|b| b.1)
}

/// Paths of every bookmark with children.
fn all_parents(items: &[OutlineItem], path: &mut Vec<usize>, out: &mut HashSet<Vec<usize>>) {
    for (k, it) in items.iter().enumerate() {
        path.push(k);
        if !it.children.is_empty() && path.len() < 16 {
            out.insert(path.clone());
            all_parents(&it.children, path, out);
        }
        path.pop();
    }
}

fn vdots(p: &mut dyn Painter, x: f32, y0: f32, y1: f32) {
    let mut y = (y0 / 2.0).ceil() * 2.0;
    while y < y1 {
        p.fill_rect(x, y, 1.0, 1.0, DOTS);
        y += 2.0;
    }
}

fn hdots(p: &mut dyn Painter, x0: f32, x1: f32, y: f32) {
    let mut x = (x0 / 2.0).ceil() * 2.0;
    while x < x1 {
        p.fill_rect(x, y, 1.0, 1.0, DOTS);
        x += 2.0;
    }
}

impl ReaderWindow {
    /// The tool row and the search box; returns where the list starts below the panel top.
    pub(crate) fn paint_bookmark_tools(&mut self, p: &mut dyn Painter, r: [f32; 4]) -> f32 {
        let is = self.px(17.6);
        let cy = r[1] + self.px(41.6);
        self.tabs5.bm_tools.clear();
        for (k, (id, icon)) in [
            ("bm.options", "bmOptions"),
            ("bm.current", "bmFind"),
            ("bm.larger", "textLarger"),
            ("bm.smaller", "textSmaller"),
        ]
        .into_iter()
        .enumerate()
        {
            let cx = r[0] + self.px(15.2 + 24.8 * k as f32);
            let b = [
                (cx - is / 2.0 - 2.0).round(),
                (cy - is / 2.0 - 2.0).round(),
                is + 4.0,
                is + 4.0,
            ];
            if self.tabs5.bm_hover == Some(id) {
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
            self.tabs5.bm_tools.push((id, b));
        }
        // Search box (125 %: 280 × 25 px, 73 px under the panel top).
        let sb = [
            r[0] + self.px(8.0),
            r[1] + self.px(58.4),
            r[2] - self.px(16.0),
            self.px(20.0),
        ];
        let edge = if self.tabs5.bm_focus {
            self.theme.accent
        } else {
            self.look.box_edge
        };
        p.fill_rect(sb[0], sb[1], sb[2], sb[3], edge);
        p.fill_rect(sb[0] + 1.0, sb[1] + 1.0, sb[2] - 2.0, sb[3] - 2.0, WHITE);
        let ms = self.px(13.6);
        reader_icons::draw(
            p,
            "search",
            sb[0] + self.px(4.0),
            sb[1] + ((sb[3] - ms) / 2.0).round(),
            ms,
            Ink {
                ink: TEXT,
                accent: TEXT,
            },
        );
        let st = self.style(13.0, false);
        let tx = sb[0] + self.px(21.6);
        let ty = sb[1] + ((sb[3] - st.size * 1.33) / 2.0).round();
        p.push_clip(sb[0], sb[1], sb[2], sb[3]);
        if self.tabs5.bm_search.is_empty() {
            p.text(&self.t("pdfBookmark.search"), tx, ty, st, MUTED);
        } else {
            let q = self.tabs5.bm_search.clone();
            p.text(&q, tx, ty, st, TEXT);
        }
        if self.tabs5.bm_focus {
            let q = self.tabs5.bm_search.clone();
            let w = self.width_of(&q, st);
            p.fill_rect((tx + w).round(), sb[1] + 3.0, 1.0, sb[3] - 6.0, TEXT);
        }
        p.pop_clip();
        self.tabs5.bm_search_box = sb;
        self.px(93.6)
    }

    /// The bookmark rows.
    pub(crate) fn paint_bookmarks(
        &mut self,
        p: &mut dyn Painter,
        r: [f32; 4],
        list_top: f32,
        y0: f32,
    ) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let outline = t.outline.clone();
        let expanded = t.expanded.clone();
        let cur = t.view.current_page();
        let q = self.tabs5.bm_search.to_lowercase();
        let mut rows = Vec::new();
        if q.is_empty() {
            flatten_outline(&outline, &expanded, &mut Vec::new(), 0, &mut rows);
        } else {
            search_rows(&outline, &q, &mut Vec::new(), &mut rows);
        }
        let body = self.style(15.5 + self.tabs5.bm_font, false);
        let row_h = self.px(19.2 + self.tabs5.bm_font);
        let current = current_path(&outline, cur);
        let mut y = y0 - self.px(4.0) + self.px(3.6);
        if rows.is_empty() {
            let key = if q.is_empty() {
                "pdfReader.noBookmarks"
            } else {
                "pdfReader.noMatches"
            };
            p.text(&self.t(key), r[0] + self.px(8.8), y, body, MUTED);
        }
        let depths: Vec<usize> = rows.iter().map(|row| row.2).collect();
        let mut cont = [false; 18];
        let (b, step) = (self.px(7.2), self.px(17.6));
        let mut out = Vec::new();
        for (i, (path, item, depth, has_kids, open)) in rows.into_iter().enumerate() {
            if y > r[1] + r[3] {
                break;
            }
            let is_last = depths[i + 1..]
                .iter()
                .find(|d| **d <= depth)
                .is_none_or(|d| *d < depth);
            cont[depth.min(17)] = !is_last;
            let x = r[0] + self.px(8.0) + depth as f32 * step;
            let ix = x + self.px(12.8);
            let mid = (y + row_h / 2.0).round();
            if y + row_h > list_top {
                if q.is_empty() {
                    #[allow(clippy::needless_range_loop)]
                    for l in 1..=depth.min(17) {
                        let lx = (r[0] + self.px(8.0) + (l - 1) as f32 * step + (b / 2.0).floor())
                            .round();
                        if l == depth {
                            vdots(p, lx, y, if is_last { mid } else { y + row_h });
                            hdots(p, lx, ix - 1.0, mid);
                        } else if cont[l] {
                            vdots(p, lx, y, y + row_h);
                        }
                    }
                    if has_kids {
                        hdots(p, x + b + 2.0, ix - 1.0, mid);
                        let by = (y + (row_h - b) / 2.0).round();
                        p.fill_rect(x, by, b + 1.0, b + 1.0, Rgba(140, 140, 140, 255));
                        p.fill_rect(x + 1.0, by + 1.0, b - 1.0, b - 1.0, WHITE);
                        p.fill_rect(x + 2.0, by + b / 2.0, b - 3.0, 1.0, TEXT);
                        if !open {
                            p.fill_rect(x + b / 2.0, by + 2.0, 1.0, b - 3.0, TEXT);
                        }
                    }
                }
                let mark = if current.as_ref() == Some(&path) {
                    self.theme.accent
                } else {
                    TEXT
                };
                let is = self.px(14.4);
                reader_icons::draw(
                    p,
                    "bookmark",
                    ix,
                    y + ((row_h - is) / 2.0).round(),
                    is,
                    Ink {
                        ink: mark,
                        accent: mark,
                    },
                );
                p.text(
                    &item.title,
                    ix + self.px(17.6),
                    y + ((row_h - body.size * 1.33) / 2.0).round(),
                    body,
                    TEXT,
                );
            }
            let toggle = (has_kids && q.is_empty()).then_some((x, x + b));
            out.push((
                y,
                row_h,
                PanelRow::Outline(path, item.target.clone(), toggle),
            ));
            y += row_h;
        }
        self.panel_rows = out;
    }

    /// Bookmark panel commands; `true` when `cmd` was one.
    pub(crate) fn bookmark_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "bm.options" => {
                let items = [
                    ("bm.expandAll", "pdfBookmark.expandAll"),
                    ("bm.collapseAll", "pdfBookmark.collapseAll"),
                ]
                .iter()
                .map(|(c, k)| MenuItem {
                    label: self.t(k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: false,
                    dim: false,
                })
                .collect();
                let r = self.tabs5.bm_tools.first().map_or([0.0; 4], |t| t.1);
                self.menu = Some((Menu::open(items, r, self.size, self.scale), "bookmarks"));
            }
            "bm.expandAll" | "bm.collapseAll" => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    t.expanded.clear();
                    if cmd == "bm.expandAll" {
                        let outline = t.outline.clone();
                        all_parents(&outline, &mut Vec::new(), &mut t.expanded);
                    }
                    t.panel_scroll = 0.0;
                }
            }
            "bm.current" => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    if let Some(path) = current_path(&t.outline, t.view.current_page()) {
                        for n in 1..path.len() {
                            t.expanded.insert(path[..n].to_vec());
                        }
                    }
                }
            }
            "bm.larger" => self.tabs5.bm_font = (self.tabs5.bm_font + 1.0).min(8.0),
            "bm.smaller" => self.tabs5.bm_font = (self.tabs5.bm_font - 1.0).max(-3.0),
            _ => return false,
        }
        true
    }

    /// Input of the Bookmarks panel's tools and search box.
    pub(crate) fn route_bookmarks(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if self.panel_open() != Some(Panel::Bookmarks) {
            self.tabs5.bm_focus = false;
            return None;
        }
        match input {
            Input::Move { x, y } => {
                let h = self
                    .tabs5
                    .bm_tools
                    .iter()
                    .find(|t| inside(t.1, *x, *y))
                    .map(|t| t.0);
                if h != self.tabs5.bm_hover {
                    self.tabs5.bm_hover = h;
                    // Let the window update its own hover too.
                    return None;
                }
                None
            }
            Input::Press { x, y, .. } => {
                if let Some(id) = self
                    .tabs5
                    .bm_tools
                    .iter()
                    .find(|t| inside(t.1, *x, *y))
                    .map(|t| t.0)
                {
                    self.bookmark_command(id);
                    return Some(true);
                }
                let on = inside(self.tabs5.bm_search_box, *x, *y);
                if on {
                    self.tabs5.bm_focus = true;
                    self.search_focus = false;
                    return Some(true);
                }
                self.tabs5.bm_focus = false;
                None
            }
            Input::Text(s) if self.tabs5.bm_focus => {
                self.tabs5.bm_search.push_str(s);
                let (q, st) = (self.tabs5.bm_search.clone(), self.style(13.0, false));
                self.measure(m, &q, st);
                if let Some(t) = self.tabs.get_mut(self.active) {
                    t.panel_scroll = 0.0;
                }
                Some(true)
            }
            Input::Key(Key::Backspace, _) if self.tabs5.bm_focus => {
                self.tabs5.bm_search.pop();
                Some(true)
            }
            Input::Key(Key::Escape, _) if self.tabs5.bm_focus => {
                self.tabs5.bm_focus = false;
                self.tabs5.bm_search.clear();
                Some(true)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, page: usize, children: Vec<OutlineItem>) -> OutlineItem {
        OutlineItem {
            title: title.into(),
            target: Some(LinkTarget::Page {
                index: page,
                top: None,
            }),
            open: false,
            children,
        }
    }

    #[test]
    fn current_bookmark_search_and_expand_all() {
        let o = vec![
            item(
                "Part A",
                0,
                vec![item("Chapter 1", 0, vec![]), item("Chapter 2", 1, vec![])],
            ),
            item("Part B", 2, vec![item("Chapter 3", 2, vec![])]),
        ];
        // The reference marks the first bookmark of the page (Part A, not Chapter 1).
        assert_eq!(current_path(&o, 0), Some(vec![0]));
        assert_eq!(current_path(&o, 1), Some(vec![0, 1]));
        assert_eq!(current_path(&o, 5), Some(vec![1]));
        let mut rows = Vec::new();
        search_rows(&o, "chapter", &mut Vec::new(), &mut rows);
        assert_eq!(rows.len(), 3);
        let mut all = HashSet::new();
        all_parents(&o, &mut Vec::new(), &mut all);
        assert_eq!(all.len(), 2);
    }
}
