//! Design tab drop-downs of BlueDoc parity round 14 (docs/ui/REPORT.md "Vòng 104"), laid out
//! after the reference's at 125 %: the Watermark gallery (category bands, page thumbnails
//! with the watermark text diagonal or horizontal, Custom Watermark… / Remove Watermark) and
//! the Page Color palette (theme colours with tints and shades, standard colours, No Color).
//! The watermark designs are BlueOffice's own (English and Vietnamese office markings).
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const BAND_BG: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const NAME_INK: Rgba = Rgba(0x44, 0x44, 0x44, 255);
const HOT: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const HOT_ROW: Rgba = Rgba(0xc5, 0xc5, 0xc5, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const DIM: Rgba = Rgba(0xa0, 0xa0, 0xa0, 255);
const MARK: Rgba = Rgba(0xc8, 0xc8, 0xc8, 255);

/// Watermark gallery sizes (reference pixels at 125 %): panel, band, cell, page, footer row.
const WM_W: f32 = 643.0;
const WM_BAND: f32 = 36.0;
const WM_LIST: f32 = 546.0;
const WM_ROW: f32 = 36.0;
const WM_CELL_W: f32 = 206.0;
const WM_CELL_H: f32 = 272.0;
const WM_PAGE: (f32, f32) = (180.0, 234.0);
const WM_BAR: f32 = 22.0;

/// One design: its text and whether it runs diagonally.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WmEntry {
    pub(crate) text: String,
    pub(crate) diagonal: bool,
    pub(crate) name: String,
}

/// The built-in designs by category: category key, en-US name, texts.
const WM_SETS: [(&str, &str, &[&str]); 4] = [
    (
        "doc14.wmConfidential",
        "Confidential",
        &["CONFIDENTIAL", "DO NOT COPY"],
    ),
    ("doc14.wmDisclaimers", "Disclaimers", &["DRAFT", "SAMPLE"]),
    ("doc14.wmUrgent", "Urgent", &["URGENT", "ASAP"]),
    (
        "doc14.wmVietnamese",
        "Vietnamese office markings",
        &["MẬT", "TỐI MẬT", "KHẨN", "HỎA TỐC"],
    ),
];

/// Category bands (name) with their designs: each text diagonal (1) and horizontal (2).
pub(crate) fn wm_sets(t: &dyn Fn(&str, &str) -> String) -> Vec<(String, Vec<WmEntry>)> {
    WM_SETS
        .iter()
        .map(|(key, en, texts)| {
            let entries = texts
                .iter()
                .flat_map(|text| {
                    [(true, 1), (false, 2)].map(|(diagonal, n)| WmEntry {
                        text: (*text).to_owned(),
                        diagonal,
                        name: format!("{text} {n}"),
                    })
                })
                .collect();
            (t(key, en), entries)
        })
        .collect()
}

/// What a press on the watermark gallery chose.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WmPick {
    /// Inside, nothing chosen.
    Nothing,
    /// A design.
    Entry(WmEntry),
    /// Custom Watermark….
    Custom,
    /// Remove Watermark.
    Remove,
}

/// The open Watermark gallery.
pub(crate) struct WatermarkGallery {
    x: f32,
    y: f32,
    scale: f32,
    sets: Vec<(String, Vec<WmEntry>)>,
    /// Footer labels: Custom Watermark…, Remove Watermark.
    footer: [String; 2],
    /// The document has a watermark (Remove is available).
    remove_on: bool,
    scroll: f32,
    hover: Option<(usize, usize)>,
    hover_row: Option<usize>,
}

impl WatermarkGallery {
    /// Open under `anchor` (the Watermark button) inside a window of `size`.
    pub(crate) fn open(
        anchor: [f32; 4],
        size: (f32, f32),
        scale: f32,
        sets: Vec<(String, Vec<WmEntry>)>,
        footer: [String; 2],
        remove_on: bool,
    ) -> Self {
        let k = scale / 1.25;
        let w = WM_W * k;
        let x = anchor[0].min(size.0 - w).max(0.0);
        let y = anchor[1] + anchor[3];
        Self {
            x,
            y,
            scale,
            sets,
            footer,
            remove_on,
            scroll: 0.0,
            hover: None,
            hover_row: None,
        }
    }

