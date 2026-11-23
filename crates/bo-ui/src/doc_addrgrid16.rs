//! Mailings › Select Recipients › Type a New List… of BlueDoc (round 16): the reference's
//! **New Address List** — an editable grid (column headers with a ▾ for sort / filter, a row
//! selector column, typing into the selected cell, horizontal scrolling), New Entry, Find…,
//! Delete Entry, Customize Columns…, OK / Cancel, in a frame resized from its corner.
//! Measured on the reference at 125 %: 808 × 465, columns 100 px, rows 22 px.
use crate::{Input, Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const FRAME: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const BG: Rgba = Rgba(0xF0, 0xF0, 0xF0, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const GRID: Rgba = Rgba(0xD9, 0xD9, 0xD9, 255);
const SEL_ROW: Rgba = Rgba(0x99, 0xC9, 0xEF, 255);
const SEL_CELL: Rgba = Rgba(0xE5, 0xF1, 0xFB, 255);
const BUTTON_EDGE: Rgba = Rgba(0xAD, 0xAD, 0xAD, 255);

/// What the dialog asks the window to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GridOut {
    None,
    Redraw,
    Cancel,
    /// OK: the columns and the rows typed (empty rows left out).
    Ok(Vec<String>, Vec<Vec<String>>),
    /// A column's ▾ menu at the rectangle (sort / filter).
    ColumnMenu(usize, [f32; 4]),
    Find,
    Customize,
}

/// The New Address List dialog.
#[derive(Debug, Clone)]
pub(crate) struct AddressGrid {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    /// Selected cell (row, column).
    pub(crate) cur: (usize, usize),
    /// Rows hidden by a column filter: (column, value).
    pub(crate) filter: Option<(usize, String)>,
    /// Columns scrolled off the left.
    pub(crate) hscroll: usize,
    rect: [f32; 4],
    scale: f32,
    /// Size in reference px (the corner grip changes it).
    pub(crate) size: (f32, f32),
    resizing: Option<(f32, f32, (f32, f32))>,
    labels: [String; 8],
    /// Maximized (the caption's maximize box), with the size to come back to.
    maximized: Option<(f32, f32)>,
    window: (f32, f32),
}

impl AddressGrid {
    /// Labels: title, intro, New Entry, Find…, Delete Entry, Customize Columns…, OK, Cancel.
    pub(crate) fn new(columns: Vec<String>, labels: [String; 8]) -> Self {
        let n = columns.len();
        Self {
            columns,
            rows: vec![vec![String::new(); n]],
            cur: (0, 0),
            filter: None,
            hscroll: 0,
            rect: [0.0; 4],
            scale: 1.25,
            size: (808.0, 465.0),
            resizing: None,
            labels,
            maximized: None,
            window: (1500.0, 1100.0),
        }
    }

    fn k(&self) -> f32 {
        self.scale / 1.25
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.k()).round()
    }

    pub(crate) fn layout(&mut self, window: (f32, f32), scale: f32) {
        self.scale = scale;
        self.window = window;
        let (w, h) = (self.px(self.size.0), self.px(self.size.1));
        self.rect = [
            ((window.0 - w) / 2.0).round().max(0.0),
            ((window.1 - h) / 2.0).round().max(0.0),
            w,
            h,
        ];
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    fn grid_rect(&self) -> [f32; 4] {
        let [x, y, w, h] = self.rect;
        [
            x + self.px(10.0),
            y + self.px(66.0),
            w - self.px(20.0),
            h - self.px(66.0) - self.px(92.0),
        ]
    }

    /// Visible rows (filter applied): indices into `rows`.
    pub(crate) fn visible(&self) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| {
                self.filter.as_ref().is_none_or(|(c, v)| {
                    self.rows[i].get(*c).map(String::as_str).unwrap_or_default() == v
                })
            })
            .collect()
    }

    fn col_w(&self) -> f32 {
        self.px(100.0)
    }

    fn row_h(&self) -> f32 {
        self.px(22.0)
    }

    fn sel_w(&self) -> f32 {
        self.px(22.0)
    }

    /// Columns that fit in the grid from `hscroll`.
    fn fit_cols(&self) -> usize {
        let g = self.grid_rect();
        (((g[2] - self.sel_w()) / self.col_w()).floor() as usize).max(1)
    }

    fn buttons(&self) -> [([f32; 4], usize); 6] {
        let [x, y, w, h] = self.rect;
        let (bw, bh) = (self.px(112.0), self.px(28.0));
        let r1 = y + h - self.px(84.0);
        let r2 = y + h - self.px(48.0);
        [
            ([x + self.px(10.0), r1, bw, bh], 2),
            ([x + self.px(136.0), r1, bw + self.px(36.0), bh], 3),
            ([x + self.px(10.0), r2, bw, bh], 4),
            ([x + self.px(136.0), r2, bw + self.px(36.0), bh], 5),
            ([x + w - self.px(250.0), r2, bw, bh], 6),
            ([x + w - self.px(124.0), r2, bw, bh], 7),
        ]
    }

    fn grip(&self) -> [f32; 4] {
        let [x, y, w, h] = self.rect;
        let s = self.px(16.0);
        [x + w - s, y + h - s, s, s]
    }

    /// Add an entry after the last and select its first cell.
    pub(crate) fn new_entry(&mut self) {
        self.rows.push(vec![String::new(); self.columns.len()]);
        self.filter = None;
        self.cur = (self.rows.len() - 1, 0);
        self.hscroll = 0;
    }

    pub(crate) fn delete_entry(&mut self) {
        if self.cur.0 < self.rows.len() {
            self.rows.remove(self.cur.0);
        }
        if self.rows.is_empty() {
            self.rows.push(vec![String::new(); self.columns.len()]);
        }
        self.cur.0 = self.cur.0.min(self.rows.len() - 1);
    }

    /// Sort the rows by column `c`.
    pub(crate) fn sort(&mut self, c: usize, ascending: bool) {
        let key = |r: &Vec<String>| r.get(c).cloned().unwrap_or_default().to_lowercase();
        self.rows.sort_by(|a, b| {
            let o = key(a).cmp(&key(b));
            if ascending {
                o
            } else {
                o.reverse()
            }
        });
    }

    /// Distinct values of column `c` (for its filter).
    pub(crate) fn values(&self, c: usize) -> Vec<String> {
        let mut v: Vec<String> = self
            .rows
            .iter()
            .filter_map(|r| r.get(c).cloned())
            .filter(|s| !s.is_empty())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Find…: the next cell after the selected one containing `what` (all columns, or the
    /// selected column only). `true` when found.
    pub(crate) fn find(&mut self, what: &str, this_column: bool) -> bool {
        let w = what.to_lowercase();
        if w.is_empty() {
            return false;
        }
        let n = self.rows.len();
        let cols = self.columns.len();
        for step in 1..=n * cols {
            let at = (self.cur.0 * cols + self.cur.1 + step) % (n * cols);
            let (r, c) = (at / cols, at % cols);
            if this_column && c != self.cur.1 {
                continue;
            }
            if self.rows[r]
                .get(c)
                .is_some_and(|v| v.to_lowercase().contains(&w))
            {
                self.cur = (r, c);
                self.filter = None;
                self.scroll_to(c);
                return true;
            }
        }
        false
    }

    fn scroll_to(&mut self, c: usize) {
        let fit = self.fit_cols();
        if c < self.hscroll {
            self.hscroll = c;
        } else if c >= self.hscroll + fit {
            self.hscroll = c + 1 - fit;
        }
    }

    /// Customize Columns: the new columns (`Some(i)`: was column i; `None`: added) with their
    /// names; the rows follow.
    pub(crate) fn set_columns(&mut self, cols: &[(Option<usize>, String)]) {
        self.rows = self
            .rows
            .iter()
            .map(|r| {
                cols.iter()
                    .map(|(o, _)| o.and_then(|i| r.get(i).cloned()).unwrap_or_default())
                    .collect()
            })
            .collect();
        self.columns = cols.iter().map(|(_, n)| n.clone()).collect();
        self.cur = (
            self.cur.0,
            self.cur.1.min(self.columns.len().saturating_sub(1)),
        );
        self.hscroll = 0;
        self.filter = None;
    }

    fn ok(&self) -> GridOut {
        let rows = self
            .rows
            .iter()
            .filter(|r| r.iter().any(|c| !c.trim().is_empty()))
            .cloned()
            .collect();
        GridOut::Ok(self.columns.clone(), rows)
    }

    pub(crate) fn input(&mut self, input: &Input) -> GridOut {
        match input {
            Input::Text(t) => {
                if let Some(cell) = self
                    .rows
                    .get_mut(self.cur.0)
                    .and_then(|r| r.get_mut(self.cur.1))
                {
                    cell.push_str(t);
                }
                GridOut::Redraw
            }
            Input::Key(k, m) => self.key(*k, m.shift),
            Input::Press { x, y, .. } => self.press(*x, *y),
            Input::Drag { x, y } | Input::Move { x, y } => {
                if let Some((sx, sy, (w0, h0))) = self.resizing {
                    let k = self.k();
                    self.size = (
                        (w0 + (x - sx) / k).clamp(560.0, 1400.0),
                        (h0 + (y - sy) / k).clamp(360.0, 1000.0),
                    );
                    return GridOut::Redraw;
                }
                GridOut::None
            }
            Input::Release { .. } => {
                self.resizing = None;
                GridOut::None
            }
            Input::Wheel(d) => {
                let max = self.columns.len().saturating_sub(self.fit_cols());
                self.hscroll = if *d < 0.0 {
                    (self.hscroll + 1).min(max)
                } else {
                    self.hscroll.saturating_sub(1)
                };
                GridOut::Redraw
            }
            _ => GridOut::None,
        }
    }

    fn key(&mut self, k: Key, shift: bool) -> GridOut {
        let cols = self.columns.len().max(1);
        let vis = self.visible();
        let vpos = vis.iter().position(|&r| r == self.cur.0).unwrap_or(0);
        match k {
            Key::Escape => return GridOut::Cancel,
            Key::Backspace => {
                if let Some(cell) = self
                    .rows
                    .get_mut(self.cur.0)
                    .and_then(|r| r.get_mut(self.cur.1))
                {
                    cell.pop();
                }
            }
            Key::Tab | Key::Right if !shift => {
                if self.cur.1 + 1 < cols {
                    self.cur.1 += 1;
                } else if vpos + 1 < vis.len() {
                    self.cur = (vis[vpos + 1], 0);
                }
            }
            Key::Tab | Key::Left => {
                if self.cur.1 > 0 {
                    self.cur.1 -= 1;
                } else if vpos > 0 {
                    self.cur = (vis[vpos - 1], cols - 1);
                }
            }
            Key::Down | Key::Enter => {
                if vpos + 1 < vis.len() {
                    self.cur.0 = vis[vpos + 1];
                } else if k == Key::Enter {
                    return self.ok();
                }
            }
            Key::Up if vpos > 0 => self.cur.0 = vis[vpos - 1],
            _ => return GridOut::None,
        }
        let c = self.cur.1;
        self.scroll_to(c);
        GridOut::Redraw
    }

    fn press(&mut self, x: f32, y: f32) -> GridOut {
        if !self.contains(x, y) {
            return GridOut::None;
        }
        if inside(self.grip(), x, y) {
            self.resizing = Some((x, y, self.size));
            return GridOut::None;
        }
        let [rx, ry, rw, _] = self.rect;
        // Title bar close cross.
        if inside(
            [rx + rw - self.px(46.0), ry, self.px(46.0), self.px(36.0)],
            x,
            y,
        ) {
            return GridOut::Cancel;
        }
        // The maximize box left of it.
        if inside(
            [rx + rw - self.px(92.0), ry, self.px(46.0), self.px(36.0)],
            x,
            y,
        ) {
            if let Some(back) = self.maximized.take() {
                self.size = back;
            } else {
                self.maximized = Some(self.size);
                let k = self.k();
                self.size = (self.window.0 / k, self.window.1 / k);
            }
            return GridOut::Redraw;
        }
        for (r, i) in self.buttons() {
            if inside(r, x, y) {
                return match i {
                    2 => {
                        self.new_entry();
                        GridOut::Redraw
                    }
                    3 => GridOut::Find,
                    4 => {
                        self.delete_entry();
                        GridOut::Redraw
                    }
                    5 => GridOut::Customize,
                    6 => self.ok(),
                    _ => GridOut::Cancel,
                };
            }
        }
        let g = self.grid_rect();
        if !inside(g, x, y) {
            return GridOut::None;
        }
        // The horizontal scroll bar at the grid's foot.
        let bar = self.px(16.0);
        if y >= g[1] + g[3] - bar {
            let max = self.columns.len().saturating_sub(self.fit_cols());
            self.hscroll = if x < g[0] + g[2] / 2.0 {
                self.hscroll.saturating_sub(1)
            } else {
                (self.hscroll + 1).min(max)
            };
            return GridOut::Redraw;
        }
        let col = ((x - g[0] - self.sel_w()) / self.col_w()).floor();
        let rowi = ((y - g[1]) / self.row_h()).floor();
        if col < 0.0 {
            // Row selector: select the row's first cell.
            let vis = self.visible();
            if rowi >= 1.0 {
                if let Some(&r) = vis.get(rowi as usize - 1) {
                    self.cur = (r, 0);
                }
            }
            return GridOut::Redraw;
        }
        let c = self.hscroll + col as usize;
        if c >= self.columns.len() {
            return GridOut::None;
        }
        if rowi < 1.0 {
            // Header: the ▾ at its right opens the sort / filter menu.
            let hx = g[0] + self.sel_w() + col * self.col_w();
            return GridOut::ColumnMenu(c, [hx, g[1], self.col_w(), self.row_h()]);
        }
        let vis = self.visible();
        if let Some(&r) = vis.get(rowi as usize - 1) {
            self.cur = (r, c);
        }
        GridOut::Redraw
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        p.fill_rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, Rgba(0, 0, 0, 40));
        p.fill_rect(x, y, w, h, FRAME);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, self.px(36.0), WHITE);
        let st = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let title = TextStyle {
            size: self.px(15.0),
            bold: false,
        };
        let lh = st.size * 1.33;
        p.text(
            &self.labels[0],
            x + self.px(12.0),
            y + self.px(8.0),
            title,
            INK,
        );
        let (cx, cy, k) = (x + w - self.px(23.0), y + self.px(18.0), self.px(6.0));
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((INK, self.scale.max(1.0))),
        );
        p.text(
            &self.labels[1],
            x + self.px(12.0),
            y + self.px(44.0),
            st,
            INK,
        );
        // Maximize box (a square; two when maximized).
        let (mx, my, m) = (x + w - self.px(69.0), y + self.px(18.0), self.px(5.0));
        let sq = |p: &mut dyn Painter, ox: f32, oy: f32| {
            p.path(
                &[
                    PathSeg::M(ox - m, oy - m),
                    PathSeg::L(ox + m, oy - m),
                    PathSeg::L(ox + m, oy + m),
                    PathSeg::L(ox - m, oy + m),
                    PathSeg::Z,
                ],
                Some(WHITE),
                Some((INK, 1.0)),
            );
        };
        if self.maximized.is_some() {
            sq(p, mx + self.px(2.0), my - self.px(2.0));
        }
        sq(p, mx, my);
        // The grid.
        let g = self.grid_rect();
        p.fill_rect(g[0], g[1], g[2], g[3], FRAME);
        p.fill_rect(g[0] + 1.0, g[1] + 1.0, g[2] - 2.0, g[3] - 2.0, WHITE);
        p.push_clip(g[0] + 1.0, g[1] + 1.0, g[2] - 2.0, g[3] - 2.0);
        let (cw, rh, sw) = (self.col_w(), self.row_h(), self.sel_w());
        // Header row.
        p.fill_rect(g[0], g[1], g[2], rh, Rgba(0xF5, 0xF5, 0xF5, 255));
        let fit = self.fit_cols() + 1;
        for (j, name) in self.columns.iter().enumerate().skip(self.hscroll).take(fit) {
            let hx = g[0] + sw + (j - self.hscroll) as f32 * cw;
            p.fill_rect(hx + cw - 1.0, g[1], 1.0, g[3], GRID);
            p.push_clip(hx, g[1], cw - self.px(16.0), rh);
            p.text(name, hx + self.px(4.0), g[1] + (rh - lh) / 2.0, st, INK);
            p.pop_clip();
            let (ax, ay, a) = (hx + cw - self.px(9.0), g[1] + rh / 2.0, self.px(3.5));
            p.path(
                &[
                    PathSeg::M(ax - a, ay - a / 2.0),
                    PathSeg::L(ax + a, ay - a / 2.0),
                    PathSeg::L(ax, ay + a / 2.0),
                    PathSeg::Z,
                ],
                Some(INK),
                None,
            );
        }
        p.fill_rect(g[0], g[1] + rh - 1.0, g[2], 1.0, GRID);
        p.fill_rect(g[0] + sw - 1.0, g[1], 1.0, g[3], GRID);
        for (vi, &r) in self.visible().iter().enumerate() {
            let ry = g[1] + rh * (vi + 1) as f32;
            if ry > g[1] + g[3] {
                break;
            }
            let selected = r == self.cur.0;
            if selected {
                p.fill_rect(g[0] + sw, ry, g[2] - sw, rh, SEL_ROW);
                // The row selector's mark.
                let (mx, my, a) = (g[0] + sw / 2.0, ry + rh / 2.0, self.px(4.0));
                p.path(
                    &[
                        PathSeg::M(mx - a / 2.0, my - a),
                        PathSeg::L(mx + a / 2.0, my),
                        PathSeg::L(mx - a / 2.0, my + a),
                        PathSeg::Z,
                    ],
                    None,
                    Some((Rgba(0x80, 0x80, 0x80, 255), 1.0)),
                );
            }
            for (j, v) in self.rows[r].iter().enumerate().skip(self.hscroll).take(fit) {
                let cx = g[0] + sw + (j - self.hscroll) as f32 * cw;
                if selected && j == self.cur.1 {
                    p.fill_rect(cx, ry, cw - 1.0, rh - 1.0, SEL_CELL);
                }
                p.push_clip(cx, ry, cw - 2.0, rh);
                p.text(v, cx + self.px(4.0), ry + (rh - lh) / 2.0, st, INK);
                if selected && j == self.cur.1 {
                    let tw = v.chars().count() as f32 * st.size * 0.52;
                    p.fill_rect(
                        cx + self.px(4.0) + tw,
                        ry + self.px(4.0),
                        1.0,
                        rh - self.px(8.0),
                        INK,
                    );
                }
                p.pop_clip();
            }
            p.fill_rect(g[0], ry + rh - 1.0, g[2], 1.0, GRID);
        }
        // Horizontal scroll bar.
        let bar = self.px(16.0);
        let by = g[1] + g[3] - bar;
        p.fill_rect(
            g[0] + 1.0,
            by,
            g[2] - 2.0,
            bar - 1.0,
            Rgba(0xF0, 0xF0, 0xF0, 255),
        );
        let total = self.columns.len().max(1) as f32;
        let tw =
            ((g[2] - 2.0 * bar) * (self.fit_cols() as f32 / total).min(1.0)).max(self.px(20.0));
        let tx = g[0]
            + bar
            + (g[2] - 2.0 * bar - tw)
                * (self.hscroll as f32 / (total - self.fit_cols() as f32).max(1.0));
        p.fill_rect(
            tx,
            by + self.px(4.0),
            tw,
            bar - self.px(8.0),
            Rgba(0xA0, 0xA0, 0xA0, 255),
        );
        p.pop_clip();
        // Buttons.
        for (r, i) in self.buttons() {
            p.fill_rect(
                r[0],
                r[1],
                r[2],
                r[3],
                if i == 6 {
                    Rgba(0x00, 0x5F, 0xB8, 255)
                } else {
                    BUTTON_EDGE
                },
            );
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
            let l = &self.labels[i];
            let tw = l.chars().count() as f32 * st.size * 0.5;
            p.text(
                l,
                r[0] + ((r[2] - tw) / 2.0).max(4.0),
                r[1] + (r[3] - lh) / 2.0,
                st,
                INK,
            );
        }
        // The corner grip.
        let gr = self.grip();
        for i in 0..3 {
            let d = self.px(4.0) * i as f32;
            p.fill_rect(
                gr[0] + gr[2] - self.px(4.0) - d,
                gr[1] + gr[3] - self.px(4.0),
                2.0,
                2.0,
                FRAME,
            );
            p.fill_rect(
                gr[0] + gr[2] - self.px(4.0),
                gr[1] + gr[3] - self.px(4.0) - d,
                2.0,
                2.0,
                FRAME,
            );
        }
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> AddressGrid {
        let labels = std::array::from_fn(|i| format!("l{i}"));
        let mut g = AddressGrid::new(vec!["First".into(), "City".into()], labels);
        g.layout((1500.0, 1100.0), 1.25);
        g
    }

    #[test]
    fn typing_entries_sort_filter_find_columns() {
        let mut g = grid();
        g.input(&Input::Text("Bob".into()));
        g.input(&Input::Key(Key::Tab, crate::Mods::default()));
        g.input(&Input::Text("Hue".into()));
        g.new_entry();
        g.input(&Input::Text("Ann".into()));
        g.input(&Input::Key(Key::Tab, crate::Mods::default()));
        g.input(&Input::Text("Hanoi".into()));
        g.sort(0, true);
        assert_eq!(g.rows[0][0], "Ann");
        g.filter = Some((1, "Hue".into()));
        assert_eq!(g.visible(), [1]);
        g.filter = None;
        g.cur = (0, 0);
        assert!(g.find("hue", false));
        assert_eq!(g.cur, (1, 1));
        g.set_columns(&[(Some(1), "Town".into()), (None, "Email".into())]);
        assert_eq!(g.columns, ["Town", "Email"]);
        assert_eq!(g.rows[1], ["Hue", ""]);
        let GridOut::Ok(cols, rows) = g.input(&Input::Key(Key::Enter, crate::Mods::default()))
        else {
            // Enter on a row that is not the last moves down; once more on the last one.
            let out = g.input(&Input::Key(Key::Enter, crate::Mods::default()));
            assert!(matches!(out, GridOut::Ok(..)), "{out:?}");
            return;
        };
        assert_eq!(cols.len(), 2);
        assert_eq!(rows.len(), 2);
    }
}
