//! Picture galleries of BlueSheet's Home tab (round 16): Cell Styles (each style shown in its
//! own font, colour, fill and border, grouped as the reference groups them) and Format as Table
//! (a small table per style: header row, banded rows), opened under their ribbon buttons
//! like the reference spreadsheet's galleries.
use super::SheetWindow;
use crate::ribbon::RibbonHit;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::Rgba;

const BG: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const HEAD_BG: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const HOT: Rgba = Rgba(0xf2, 0x94, 0x36, 255);
const HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);

/// What a tile shows.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tile {
    /// A cell style: name, fill, font colour, bold, size (points), bottom border colour.
    Cell {
        name: String,
        fill: Option<Rgba>,
        color: Rgba,
        bold: bool,
        size: f32,
        line: Option<Rgba>,
    },
    /// A table style: header fill, header text colour, band fill.
    Table {
        header: Option<Rgba>,
        header_ink: Rgba,
        stripe: Rgba,
        edge: Rgba,
    },
}

/// An open gallery: sections of tiles, then menu lines.
#[derive(Debug, Clone)]
pub(super) struct Gallery {
    pub sections: Vec<(String, Vec<(String, Tile)>)>,
    pub footer: Vec<(String, String)>,
    /// Tile size and columns (points).
    pub tile: (f32, f32),
    pub cols: usize,
    pub origin: (f32, f32),
    pub k: f32,
    pub hover: Option<(usize, usize)>,
}

/// Section caption and gap between tiles (points; 30 px and 8 px at 125 %, measured).
const HEAD_H: f32 = 24.0;
const GAP: f32 = 6.4;
const FOOT_H: f32 = 22.0;

impl Gallery {
    fn d(&self, v: f32) -> f32 {
        (v * self.k).round()
    }

    fn width(&self) -> f32 {
        GAP + self.cols as f32 * (self.tile.0 + GAP)
    }

    /// Top of section `s` and of its tiles (points from the gallery's top).
    fn section_y(&self, s: usize) -> f32 {
        let mut y = 1.0;
        for (_, tiles) in self.sections.iter().take(s) {
            let rows = tiles.len().div_ceil(self.cols);
            y += HEAD_H + GAP + rows as f32 * (self.tile.1 + GAP);
        }
        y
    }

    fn height(&self) -> f32 {
        self.section_y(self.sections.len()) + 4.0 + self.footer.len() as f32 * FOOT_H + 4.0
    }

    /// Device rectangle.
    pub fn rect(&self) -> [f32; 4] {
        [
            self.origin.0,
            self.origin.1,
            self.d(self.width()),
            self.d(self.height()),
        ]
    }

    fn tile_rect(&self, s: usize, i: usize) -> [f32; 4] {
        let (row, col) = (i / self.cols, i % self.cols);
        let y = self.section_y(s) + HEAD_H + GAP + row as f32 * (self.tile.1 + GAP);
        let x = GAP + col as f32 * (self.tile.0 + GAP);
        [x, y, self.tile.0, self.tile.1]
    }

    /// The choice under (`x`, `y`): `(section, tile)` or `(usize::MAX, footer line)`.
    pub fn hit(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        let (lx, ly) = ((x - self.origin.0) / self.k, (y - self.origin.1) / self.k);
        if lx < 0.0 || ly < 0.0 || lx >= self.width() || ly >= self.height() {
            return None;
        }
        for (s, (_, tiles)) in self.sections.iter().enumerate() {
            for i in 0..tiles.len() {
                let r = self.tile_rect(s, i);
                if lx >= r[0] && lx < r[0] + r[2] && ly >= r[1] && ly < r[1] + r[3] {
                    return Some((s, i));
                }
            }
        }
        let fy = self.section_y(self.sections.len()) + 4.0;
        let i = ((ly - fy) / FOOT_H).floor();
        (i >= 0.0 && (i as usize) < self.footer.len()).then_some((usize::MAX, i as usize))
    }

    /// The command of a hit.
    pub fn choice(&self, (s, i): (usize, usize)) -> Option<String> {
        if s == usize::MAX {
            return self.footer.get(i).map(|(_, c)| c.clone());
        }
        self.sections.get(s)?.1.get(i).map(|(c, _)| c.clone())
    }