    fn k(&self) -> f32 {
        self.scale / 1.25
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.k()).round()
    }

    fn height(&self) -> f32 {
        WM_BAND + WM_LIST + 2.0 * WM_ROW
    }

    /// Tops of the category bands and the list's content height (reference pixels; the
    /// first band is the panel's own header).
    fn layout(&self) -> (Vec<f32>, f32) {
        let mut y = 0.0;
        let mut tops = Vec::new();
        for (i, (_, entries)) in self.sets.iter().enumerate() {
            if i > 0 {
                tops.push(y);
                y += WM_BAND;
            } else {
                tops.push(-WM_BAND);
            }
            y += (entries.len().div_ceil(3)) as f32 * WM_CELL_H;
        }
        (tops, y)
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x
            && x < self.x + self.px(WM_W)
            && y >= self.y
            && y < self.y + self.px(self.height())
    }

    /// The design under device point (x, y): (category, entry).
    fn entry_at(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        let (lx, ly) = ((x - self.x) / self.k(), (y - self.y) / self.k());
        if !(0.0..WM_W - WM_BAR).contains(&lx) || !(WM_BAND..WM_BAND + WM_LIST).contains(&ly) {
            return None;
        }
        let at = ly - WM_BAND + self.scroll;
        let (tops, _) = self.layout();
        for (c, (_, entries)) in self.sets.iter().enumerate() {
            let start = tops[c] + WM_BAND;
            let rows = entries.len().div_ceil(3);
            let rel = at - start;
            if rel >= 0.0 && rel < rows as f32 * WM_CELL_H {
                let row = (rel / WM_CELL_H) as usize;
                let col = ((lx - 8.0) / WM_CELL_W).floor();
                if !(0.0..3.0).contains(&col) {
                    return None;
                }
                let i = row * 3 + col as usize;
                return (i < entries.len()).then_some((c, i));
            }
        }
        None
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        let ly = (y - self.y) / self.k() - WM_BAND - WM_LIST;
        (0.0..2.0 * WM_ROW)
            .contains(&ly)
            .then(|| (ly / WM_ROW) as usize)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.entry_at(x, y);
        let r = if self.contains(x, y) {
            self.row_at(y)
        } else {
            None
        };
        let changed = h != self.hover || r != self.hover_row;
        self.hover = h;
        self.hover_row = r;
        changed
    }

    pub(crate) fn wheel(&mut self, d: f32) -> bool {
        let max = (self.layout().1 - WM_LIST).max(0.0);
        let old = self.scroll;
        self.scroll = (self.scroll - d * 60.0).clamp(0.0, max);
        old != self.scroll
    }

    /// A press: what it chose, `None` outside the panel.
    pub(crate) fn press(&mut self, x: f32, y: f32) -> Option<WmPick> {
        if !self.contains(x, y) {
            return None;
        }
        let lx = (x - self.x) / self.k();
        let ly = (y - self.y) / self.k();
        if lx >= WM_W - WM_BAR && (WM_BAND..WM_BAND + WM_LIST).contains(&ly) {
            let page = WM_LIST * 0.8 / 60.0;
            self.wheel(if ly < WM_BAND + WM_LIST / 2.0 {
                page
            } else {
                -page
            });
            return Some(WmPick::Nothing);
        }
        match self.row_at(y) {
            Some(0) => return Some(WmPick::Custom),
            Some(_) if self.remove_on => return Some(WmPick::Remove),
            Some(_) => return Some(WmPick::Nothing),
            None => {}
        }
        Some(
            self.entry_at(x, y)
                .and_then(|(c, i)| self.sets.get(c)?.1.get(i).cloned())
                .map_or(WmPick::Nothing, WmPick::Entry),
        )
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let (x, y, w, h) = (self.x, self.y, self.px(WM_W), self.px(self.height()));
        shadow(p, [x, y, w, h], self.k());
        p.fill_rect(x, y, w, h, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        let bold = TextStyle {
            size: self.px(14.5),
            bold: true,
        };
        let line_h = bold.size * 1.33;
        let band = self.px(WM_BAND);
        let top = y + band;
        let list = self.px(WM_LIST);
        let (tops, content) = self.layout();
        let list_w = w - self.px(WM_BAR) - 2.0;
        p.push_clip(x + 1.0, top, list_w, list);
        for (c, (name, entries)) in self.sets.iter().enumerate() {
            if c > 0 {
                let by = top + self.px(tops[c] - self.scroll);
                p.fill_rect(x + 1.0, by, list_w, band, BAND_BG);
                p.text(
                    name,
                    x + self.px(12.0),
                    (by + (band - line_h) / 2.0).round(),
                    bold,
                    NAME_INK,
                );
            }
            let start = tops[c] + WM_BAND;
            for (i, e) in entries.iter().enumerate() {
                let cy = top + self.px(start + (i / 3) as f32 * WM_CELL_H - self.scroll);
                if cy > top + list || cy + self.px(WM_CELL_H) < top {
                    continue;
                }
                let cx = x + self.px(8.0 + (i % 3) as f32 * WM_CELL_W);
                self.paint_cell(p, e, [cx, cy], self.hover == Some((c, i)));
            }
        }
        p.pop_clip();
        // The first category's band, over the list.
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, band - 1.0, BAND_BG);
        if let Some((name, _)) = self.sets.first() {
            p.text(
                name,
                x + self.px(12.0),
                (y + (band - line_h) / 2.0).round(),
                bold,
                NAME_INK,
            );
        }
        scroll_bar(
            p,
            [x + w - self.px(WM_BAR) - 1.0, top, self.px(WM_BAR), list],
            WM_LIST / content.max(1.0),
            self.scroll / (content - WM_LIST).max(1.0),
            self.k(),
        );
        // Footer rows.
        let fy = top + list;
        p.fill_rect(x + 1.0, fy, w - 2.0, 1.0, EDGE);
        let st = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        for (i, label) in self.footer.iter().enumerate() {
            let ry = fy + self.px(i as f32 * WM_ROW);
            let on = i == 0 || self.remove_on;
            if on && self.hover_row == Some(i) {
                p.fill_rect(x + 1.0, ry + 1.0, w - 2.0, self.px(WM_ROW) - 1.0, HOT_ROW);
            }
            p.text(
                label,
                x + self.px(44.0),
                (ry + (self.px(WM_ROW) - st.size * 1.33) / 2.0).round(),
                st,
                if on { INK } else { DIM },
            );
            footer_icon(
                p,
                x + self.px(13.0),
                ry + self.px(9.0),
                self.px(18.0),
                i == 1,
                on,
            );
        }
    }

    /// One design: the page with its watermark, its name below; hovered: framed.
    fn paint_cell(&self, p: &mut dyn Painter, e: &WmEntry, at: [f32; 2], hot: bool) {
        let (cw, ch) = (self.px(WM_CELL_W - 10.0), self.px(WM_CELL_H - 4.0));
        if hot {
            p.fill_rect(at[0], at[1], cw, ch, INK);
            p.fill_rect(at[0] + 2.0, at[1] + 2.0, cw - 4.0, ch - 4.0, HOT);
        }
        let (pw, ph) = (self.px(WM_PAGE.0), self.px(WM_PAGE.1));
        let px = at[0] + ((cw - pw) / 2.0).round();
        let py = at[1] + self.px(4.0);
        p.fill_rect(px, py, pw, ph, INK);
        p.fill_rect(px + 1.0, py + 1.0, pw - 2.0, ph - 2.0, WHITE);
        p.push_clip(px + 1.0, py + 1.0, pw - 2.0, ph - 2.0);
        paint_mark(p, &e.text, e.diagonal, [px, py, pw, ph]);
        p.pop_clip();
        let st = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let tw = st.size * 0.56 * e.name.chars().count() as f32;
        p.text(
            &e.name,
            (at[0] + (cw - tw) / 2.0).round(),
            py + ph + self.px(5.0),
            st,
            NAME_INK,
        );
    }
}

