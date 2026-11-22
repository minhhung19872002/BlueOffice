//! Insert › Table › Quick Tables of BlueDoc (parity round 12, docs/ui/REPORT.md "Vòng 102"):
//! the built-in table designs of BlueOffice's building block library (gallery `tbls`,
//! ECMA-376 Part 1, §17.12.5 docPart) in a scrolling gallery laid out after the reference's
//! (a "Built-In" band, each entry's name over a picture of the table, a footer command). The
//! pictures are drawn from the tables themselves (cells, fills, lines, the first text of each
//! cell); the contents are BlueOffice's own.
use crate::{Painter, TextStyle};
use bo_model::{Block, Table};
use bo_render::{PathSeg, Rgba};

/// Reference sizes at 125 % (device pixels there): panel, band, entry name, picture, footer.
const W: f32 = 621.0;
const H: f32 = 676.0;
const BAND: f32 = 38.0;
const NAME: f32 = 30.0;
const PICTURE: f32 = 170.0;
const FOOT: f32 = 38.0;
const BAR: f32 = 20.0;

const EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const BAND_BG: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const NAME_INK: Rgba = Rgba(0x59, 0x59, 0x59, 255);
const HOT: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const DIM: Rgba = Rgba(0xa0, 0xa0, 0xa0, 255);

/// One design: building block key (`builtin:N`), name and its table.
pub(crate) struct QuickEntry {
    pub(crate) key: String,
    pub(crate) name: String,
    /// The text above the table (a calendar's month), if any.
    pub(crate) title: Option<String>,
    pub(crate) table: Table,
}

/// The built-in Quick Tables of the library, in gallery order.
pub(crate) fn entries() -> Vec<QuickEntry> {
    bo_engine::blocks::library::parts()
        .into_iter()
        .enumerate()
        .filter(|(_, p)| p.props.gallery == "tbls" && !p.props.is_placeholder())
        .filter_map(|(i, p)| {
            let title = p.blocks.iter().find_map(|b| match b {
                Block::Paragraph(q) => Some(q.plain_text()).filter(|t| !t.trim().is_empty()),
                _ => None,
            });
            let table = p.blocks.into_iter().find_map(|b| match b {
                Block::Table(t) => Some(t),
                _ => None,
            })?;
            Some(QuickEntry {
                key: format!("builtin:{i}"),
                name: p.props.name,
                title,
                table,
            })
        })
        .collect()
}

/// The open gallery.
pub(crate) struct QuickTables {
    x: f32,
    y: f32,
    scale: f32,
    pub(crate) entries: Vec<QuickEntry>,
    /// Labels: the band ("Built-In") and the footer command.
    labels: [String; 2],
    /// First pixel of the list shown (reference pixels).
    scroll: f32,
    hover: Option<usize>,
}

impl QuickTables {
    /// Open beside `anchor` (the Quick Tables row), inside a window of `size`.
    pub(crate) fn open(
        anchor: [f32; 4],
        size: (f32, f32),
        labels: [String; 2],
        scale: f32,
    ) -> Self {
        let k = scale / 1.25;
        let (w, h) = (W * k, H * k);
        let mut x = anchor[0] + anchor[2];
        if x + w > size.0 {
            x = (anchor[0] - w).max(0.0);
        }
        let y = (anchor[1]).min(size.1 - h).max(0.0);
        Self {
            x,
            y,
            scale,
            entries: entries(),
            labels,
            scroll: 0.0,
            hover: None,
        }
    }