    pub fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect();
        for (o, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
            let o = (o * self.k).round();
            p.fill_rect(
                x - o + 2.0,
                y - o + 3.0,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
        let style = TextStyle {
            size: (12.0 * self.k).round(),
            bold: false,
        };
        let line = style.size * 1.33;
        for (s, (title, tiles)) in self.sections.iter().enumerate() {
            let hy = y + self.d(self.section_y(s));
            p.fill_rect(x + 1.0, hy, w - 2.0, self.d(HEAD_H), HEAD_BG);
            let bold = TextStyle {
                bold: true,
                ..style
            };
            p.text(
                title,
                x + self.d(6.0),
                (hy + (self.d(HEAD_H) - line) / 2.0).round(),
                bold,
                INK,
            );
            for (i, (_, tile)) in tiles.iter().enumerate() {
                let r = self.tile_rect(s, i);
                let r = [
                    x + self.d(r[0]),
                    y + self.d(r[1]),
                    self.d(r[2]),
                    self.d(r[3]),
                ];
                if self.hover == Some((s, i)) {
                    p.fill_rect(r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0, HOT);
                    p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, BG);
                }
                self.paint_tile(p, r, tile, style);
            }
        }
        let fy = y + self.d(self.section_y(self.sections.len()) + 4.0);
        p.fill_rect(x + 1.0, fy - self.d(2.0), w - 2.0, 1.0, EDGE);
        for (i, (label, _)) in self.footer.iter().enumerate() {
            let ry = fy + self.d(i as f32 * FOOT_H);
            if self.hover == Some((usize::MAX, i)) {
                p.fill_rect(x + 1.0, ry, w - 2.0, self.d(FOOT_H), HOVER);
            }
            p.text(
                label,
                x + self.d(30.0),
                (ry + (self.d(FOOT_H) - line) / 2.0).round(),
                style,
                INK,
            );
        }
    }

    fn paint_tile(&self, p: &mut dyn Painter, r: [f32; 4], tile: &Tile, style: TextStyle) {
        match tile {
            Tile::Cell {
                name,
                fill,
                color,
                bold,
                size,
                line,
            } => {
                // Only Normal is framed (the current style), as on the reference.
                if name == "Normal" {
                    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xab, 0xab, 0xab, 255));
                } else {
                    p.fill_rect(r[0], r[1], r[2], r[3], fill.unwrap_or(BG));
                }
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    fill.unwrap_or(BG),
                );
                if let Some(l) = line {
                    p.fill_rect(
                        r[0] + 2.0,
                        r[1] + r[3] - self.d(4.0),
                        r[2] - 4.0,
                        self.d(2.0),
                        *l,
                    );
                }
                // Large styles (titles) are shown smaller to fit, as the reference does.
                let ts = TextStyle {
                    // The reference shows tile text about 15 % larger than the menu text.
                    size: (style.size * 1.15 * (size / 11.0).clamp(1.0, 1.4)).round(),
                    bold: *bold,
                };
                p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
                p.text(
                    name,
                    r[0] + self.d(4.0),
                    (r[1] + (r[3] - ts.size * 1.33) / 2.0).round(),
                    ts,
                    *color,
                );
                p.pop_clip();
            }
            Tile::Table {
                header,
                header_ink,
                stripe,
                edge,
            } => {
                p.fill_rect(r[0], r[1], r[2], r[3], *edge);
                p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BG);
                let rows = 5;
                let rh = (r[3] - 2.0) / rows as f32;
                for k in 0..rows {
                    let ry = (r[1] + 1.0 + k as f32 * rh).round();
                    let fill = match (k, header) {
                        (0, Some(hf)) => Some(*hf),
                        (k, _) if k % 2 == 1 => Some(*stripe),
                        _ => None,
                    };
                    if let Some(f) = fill {
                        p.fill_rect(r[0] + 1.0, ry, r[2] - 2.0, rh.ceil(), f);
                    }
                    // Cell text dashes.
                    let ink = if k == 0 {
                        *header_ink
                    } else {
                        Rgba(0x80, 0x80, 0x80, 255)
                    };
                    for c in 0..4 {
                        let cx = r[0] + 3.0 + c as f32 * (r[2] - 6.0) / 4.0;
                        p.fill_rect(
                            cx.round(),
                            (ry + rh / 2.0).round(),
                            ((r[2] - 6.0) / 4.0 - 4.0).round(),
                            1.0,
                            ink,
                        );
                    }
                }
            }
        }
    }
}

fn hex(s: &str) -> Option<Rgba> {
    let s = s.trim_start_matches('#');
    let s = if s.len() == 8 { &s[2..] } else { s };
    let v = u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6)?;
    Some(Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255))
}

/// Gallery groups of the cell styles (the reference's sections, for the styles BlueSheet has).
const CELL_SECTIONS: [(&str, &str, &[&str]); 4] = [
    (
        "sheetGallery.goodBad",
        "Good, Bad and Neutral",
        &["Normal", "Bad", "Good", "Neutral"],
    ),
    (
        "sheetGallery.dataModel",
        "Data and Model",
        &[
            "Calculation",
            "Check Cell",
            "Explanatory Text",
            "Input",
            "Linked Cell",
            "Note",
            "Output",
            "Warning Text",
        ],
    ),
    (
        "sheetGallery.titles",
        "Titles and Headings",
        &[
            "Heading 1",
            "Heading 2",
            "Heading 3",
            "Heading 4",
            "Title",
            "Total",
        ],
    ),
    (
        "sheetGallery.themed",
        "Themed Cell Styles",
        &[
            "20% - Accent1",
            "20% - Accent2",
            "20% - Accent3",
            "20% - Accent4",
            "20% - Accent5",
            "20% - Accent6",
            "40% - Accent1",
            "40% - Accent2",
            "40% - Accent3",
            "40% - Accent4",
            "40% - Accent5",
            "40% - Accent6",
            "60% - Accent1",
            "60% - Accent2",
            "60% - Accent3",
            "60% - Accent4",
            "60% - Accent5",
            "60% - Accent6",
            "Accent1",
            "Accent2",
            "Accent3",
            "Accent4",
            "Accent5",
            "Accent6",
        ],
    ),
];