/// The watermark text on a page thumbnail: horizontal across the middle, or its letters
/// stepping up along the rising diagonal.
fn paint_mark(p: &mut dyn Painter, text: &str, diagonal: bool, r: [f32; 4]) {
    let n = text.chars().count().max(1) as f32;
    if diagonal {
        let span = (r[2] * r[2] + r[3] * r[3]).sqrt() * 0.62;
        let size = (span / n * 1.25).clamp(9.0, r[2] * 0.22);
        let st = TextStyle { size, bold: false };
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        let (dx, dy) = (r[2] * 0.62 / n, -r[3] * 0.48 / n);
        let x0 = cx - dx * n / 2.0;
        let y0 = cy - dy * n / 2.0 - size * 0.7;
        for (i, c) in text.chars().enumerate() {
            let i = i as f32 + 0.5;
            let mut b = [0u8; 4];
            p.text(
                c.encode_utf8(&mut b),
                x0 + dx * i - size * 0.3,
                y0 + dy * i,
                st,
                MARK,
            );
        }
    } else {
        let size = (r[2] * 0.86 / (n * 0.56)).min(r[2] * 0.18);
        let st = TextStyle { size, bold: false };
        let tw = size * 0.56 * n;
        p.text(
            text,
            r[0] + (r[2] - tw) / 2.0,
            r[1] + (r[3] - size * 1.33) / 2.0,
            st,
            MARK,
        );
    }
}