    fn k(&self) -> f32 {
        self.scale / 1.25
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.k()).round()
    }

    #[allow(clippy::unused_self)]
    fn list_h(&self) -> f32 {
        H - BAND - FOOT
    }

    fn content_h(&self) -> f32 {
        self.entries.len() as f32 * (NAME + PICTURE)
    }

    /// The panel contains device point (x, y).
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.px(W) && y >= self.y && y < self.y + self.px(H)
    }

    /// Entry under device point (x, y).
    fn entry_at(&self, x: f32, y: f32) -> Option<usize> {
        let (lx, ly) = ((x - self.x) / self.k(), (y - self.y) / self.k());
        if !(0.0..W - BAR).contains(&lx) || !(BAND..H - FOOT).contains(&ly) {
            return None;
        }
        let i = ((ly - BAND + self.scroll) / (NAME + PICTURE)).floor() as usize;
        (i < self.entries.len()).then_some(i)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.entry_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    pub(crate) fn wheel(&mut self, d: f32) -> bool {
        let max = (self.content_h() - self.list_h()).max(0.0);
        let old = self.scroll;
        self.scroll = (self.scroll - d * 60.0).clamp(0.0, max);
        old != self.scroll
    }

    /// A press: the building block key to insert, `Some("")` when the press was on the
    /// panel without choosing, `None` outside it.
    pub(crate) fn press(&mut self, x: f32, y: f32) -> Option<String> {
        if !self.contains(x, y) {
            return None;
        }
        let ly = (y - self.y) / self.k();
        let lx = (x - self.x) / self.k();
        if lx >= W - BAR && (BAND..H - FOOT).contains(&ly) {
            // The scroll bar: a page up or down.
            let page = self.list_h() * 0.8;
            self.wheel(if ly < BAND + self.list_h() / 2.0 {
                page / 60.0
            } else {
                -page / 60.0
            });
            return Some(String::new());
        }
        Some(
            self.entry_at(x, y)
                .and_then(|i| self.entries.get(i))
                .map(|e| e.key.clone())
                .unwrap_or_default(),
        )
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let (x, y, w, h) = (self.x, self.y, self.px(W), self.px(H));
        for (o, a) in [(4.0, 14u8), (2.0, 24)] {
            let o = self.px(o);
            p.fill_rect(
                x - o + 2.0,
                y - o + 3.0,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        // "Built-In" band.
        let band = self.px(BAND);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, band - 1.0, BAND_BG);
        let bold = TextStyle {
            size: self.px(14.5),
            bold: true,
        };
        let line_h = bold.size * 1.33;
        p.text(
            &self.labels[0],
            x + self.px(12.0),
            (y + (band - line_h) / 2.0).round(),
            bold,
            NAME_INK,
        );
        // Entries.
        let top = y + band;
        let list = self.px(self.list_h());
        p.push_clip(x + 1.0, top, w - self.px(BAR) - 1.0, list);
        for (i, e) in self.entries.iter().enumerate() {
            let ey = top + self.px(i as f32 * (NAME + PICTURE) - self.scroll);
            if ey > top + list || ey + self.px(NAME + PICTURE) < top {
                continue;
            }
            let eh = self.px(NAME + PICTURE);
            let ew = w - self.px(BAR) - 2.0;
            if self.hover == Some(i) {
                p.fill_rect(x + 1.0, ey, ew, eh, INK);
                p.fill_rect(x + 3.0, ey + 2.0, ew - 4.0, eh - 4.0, HOT);
            }
            p.text(
                &e.name,
                x + self.px(6.0),
                (ey + (self.px(NAME) - line_h) / 2.0).round(),
                bold,
                if self.hover == Some(i) { INK } else { NAME_INK },
            );
            let pic = [
                x + self.px(10.0),
                ey + self.px(NAME),
                ew - self.px(18.0),
                self.px(PICTURE - 8.0),
            ];
            p.fill_rect(pic[0], pic[1], pic[2], pic[3], WHITE);
            self.paint_table(p, e, pic);
        }
        p.pop_clip();
        // Scroll bar.
        let bx = x + w - self.px(BAR) - 1.0;
        p.fill_rect(bx, top, self.px(BAR), list, Rgba(0xf0, 0xf0, 0xf0, 255));
        let content = self.content_h().max(1.0);
        let th = (list * self.list_h() / content)
            .min(list)
            .max(self.px(24.0));
        let max = (content - self.list_h()).max(1.0);
        let ty = top + (list - th) * (self.scroll / max).min(1.0);
        p.fill_rect(
            bx + self.px(5.0),
            ty,
            self.px(BAR - 10.0),
            th,
            Rgba(0xc2, 0xc3, 0xc9, 255),
        );
        // Footer: save the selection (not available yet).
        let fy = y + h - self.px(FOOT);
        p.fill_rect(x + 1.0, fy, w - 2.0, 1.0, EDGE);
        let st = TextStyle {
            size: self.px(14.0),
            bold: false,
        };
        p.text(
            &self.labels[1],
            x + self.px(44.0),
            (fy + (self.px(FOOT) - st.size * 1.33) / 2.0).round(),
            st,
            DIM,
        );
        let ix = x + self.px(12.0);
        let iy = fy + self.px(11.0);
        let s = self.px(16.0);
        p.path(
            &[
                PathSeg::M(ix, iy),
                PathSeg::L(ix + s, iy),
                PathSeg::L(ix + s, iy + s),
                PathSeg::L(ix, iy + s),
                PathSeg::Z,
                PathSeg::M(ix, iy + s / 3.0),
                PathSeg::L(ix + s, iy + s / 3.0),
                PathSeg::M(ix + s / 2.0, iy),
                PathSeg::L(ix + s / 2.0, iy + s),
            ],
            None,
            Some((DIM, 1.0)),
        );
    }

    /// The table in `r`: columns scaled to fit, rows 21 px, fills, lines and each cell's text.
    fn paint_table(&self, p: &mut dyn Painter, e: &QuickEntry, r: [f32; 4]) {
        let t = &e.table;
        let mut y = r[1] + self.px(6.0);
        if let Some(title) = &e.title {
            let st = TextStyle {
                size: self.px(20.0),
                bold: true,
            };
            p.text(
                title,
                r[0] + self.px(8.0),
                y,
                st,
                Rgba(0x1f, 0x4e, 0x79, 255),
            );
            y += self.px(34.0);
        }
        let total: f32 = t.grid.iter().map(|g| g.0 as f32).sum::<f32>().max(1.0);
        let width = (r[2] - self.px(16.0)).min(total / 20.0 * 1.1 * self.k() * 1.25);
        let x0 = r[0] + self.px(8.0);
        let col_x: Vec<f32> = std::iter::once(0.0)
            .chain(t.grid.iter().scan(0.0, |acc, g| {
                *acc += g.0 as f32;
                Some(*acc)
            }))
            .map(|v| x0 + (v / total * width).round())
            .collect();
        let rh = self.px(23.0);
        let st = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let line =
            |p: &mut dyn Painter, b: &Option<bo_model::Border>, x: f32, y: f32, w: f32, h: f32| {
                if b.as_ref().is_some_and(bo_model::Border::is_visible) {
                    let th = b
                        .as_ref()
                        .map_or(1.0, |b| (b.size as f32 / 8.0).max(1.0).round());
                    let ink = b
                        .as_ref()
                        .and_then(|b| b.color)
                        .map_or(INK, |c| Rgba(c.r, c.g, c.b, 255));
                    p.fill_rect(x, y, w.max(th), h.max(th), ink);
                }
            };
        let borders = t.props.borders.clone().unwrap_or_default();
        let rows = t.rows.len();
        for (ri, row) in t.rows.iter().enumerate() {
            let mut g = 0usize;
            for cell in &row.cells {
                let span = cell.props.grid_span.max(1) as usize;
                let (cx0, cx1) = (
                    col_x[g.min(col_x.len() - 1)],
                    col_x[(g + span).min(col_x.len() - 1)],
                );
                if let Some(c) = cell.props.shading {
                    p.fill_rect(cx0, y, cx1 - cx0, rh, Rgba(c.r, c.g, c.b, 255));
                }
                let para = cell.blocks.iter().find_map(|b| match b {
                    Block::Paragraph(q) => Some(q),
                    _ => None,
                });
                if let Some(q) = para {
                    let text = q.plain_text();
                    let run = q.runs.first().map(|r| &r.props);
                    let bold = run.and_then(|r| r.bold).unwrap_or(false);
                    let ink = run
                        .and_then(|r| r.color)
                        .map_or(INK, |c| Rgba(c.r, c.g, c.b, 255));
                    let st = TextStyle { bold, ..st };
                    let n = text.chars().count() as f32 * st.size * 0.52;
                    let tx = match q.props.align {
                        Some(bo_model::Alignment::Center) => (cx0 + cx1 - n) / 2.0,
                        Some(bo_model::Alignment::Right) => cx1 - n - self.px(4.0),
                        _ => cx0 + self.px(4.0),
                    };
                    p.push_clip(cx0, y, cx1 - cx0, rh);
                    p.text(
                        &text,
                        tx.round(),
                        (y + (rh - st.size * 1.33) / 2.0).round(),
                        st,
                        ink,
                    );
                    p.pop_clip();
                }
                // Lines: the table's edges and inside lines.
                let top = if ri == 0 {
                    &borders.top
                } else {
                    &borders.inside_h
                };
                line(p, top, cx0, y, cx1 - cx0, 0.0);
                if ri + 1 == rows {
                    line(p, &borders.bottom, cx0, y + rh, cx1 - cx0, 0.0);
                }
                let left = if g == 0 {
                    &borders.left
                } else {
                    &borders.inside_v
                };
                line(p, left, cx0, y, 0.0, rh);
                if g + span >= t.grid.len() {
                    line(p, &borders.right, cx1, y, 0.0, rh);
                }
                g += span;
            }
            y += rh;
            if y > r[1] + r[3] {
                break;
            }
        }
    }
}