impl SheetWindow {
    /// Open the Cell Styles gallery (`cells`) or the Format as Table gallery.
    pub(super) fn open_gallery(&mut self, cells: bool) -> bool {
        let id = if cells {
            "sheet.cellStyles"
        } else {
            "sheet.formatTable"
        };
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let mut g = if cells {
            self.cell_style_gallery()
        } else {
            self.table_gallery()
        };
        g.k = self.scale;
        let w = (g.width() * g.k).round();
        g.origin = (
            anchor[0].min(self.size.0 - w - 2.0).max(0.0).round(),
            (anchor[1] + anchor[3] + 2.0).round(),
        );
        self.gallery = Some(g);
        true
    }

    fn cell_style_gallery(&self) -> Gallery {
        // Each style's look, read from a scratch workbook styled with it.
        let mut wb = bo_model::sheet::Workbook::new();
        let mut sections = Vec::new();
        for (key, fallback, names) in CELL_SECTIONS {
            let mut tiles = Vec::new();
            for (i, name) in names.iter().enumerate() {
                let at = bo_model::sheet::CellRef::new(0, i as u32);
                let range = bo_model::sheet::CellRange::new(at, at);
                bo_sheet::style::apply_named_style(&mut wb, 0, range, name);
                let st = bo_sheet::style::format_state(
                    &wb,
                    0,
                    0,
                    i as u32,
                    &bo_sheet::numfmt::Locale::EN_US,
                );
                let line = st
                    .borders
                    .get(2)
                    .filter(|b| !b.is_empty())
                    .map(|_| Rgba(0x4a, 0x7b, 0xb7, 255));
                tiles.push((
                    format!("style.{name}"),
                    Tile::Cell {
                        name: (*name).to_owned(),
                        fill: hex(&st.fill_color),
                        color: hex(&st.font_color).unwrap_or(INK),
                        bold: st.bold,
                        size: st.font_size as f32,
                        line,
                    },
                ));
            }
            sections.push((self.t(key, fallback), tiles));
        }
        Gallery {
            sections,
            footer: Vec::new(),
            // 153 × 36 px tiles, six to a row, as the reference's gallery.
            tile: (122.4, 28.8),
            cols: 6,
            origin: (0.0, 0.0),
            k: 1.0,
            hover: None,
        }
    }

    fn table_gallery(&self) -> Gallery {
        let entries = bo_sheet::table_style::gallery();
        let mut sections = Vec::new();
        for (family, key, fallback) in [
            ("light", "sheetGallery.light", "Light"),
            ("medium", "sheetGallery.medium", "Medium"),
            ("dark", "sheetGallery.dark", "Dark"),
        ] {
            let tiles = entries
                .iter()
                .filter(|e| e.family == family)
                .map(|e| {
                    let header = e.header.as_deref().and_then(hex);
                    (
                        format!("table.{}", e.name),
                        Tile::Table {
                            header,
                            header_ink: e
                                .header_font
                                .as_deref()
                                .and_then(hex)
                                .unwrap_or(if header.is_some() { BG } else { INK }),
                            stripe: hex(&e.stripe).unwrap_or(BG),
                            edge: header.unwrap_or(Rgba(0xa6, 0xa6, 0xa6, 255)),
                        },
                    )
                })
                .collect();
            sections.push((self.t(key, fallback), tiles));
        }
        Gallery {
            sections,
            footer: Vec::new(),
            tile: (49.0, 37.0),
            cols: 7,
            origin: (0.0, 0.0),
            k: 1.0,
            hover: None,
        }
    }

    /// Input while a gallery is open: `Some(redraw)` when it took the input.
    pub(super) fn gallery_input(
        &mut self,
        input: &Input,
        text: &dyn crate::TextMeasure,
    ) -> Option<bool> {
        let g = self.gallery.as_mut()?;
        Some(match *input {
            Input::Move { x, y } => {
                let h = g.hit(x, y);
                std::mem::replace(&mut g.hover, h) != h
            }
            Input::Press { x, y, .. } => {
                let choice = g.hit(x, y).and_then(|h| g.choice(h));
                self.gallery = None;
                self.ribbon.clear_pointer();
                if let Some(c) = choice {
                    self.action(&c, text);
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.gallery = None;
                true
            }
            Input::Release { .. } | Input::Drag { .. } | Input::Wheel(_) => true,
            _ => return None,
        })
    }

    pub(super) fn paint_gallery(&self, p: &mut dyn Painter) {
        if let Some(g) = &self.gallery {
            g.paint(p);
        }
    }

    /// Format as Table from the gallery with the selection's region (tests).
    #[cfg(test)]
    pub(super) fn gallery_choice_at(&self, s: usize, i: usize) -> Option<String> {
        self.gallery.as_ref()?.choice((s, i))
    }
}