/// Footer icons: a page with a text mark (Custom), the page with a red cross (Remove).
fn footer_icon(p: &mut dyn Painter, x: f32, y: f32, s: f32, remove: bool, on: bool) {
    let grey = if on { Rgba(0x6e, 0x6e, 0x6e, 255) } else { DIM };
    let page = [
        PathSeg::M(x + s * 0.15, y),
        PathSeg::L(x + s * 0.65, y),
        PathSeg::L(x + s * 0.85, y + s * 0.2),
        PathSeg::L(x + s * 0.85, y + s),
        PathSeg::L(x + s * 0.15, y + s),
        PathSeg::Z,
    ];
    p.path(&page, Some(WHITE), Some((grey, 1.0)));
    let accent = if !on {
        DIM
    } else if remove {
        Rgba(0xc0, 0x39, 0x2b, 255)
    } else {
        Rgba(0xd0, 0x80, 0x80, 255)
    };
    if remove {
        let (a, b) = (x + s * 0.45, y + s * 0.5);
        let d = s * 0.25;
        p.path(
            &[
                PathSeg::M(a, b),
                PathSeg::L(a + 2.0 * d, b + 2.0 * d),
                PathSeg::M(a + 2.0 * d, b),
                PathSeg::L(a, b + 2.0 * d),
            ],
            None,
            Some((accent, 1.5)),
        );
    } else {
        p.path(
            &[
                PathSeg::M(x + s * 0.3, y + s * 0.8),
                PathSeg::L(x + s * 0.5, y + s * 0.3),
                PathSeg::L(x + s * 0.7, y + s * 0.8),
                PathSeg::M(x + s * 0.38, y + s * 0.62),
                PathSeg::L(x + s * 0.62, y + s * 0.62),
            ],
            None,
            Some((accent, 1.2)),
        );
    }
}

fn shadow(p: &mut dyn Painter, r: [f32; 4], k: f32) {
    for (o, a) in [(4.0, 14u8), (2.0, 24)] {
        let o = (o * k).round();
        p.fill_rect(
            r[0] - o + 2.0,
            r[1] - o + 3.0,
            r[2] + 2.0 * o,
            r[3] + 2.0 * o,
            Rgba(0, 0, 0, a),
        );
    }
}

/// A vertical scroll bar: track, thumb of `frac` of it at `pos` (0–1), arrows.
fn scroll_bar(p: &mut dyn Painter, r: [f32; 4], frac: f32, pos: f32, k: f32) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xf0, 0xf0, 0xf0, 255));
    let arrow = (17.0 * k).round();
    let track = r[3] - 2.0 * arrow;
    let th = (track * frac.min(1.0)).max(24.0 * k);
    let ty = r[1] + arrow + (track - th) * pos.clamp(0.0, 1.0);
    p.fill_rect(
        r[0] + (5.0 * k).round(),
        ty,
        r[2] - (10.0 * k).round(),
        th,
        Rgba(0xc2, 0xc3, 0xc9, 255),
    );
    let c = Rgba(0x60, 0x60, 0x60, 255);
    let (cx, a) = (r[0] + r[2] / 2.0, 4.0 * k);
    for (cy, up) in [
        (r[1] + arrow / 2.0, true),
        (r[1] + r[3] - arrow / 2.0, false),
    ] {
        let s = if up { -1.0 } else { 1.0 };
        p.path(
            &[
                PathSeg::M(cx - a, cy - s * a / 2.0),
                PathSeg::L(cx + a, cy - s * a / 2.0),
                PathSeg::L(cx, cy + s * a / 2.0),
                PathSeg::Z,
            ],
            Some(c),
            None,
        );
    }
}

// ---------------------------------------------------------------------------------- Page Color

/// Page Color palette sizes (reference pixels at 125 %).
const PC_W: f32 = 404.0;
const PC_CELL: f32 = 34.0;
const PC_PITCH: f32 = 40.0;
const PC_LEFT: f32 = 4.0;

/// What a press on the palette chose.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PcPick {
    Nothing,
    /// A colour (`RRGGBB`).
    Color(String),
    /// No Color.
    None,
}

/// The open Page Color palette.
pub(crate) struct PagePalette {
    x: f32,
    y: f32,
    scale: f32,
    /// 60 theme colours (10 × 6) then 10 standard colours.
    colors: Vec<String>,
    /// High-contrast only, Theme Colors, Standard Colors, No Color, More Colors…,
    /// Fill Effects….
    labels: [String; 6],
    /// The page colour now.
    current: Option<String>,
    hover: Option<usize>,
    hover_row: Option<usize>,
}

/// Rows of the palette (reference px): top of the first theme row, the tints, the standard
/// row, the No Color row; total height.
const PC_THEME_Y: f32 = 95.0;
const PC_TINT_Y: f32 = 148.0;
const PC_STD_Y: f32 = 393.0;
const PC_ROWS_Y: f32 = 447.0;
const PC_ROW: f32 = 36.0;
const PC_H: f32 = 561.0;

impl PagePalette {
    pub(crate) fn open(
        anchor: [f32; 4],
        size: (f32, f32),
        scale: f32,
        colors: Vec<String>,
        labels: [String; 6],
        current: Option<String>,
    ) -> Self {
        let k = scale / 1.25;
        let w = PC_W * k;
        let x = anchor[0].min(size.0 - w).max(0.0);
        Self {
            x,
            y: anchor[1] + anchor[3],
            scale,
            colors,
            labels,
            current,
            hover: None,
            hover_row: None,
        }
    }

    fn k(&self) -> f32 {
        self.scale / 1.25
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.k()).round()
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.px(PC_W) && y >= self.y && y < self.y + self.px(PC_H)
    }

    /// Cell rectangle of colour `i` (reference px from the panel's corner).
    fn cell(i: usize) -> [f32; 4] {
        let col = (i % 10) as f32;
        let x = PC_LEFT + col * PC_PITCH;
        match i {
            0..=9 => [x, PC_THEME_Y, PC_CELL, PC_CELL],
            10..=59 => {
                let row = ((i - 10) / 10) as f32;
                [x, PC_TINT_Y + row * 36.0, PC_CELL, 36.0]
            }
            _ => [x, PC_STD_Y, PC_CELL, PC_CELL],
        }
    }

    fn color_at(&self, x: f32, y: f32) -> Option<usize> {
        let (lx, ly) = ((x - self.x) / self.k(), (y - self.y) / self.k());
        (0..self.colors.len()).find(|&i| {
            let r = Self::cell(i);
            lx >= r[0] && lx < r[0] + r[2] && ly >= r[1] && ly < r[1] + r[3]
        })
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        let ly = (y - self.y) / self.k() - PC_ROWS_Y;
        (0.0..3.0 * PC_ROW)
            .contains(&ly)
            .then(|| (ly / PC_ROW) as usize)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.color_at(x, y);
        let r = if self.contains(x, y) {
            self.row_at(y)
        } else {
            None
        };
        let changed = h != self.hover || r != self.hover_row;
        self.hover = h;
        self.hover_row = r;
        changed
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> Option<PcPick> {
        if !self.contains(x, y) {
            return None;
        }
        if let Some(i) = self.color_at(x, y) {
            return Some(PcPick::Color(self.colors[i].clone()));
        }
        Some(match self.row_at(y) {
            Some(0) => PcPick::None,
            _ => PcPick::Nothing,
        })
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let (x, y, w, h) = (self.x, self.y, self.px(PC_W), self.px(PC_H));
        shadow(p, [x, y, w, h], self.k());
        p.fill_rect(x, y, w, h, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        let st = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let bold = TextStyle {
            size: self.px(14.5),
            bold: true,
        };
        let lh = st.size * 1.33;
        // High-contrast only (dimmed, as the reference with a theme palette).
        let b = self.px(16.0);
        p.fill_rect(x + self.px(8.0), y + self.px(18.0), b, b, DIM);
        p.fill_rect(
            x + self.px(8.0) + 1.0,
            y + self.px(18.0) + 1.0,
            b - 2.0,
            b - 2.0,
            WHITE,
        );
        p.text(
            &self.labels[0],
            x + self.px(33.0),
            y + self.px(26.0) - lh / 2.0,
            st,
            DIM,
        );
        p.fill_rect(x + 1.0, y + self.px(48.0), w - 2.0, 1.0, BAND_BG);
        p.text(
            &self.labels[1],
            x + self.px(16.0),
            y + self.px(72.0) - lh / 2.0,
            bold,
            NAME_INK,
        );
        p.text(
            &self.labels[2],
            x + self.px(16.0),
            y + self.px(369.0) - lh / 2.0,
            bold,
            NAME_INK,
        );
        for (i, hex) in self.colors.iter().enumerate() {
            let r = Self::cell(i);
            let (cx, cy, cw, ch) = (
                x + self.px(r[0]),
                y + self.px(r[1]),
                self.px(r[2]),
                self.px(r[3]),
            );
            let c = crate::doc_wm14::hex_rgba(hex);
            p.fill_rect(cx, cy, cw, ch, c);
            // Light cells get a grey frame (white would vanish), as the reference.
            if i < 10 || i >= 60 {
                outline(p, [cx, cy, cw, ch], Rgba(0xe0, 0xe0, 0xe0, 255));
            }
            if self.hover == Some(i) {
                outline(
                    p,
                    [cx - 1.0, cy - 1.0, cw + 2.0, ch + 2.0],
                    Rgba(0xf2, 0x94, 0x36, 255),
                );
                outline(p, [cx, cy, cw, ch], WHITE);
            }
            if self
                .current
                .as_deref()
                .is_some_and(|h| h.eq_ignore_ascii_case(hex))
            {
                outline(p, [cx, cy, cw, ch], INK);
            }
        }
        p.fill_rect(x + 1.0, y + self.px(437.0), w - 2.0, 1.0, BAND_BG);
        for (i, label) in self.labels[3..].iter().enumerate() {
            let ry = y + self.px(PC_ROWS_Y + i as f32 * PC_ROW);
            let on = i == 0;
            if on && (self.hover_row == Some(0) || self.current.is_none()) {
                p.fill_rect(x + 2.0, ry, w - 4.0, self.px(PC_ROW), HOT_ROW);
            }
            if i == 0 {
                let s = self.px(22.0);
                let sx = x + self.px(14.0);
                let sy = ry + (self.px(PC_ROW) - s) / 2.0;
                outline(p, [sx, sy, s, s], DIM);
            }
            p.text(
                label,
                x + self.px(if i == 0 { 44.0 } else { 34.0 }),
                ry + (self.px(PC_ROW) - lh) / 2.0,
                st,
                if on { INK } else { DIM },
            );
        }
    }
}

fn outline(p: &mut dyn Painter, r: [f32; 4], c: Rgba) {
    p.fill_rect(r[0], r[1], r[2], 1.0, c);
    p.fill_rect(r[0], r[1] + r[3] - 1.0, r[2], 1.0, c);
    p.fill_rect(r[0], r[1], 1.0, r[3], c);
    p.fill_rect(r[0] + r[2] - 1.0, r[1], 1.0, r[3], c);
}

/// `RRGGBB` as a colour (black when malformed).
pub(crate) fn hex_rgba(hex: &str) -> Rgba {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gallery() -> WatermarkGallery {
        let t = |_: &str, en: &str| en.to_owned();
        WatermarkGallery::open(
            [100.0, 150.0, 90.0, 120.0],
            (1500.0, 1102.0),
            1.25,
            wm_sets(&t),
            ["Custom Watermark...".into(), "Remove Watermark".into()],
            false,
        )
    }

    #[test]
    fn watermark_gallery_picks_designs_and_footer_rows() {
        let mut g = gallery();
        // First design: diagonal CONFIDENTIAL, under the panel's band.
        let pick = g.press(100.0 + 60.0, 270.0 + 36.0 + 100.0);
        assert_eq!(
            pick,
            Some(WmPick::Entry(WmEntry {
                text: "CONFIDENTIAL".into(),
                diagonal: true,
                name: "CONFIDENTIAL 1".into(),
            }))
        );
        // Second column: horizontal.
        let Some(WmPick::Entry(e)) = g.press(100.0 + 8.0 + 206.0 + 60.0, 270.0 + 136.0) else {
            panic!("entry");
        };
        assert!(!e.diagonal);
        let footer = 270.0 + 36.0 + 546.0;
        assert_eq!(g.press(200.0, footer + 10.0), Some(WmPick::Custom));
        // Remove is dimmed without a watermark.
        assert_eq!(g.press(200.0, footer + 46.0), Some(WmPick::Nothing));
        assert_eq!(g.press(10.0, 10.0), None);
        // The list scrolls to the Vietnamese markings.
        assert!(g.wheel(-100.0));
    }

    #[test]
    fn page_palette_picks_colours_and_no_color() {
        let colors: Vec<String> = (0..70).map(|i| format!("{:06X}", i * 1000)).collect();
        let mut pal = PagePalette::open(
            [1200.0, 100.0, 60.0, 100.0],
            (1500.0, 1102.0),
            1.25,
            colors,
            Default::default(),
            None,
        );
        let x0 = 1500.0 - 404.0;
        assert_eq!(
            pal.press(x0 + 4.0 + 40.0 + 10.0, 200.0 + 95.0 + 10.0),
            Some(PcPick::Color(format!("{:06X}", 1000)))
        );
        assert_eq!(
            pal.press(x0 + 10.0, 200.0 + 393.0 + 10.0),
            Some(PcPick::Color(format!("{:06X}", 60 * 1000)))
        );
        assert_eq!(
            pal.press(x0 + 100.0, 200.0 + 447.0 + 10.0),
            Some(PcPick::None)
        );
    }
}
