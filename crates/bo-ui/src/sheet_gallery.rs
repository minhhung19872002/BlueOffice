//! Picture galleries of BlueSheet's Home tab (round 16): Cell Styles (each style shown in its
//! own font, colour, fill and border, grouped as the reference groups them) and Format as Table
//! (a small table per style: header row, banded rows), opened under their ribbon buttons
//! like the reference spreadsheet's galleries.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::ribbon::RibbonHit;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::Rgba;
use serde_json::json;

const BG: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const HEAD_BG: Rgba = Rgba(0xeb, 0xeb, 0xeb, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const HOT: Rgba = Rgba(0xd2, 0xd0, 0xce, 255);
const HOT_EDGE: Rgba = Rgba(0x88, 0x8a, 0x86, 255);
const HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
/// Caption text, menu rule and size grip of the reference's gallery (measured).
const HEAD_INK: Rgba = Rgba(0x5e, 0x5e, 0x5e, 255);
const RULE: Rgba = Rgba(0xe2, 0xe4, 0xe7, 255);
const GRIP: Rgba = Rgba(0xa0, 0xa0, 0xa0, 255);
const ICON_INK: Rgba = Rgba(0x72, 0x72, 0x72, 255);
const ICON_BLUE: Rgba = Rgba(0x2b, 0x79, 0xd0, 255);

/// Own drawings for the gallery's menu lines: a small table (New Cell Style…) and a table
/// with an arrow into it (Merge Styles…); (`x`, `cy`) is the left edge and vertical middle.
fn paint_menu_icon(p: &mut dyn Painter, icon: &str, x: f32, cy: f32, k: f32) {
    if icon.is_empty() {
        return;
    }
    let u = |v: f32| (v * k / 1.25).round();
    let (w, h) = (u(22.0), u(16.0));
    let (x, y) = (x.round(), (cy - h / 2.0).round());
    p.fill_rect(x, y, w, h, ICON_INK);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, BG);
    for i in 1..3 {
        p.fill_rect(
            x + 1.0,
            y + (h * i as f32 / 3.0).round(),
            w - 2.0,
            1.0,
            ICON_INK,
        );
    }
    p.fill_rect(x + u(7.0), y + 1.0, 1.0, h - 2.0, ICON_INK);
    if icon == "merge" {
        // Arrow pointing into the table's top row.
        p.fill_rect(x + u(12.0), y - u(6.0), u(2.0), u(5.0), ICON_BLUE);
        p.fill_rect(x + u(10.0), y - u(2.0), u(6.0), u(2.0), ICON_BLUE);
    } else {
        p.fill_rect(
            x + u(8.0),
            y + (h / 3.0).round() + 1.0,
            w - u(9.0),
            (h / 3.0).round() - 1.0,
            ICON_BLUE,
        );
    }
}

/// What a tile shows.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tile {
    /// A cell style: name (as shown), fill, font colour, weight, slant, size (points),
    /// typeface and its edges (top, right, bottom, left: line kind and colour).
    Cell {
        name: String,
        fill: Option<Rgba>,
        color: Rgba,
        bold: bool,
        italic: bool,
        size: f32,
        family: String,
        edges: [Option<(Line, Rgba)>; 4],
    },
    /// A table style (`TableStyleMedium2`), drawn as a small table in its look.
    Table { name: String },
}

/// A cell edge as the gallery draws it (§18.18.3 ST_BorderStyle, grouped by weight).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Line {
    Thin,
    Medium,
    Thick,
    Double,
}

impl Line {
    fn of(style: Option<&str>) -> Option<Line> {
        Some(match style? {
            "" | "none" => return None,
            "medium" | "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" | "slantDashDot" => {
                Line::Medium
            }
            "thick" => Line::Thick,
            "double" => Line::Double,
            _ => Line::Thin,
        })
    }
}

/// An open gallery: sections of tiles, then menu lines.
#[derive(Debug, Clone)]
pub(super) struct Gallery {
    pub sections: Vec<(String, Vec<(String, Tile)>)>,
    /// Menu lines: label, command, icon (`"new"` / `"merge"` / `""`).
    pub footer: Vec<(String, String, &'static str)>,
    /// Tile size and columns (points).
    pub tile: (f32, f32),
    pub cols: usize,
    /// Distance between tile columns and rows, first tile's offset from the left edge and
    /// from its row's top, section caption height, menu line height, total width (points).
    pub pitch: (f32, f32),
    pub inset: (f32, f32),
    pub head: f32,
    pub foot: f32,
    pub width: f32,
    pub origin: (f32, f32),
    pub k: f32,
    pub hover: Option<(usize, usize)>,
}

/// Size grip strip under the menu lines (points; 15 px at 125 %, measured).
const GRIP_H: f32 = 12.0;

impl Gallery {
    fn d(&self, v: f32) -> f32 {
        (v * self.k).round()
    }

    fn width(&self) -> f32 {
        self.width
    }

    /// Top of section `s` (points from the gallery's top).
    fn section_y(&self, s: usize) -> f32 {
        let mut y = 2.4;
        for (_, tiles) in self.sections.iter().take(s) {
            let rows = tiles.len().div_ceil(self.cols);
            y += self.head + rows as f32 * self.pitch.1;
        }
        y
    }

    fn footer_y(&self) -> f32 {
        self.section_y(self.sections.len()) + 1.0
    }

    fn height(&self) -> f32 {
        self.footer_y() + self.footer.len() as f32 * self.foot + GRIP_H
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
        let y = self.section_y(s) + self.head + row as f32 * self.pitch.1 + self.inset.1;
        let x = self.inset.0 + col as f32 * self.pitch.0;
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
        let i = ((ly - self.footer_y()) / self.foot).floor();
        (i >= 0.0 && (i as usize) < self.footer.len()).then_some((usize::MAX, i as usize))
    }

    /// The command of a hit.
    pub fn choice(&self, (s, i): (usize, usize)) -> Option<String> {
        if s == usize::MAX {
            return self.footer.get(i).map(|(_, c, _)| c.clone());
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
        // Section captions: bold grey text on a grey band (measured 34 px, #EBEBEB, text
        // 12 px from the edge).
        let caption = TextStyle {
            size: (13.0 * self.k).round(),
            bold: true,
        };
        for (s, (title, tiles)) in self.sections.iter().enumerate() {
            let hy = y + self.d(self.section_y(s));
            p.fill_rect(x + 1.0, hy, w - 2.0, self.d(self.head), HEAD_BG);
            p.text(
                title,
                x + self.d(9.6),
                (hy + (self.d(self.head) - caption.size * 1.33) / 2.0).round(),
                caption,
                HEAD_INK,
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
                    // The reference's hot tile: a dark grey outline around a light grey ring.
                    p.fill_rect(r[0] - 4.0, r[1] - 4.0, r[2] + 8.0, r[3] + 8.0, HOT_EDGE);
                    p.fill_rect(r[0] - 3.0, r[1] - 3.0, r[2] + 6.0, r[3] + 6.0, HOT);
                }
                self.paint_tile(p, r, tile, style);
            }
        }
        let fy = y + self.d(self.footer_y());
        if !self.footer.is_empty() {
            p.fill_rect(x + 1.0, fy - 1.0, w - 2.0, 1.0, RULE);
        }
        for (i, (label, _, icon)) in self.footer.iter().enumerate() {
            let ry = fy + self.d(i as f32 * self.foot);
            if self.hover == Some((usize::MAX, i)) {
                p.fill_rect(x + 1.0, ry, w - 2.0, self.d(self.foot), HOVER);
            }
            paint_menu_icon(
                p,
                icon,
                x + self.d(4.8),
                ry + self.d(self.foot / 2.0),
                self.k,
            );
            // Menu lines a little larger than the gallery's other text (measured).
            let big = TextStyle {
                size: (13.0 * self.k).round(),
                bold: false,
            };
            p.text(
                label,
                x + self.d(36.0),
                (ry + (self.d(self.foot) - big.size * 1.33) / 2.0).round(),
                big,
                INK,
            );
        }
        // Size grip: dots in the bottom strip's middle.
        let gy = y + h - self.d(GRIP_H / 2.0);
        for d in 0..4 {
            p.fill_rect(
                (x + w / 2.0 + (d as f32 - 1.5) * self.d(4.0)).round(),
                gy.round(),
                self.d(1.6).max(1.0),
                self.d(1.6).max(1.0),
                GRIP,
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
                italic,
                size,
                family,
                edges,
            } => {
                if let Some(f) = fill {
                    p.fill_rect(r[0], r[1], r[2], r[3], *f);
                }
                // Edges inside the tile, as the reference draws them: thin 1 px, medium
                // 2 px, thick 3 px, double two 1 px lines 2 px apart.
                let [x, y, w, h] = r;
                for (side, edge) in edges.iter().enumerate() {
                    let Some((kind, c)) = edge else { continue };
                    let lines: &[(f32, f32)] = match kind {
                        Line::Thin => &[(0.0, 1.0)],
                        Line::Medium => &[(0.0, 2.0)],
                        Line::Thick => &[(0.0, 3.0)],
                        Line::Double => &[(0.0, 1.0), (3.0, 1.0)],
                    };
                    for &(off, t) in lines {
                        let (off, t) = (
                            (off * self.k / 1.25).round(),
                            (t * self.k / 1.25).round().max(1.0),
                        );
                        match side {
                            0 => p.fill_rect(x, y + off, w, t, *c),
                            1 => p.fill_rect(x + w - off - t, y, t, h, *c),
                            2 => p.fill_rect(x, y + h - off - t, w, t, *c),
                            _ => p.fill_rect(x + off, y, t, h, *c),
                        }
                    }
                }
                // The style's own typeface and size, shown about 20 % larger than on the
                // grid at 100 % (measured: 11 pt Calibri is 22 px high at 125 %).
                let px = (size * 4.0 / 3.0 * self.k * 1.2).round();
                let font = crate::Font {
                    family,
                    size: px,
                    weight: if *bold { 700 } else { 400 },
                    italic: *italic,
                    underline: false,
                };
                let _ = style;
                p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
                p.text_font(
                    name,
                    r[0] + self.d(5.6),
                    (r[1] + (r[3] - px * 1.22) / 2.0).round(),
                    &font,
                    *color,
                );
                p.pop_clip();
            }
            Tile::Table { name } => paint_table_tile(p, r, name),
        }
    }
}

/// A style's preview as the reference draws it: a table of five rows (header and four
/// banded rows) and five columns in the style's fills and lines, a dash in each cell in the
/// style's text colour (77 × 56 px tiles at 125 %).
fn paint_table_tile(p: &mut dyn Painter, r: [f32; 4], name: &str) {
    use bo_model::sheet::{CellRange, CellRef, Table, TableStyleInfo};
    let t = Table {
        range: CellRange::new(CellRef::new(0, 0), CellRef::new(4, 4)),
        header_row_count: None,
        style: Some(TableStyleInfo {
            name: Some(name.to_owned()),
            show_row_stripes: true,
            ..TableStyleInfo::default()
        }),
        ..Table::default()
    };
    let [x, y, w, h] = r;
    p.fill_rect(x, y, w, h, BG);
    let (cw, rh) = ((w - 1.0) / 5.0, (h - 1.0) / 5.0);
    let col_x = |c: u32| (x + c as f32 * cw).round();
    let row_y = |k: u32| (y + k as f32 * rh).round();
    let looks: Vec<Vec<bo_sheet::table_style::Look>> = (0..5)
        .map(|k| {
            (0..5)
                .map(|c| bo_sheet::table_style::look(&t, k, c).unwrap_or_default())
                .collect()
        })
        .collect();
    for (k, row) in looks.iter().enumerate() {
        for (c, l) in row.iter().enumerate() {
            let (x0, y0) = (col_x(c as u32), row_y(k as u32));
            let (x1, y1) = (col_x(c as u32 + 1), row_y(k as u32 + 1));
            if let Some(f) = l.fill.as_deref().and_then(hex) {
                p.fill_rect(x0, y0, x1 - x0 + 1.0, y1 - y0 + 1.0, f);
            }
        }
    }
    for (k, row) in looks.iter().enumerate() {
        for (c, l) in row.iter().enumerate() {
            let (x0, y0) = (col_x(c as u32), row_y(k as u32));
            let (x1, y1) = (col_x(c as u32 + 1), row_y(k as u32 + 1));
            for (side, e) in l.borders.iter().enumerate() {
                let Some(e) = e else { continue };
                let Some(color) = hex(&e.color) else { continue };
                let t = if e.width >= 2 || e.double { 2.0 } else { 1.0 };
                match side {
                    0 => p.fill_rect(x0, y0, x1 - x0 + 1.0, t, color),
                    1 => p.fill_rect(x0, y1 - t + 1.0, x1 - x0 + 1.0, t, color),
                    2 => p.fill_rect(x0, y0, t, y1 - y0 + 1.0, color),
                    _ => p.fill_rect(x1 - t + 1.0, y0, t, y1 - y0 + 1.0, color),
                }
            }
            // The cell's text as a dash in the middle.
            let ink = l.font.as_deref().and_then(hex).unwrap_or(INK);
            let dw = ((x1 - x0) * 0.45).round();
            p.fill_rect(
                (x0 + (x1 - x0 - dw) / 2.0).round(),
                ((y0 + y1) / 2.0).round(),
                dw,
                if l.bold { 2.0 } else { 1.0 },
                ink,
            );
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
const CELL_SECTIONS: [(&str, &str, &[&str]); 5] = [
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
    (
        "sheetGallery.numberFormat",
        "Number Format",
        &["Comma", "Comma [0]", "Currency", "Currency [0]", "Percent"],
    ),
];

impl SheetWindow {
    /// Open the Cell Styles gallery (`cells`) or the Format as Table gallery.
    /// The Format as Table gallery under Table Design's Quick Styles.
    pub(super) fn open_quick_styles(&mut self) -> bool {
        let Some(anchor) = self
            .ribbon
            .rect_of(RibbonHit::Command("sheet.tblQuickStyles"))
        else {
            return false;
        };
        let mut g = self.table_gallery();
        g.k = self.scale;
        let w = (g.width() * g.k).round();
        g.origin = (
            anchor[0].min(self.size.0 - w - 2.0).max(0.0).round(),
            (anchor[1] + anchor[3] + 2.0).round(),
        );
        self.gallery = Some(g);
        true
    }

    pub(super) fn open_gallery(&mut self, cells: bool, text: &dyn crate::TextMeasure) -> bool {
        let id = if cells {
            "sheet.cellStyles"
        } else {
            "sheet.formatTable"
        };
        // Table Design's Quick Styles opens the same gallery under its own button.
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)).or_else(|| {
            (!cells)
                .then(|| {
                    self.ribbon
                        .rect_of(RibbonHit::Command("sheet.tblQuickStyles"))
                })
                .flatten()
        }) else {
            return false;
        };
        let mut g = if cells {
            self.cell_style_gallery(text)
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

    fn cell_style_gallery(&self, text: &dyn crate::TextMeasure) -> Gallery {
        // Each built-in style's look, read from a scratch workbook styled with it; the
        // workbook's own (custom) styles from its stylesheet.
        let mut wb = bo_model::sheet::Workbook::new();
        let k = self.scale;
        let fit = |name: &str, size: f32, bold: bool, italic: bool, family: &str| -> String {
            // The reference ends names too long for a tile with "…" (Explanatory T…),
            // measured in the style's own font.
            let room = 122.4 * k - 14.0;
            let font = crate::Font {
                family,
                size: (size * 4.0 / 3.0 * k * 1.2).round(),
                weight: if bold { 700 } else { 400 },
                italic,
                underline: false,
            };
            let w = |s: &str| text.measure_font(s, &font).0;
            if w(name) <= room {
                return name.to_owned();
            }
            let mut out: String = name.to_owned();
            while !out.is_empty() && w(&format!("{out}…")) > room {
                out.pop();
            }
            format!("{}…", out.trim_end())
        };
        let tile_of =
            |styles: &bo_model::sheet::Stylesheet, xf: &bo_model::sheet::CellXf, name: &str| {
                let font = styles
                    .fonts
                    .get(xf.font_id as usize)
                    .or_else(|| styles.fonts.first())
                    .cloned()
                    .unwrap_or_default();
                let fill = match styles.fills.get(xf.fill_id as usize) {
                    Some(bo_model::sheet::Fill::Pattern(p))
                        if p.pattern.as_deref() == Some("solid") =>
                    {
                        p.fg.as_ref().and_then(|c| c.rgb.as_deref()).and_then(hex)
                    }
                    _ => None,
                };
                let b = styles
                    .borders
                    .get(xf.border_id as usize)
                    .cloned()
                    .unwrap_or_default();
                let edge = |e: &bo_model::sheet::BorderEdge| {
                    Line::of(e.style.as_deref()).map(|l| {
                        let c = e
                            .color
                            .as_ref()
                            .and_then(|c| c.rgb.as_deref())
                            .and_then(hex);
                        (l, c.unwrap_or(INK))
                    })
                };
                let size = font.size.unwrap_or(11.0) as f32;
                let family = font.name.clone().unwrap_or_else(|| "Calibri".into());
                Tile::Cell {
                    name: fit(name, size, font.bold, font.italic, &family),
                    fill,
                    color: font
                        .color
                        .as_ref()
                        .and_then(|c| c.rgb.as_deref())
                        .and_then(hex)
                        .unwrap_or(INK),
                    bold: font.bold,
                    italic: font.italic,
                    size,
                    family,
                    edges: [edge(&b.top), edge(&b.right), edge(&b.bottom), edge(&b.left)],
                }
            };
        let mut sections = Vec::new();
        if let Some(own) = self.view.engine().workbook() {
            let tiles: Vec<_> = bo_sheet::style::custom_styles(own)
                .into_iter()
                .filter_map(|name| {
                    let s = own.styles.cell_styles.iter().find(|s| s.name == name)?;
                    let xf = own.styles.cell_style_xfs.get(s.xf_id as usize)?;
                    Some((format!("style.{name}"), tile_of(&own.styles, xf, &name)))
                })
                .collect();
            if !tiles.is_empty() {
                sections.push((self.t("sheetGallery.custom", "Custom"), tiles));
            }
        }
        let mut n = 0;
        for (key, fallback, names) in CELL_SECTIONS {
            let mut tiles = Vec::new();
            for name in names {
                let at = bo_model::sheet::CellRef::new(0, n);
                n += 1;
                let range = bo_model::sheet::CellRange::new(at, at);
                bo_sheet::style::apply_named_style(&mut wb, 0, range, name);
                let style = wb.sheets[0].cell(0, at.col).map_or(0, |c| c.style);
                let xf = wb.styles.xf(style).cloned().unwrap_or_default();
                tiles.push((format!("style.{name}"), tile_of(&wb.styles, &xf, name)));
            }
            sections.push((self.t(key, fallback), tiles));
        }
        Gallery {
            sections,
            footer: vec![
                (
                    self.t("sheetGallery.newCellStyle", "New Cell Style..."),
                    "sheet.newCellStyle".into(),
                    "new",
                ),
                (
                    self.t("sheetGallery.mergeStylesCmd", "Merge Styles..."),
                    "sheet.mergeStyles".into(),
                    "merge",
                ),
            ],
            // Measured at 125 %: 153 × 34 px tiles, 161 × 42 px apart, six to a row, first
            // 5 px from the edge and 4 px under its row's top; captions 34 px; menu lines
            // 36 px; 991 px wide.
            tile: (122.4, 27.2),
            cols: 6,
            pitch: (128.8, 33.6),
            inset: (4.0, 3.2),
            head: 27.2,
            foot: 28.8,
            width: 792.8,
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
                    (
                        format!("table.{}", e.name),
                        Tile::Table {
                            name: e.name.clone(),
                        },
                    )
                })
                .collect();
            sections.push((self.t(key, fallback), tiles));
        }
        Gallery {
            sections,
            // The reference's menu lines under the styles (new styles are not made yet: the
            // status bar says so).
            footer: vec![
                (
                    self.t("sheetGallery.newTableStyle", "New Table Style..."),
                    "sheet.newTableStyle".into(),
                    "new",
                ),
                (
                    self.t("sheetGallery.newPivotStyle", "New PivotTable Style..."),
                    "sheet.newPivotStyle".into(),
                    "merge",
                ),
            ],
            // Measured at 125 % (round 27): 77 × 56 px tiles, 86 × 66 px apart, 5 px from
            // the edge, 34 px captions, 625 px wide.
            tile: (61.6, 44.8),
            cols: 7,
            pitch: (68.8, 52.8),
            inset: (4.0, 3.2),
            head: 27.2,
            foot: 28.8,
            width: 500.0,
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

    /// The gallery's menu lines: New Cell Style…, Merge Styles…; `None` for other ids.
    pub(super) fn gallery_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "sheet.newCellStyle" => self.open_cell_style_form(),
            "sheet.newTableStyle" | "sheet.newPivotStyle" => {
                self.status = self.t(
                    "sheetGallery.newStyleLater",
                    "Custom table styles are not available yet.",
                );
                true
            }
            "sheet.mergeStyles" => {
                self.pick = Some("styles");
                self.requests.push(crate::Request::PickMergeData);
                true
            }
            _ => return None,
        })
    }

    /// File types for the host's file dialog of a `Request::PickMergeData` (description,
    /// patterns).
    pub fn pick_filters(&self) -> Vec<(String, String)> {
        match self.pick {
            Some("theme") => vec![(
                self.t("sheetTheme.themeFiles", "Theme files (*.thmx)"),
                "*.thmx".into(),
            )],
            Some("text") => vec![(
                self.t("sheetData.textFiles", "Text Files (*.prn;*.txt;*.csv)"),
                "*.prn;*.txt;*.csv".into(),
            )],
            _ => vec![(
                self.t("sheetGallery.workbooks", "Workbooks (*.xlsx;*.xlsm)"),
                "*.xlsx;*.xlsm".into(),
            )],
        }
    }

    /// The host read the file picked for a `Request::PickMergeData` (`name` is its file name).
    pub fn picked_file(&mut self, bytes: &[u8], name: &str) -> bool {
        use base64::Engine as _;
        match self.pick.take() {
            Some("text") => self.import_text(bytes, name),
            Some("theme") => self.picked_theme(bytes),
            Some("styles") => {
                let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                let merged = self
                    .view
                    .query(json!({ "cmd": "mergeStyles", "data": data }))
                    .and_then(|v| v.get("merged").and_then(serde_json::Value::as_u64));
                self.view.changed();
                self.status = if let Some(n) = merged {
                    self.t("sheetGallery.merged", "{n} styles merged")
                        .replace("{n}", &n.to_string())
                } else {
                    let _ = self.view.take_error();
                    self.t("sheetGallery.notWorkbook", "The file is not a workbook.")
                };
                true
            }
            _ => false,
        }
    }

    /// The Style dialog of New Cell Style… (measured 404 × 459 px): name, Format…, and
    /// what the style includes, described from the active cell's format.
    fn open_cell_style_form(&mut self) -> bool {
        let (r, c) = self.view.active();
        let Some(st) = self.view.engine().workbook().map(|wb| {
            bo_sheet::style::format_state(
                wb,
                self.view.sheet(),
                r,
                c,
                &bo_sheet::numfmt::Locale::EN_US,
            )
        }) else {
            return false;
        };
        let t = |k: &str, d: &str| self.t(k, d);
        let taken = |n: &str| {
            self.view.engine().workbook().is_some_and(|wb| {
                wb.styles
                    .cell_styles
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case(n))
            })
        };
        let base = t("sheetGallery.styleN", "Style");
        let name = (1..10_000)
            .map(|i| format!("{base} {i}"))
            .find(|n| !taken(n))
            .unwrap_or_default();
        let mut f = Form::new(
            &t("sheetGallery.styleTitle", "Style"),
            (404.0, 459.0),
            vec![],
        );
        f.add(
            "nameLabel",
            None,
            [18.0, 52.0, 0.0, 0.0],
            Kind::Label(t("sheetGallery.styleName", "Style name:")),
        );
        let caret = name.chars().count();
        f.add(
            "name",
            None,
            [98.0, 47.0, 287.0, 25.0],
            Kind::Edit {
                value: name.chars().collect(),
                caret,
            },
        );
        f.add(
            "format",
            None,
            [273.0, 79.0, 112.0, 25.0],
            Kind::Button {
                label: t("sheetGallery.format", "Format..."),
                default: false,
            },
        );
        f.enable("format", false);
        f.add(
            "includes",
            None,
            [18.0, 122.0, 368.0, 16.0],
            Kind::Group(t("sheetGallery.includes", "Style includes")),
        );
        let general = t("sheetGallery.general", "General");
        let number = if st.num_fmt.is_empty() || st.num_fmt == "General" {
            general.clone()
        } else {
            st.num_fmt.clone()
        };
        let cap = |s: &str| {
            let mut c = s.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };
        let h = if st.h_align.is_empty() {
            general.clone()
        } else {
            cap(&st.h_align)
        };
        let v = if st.v_align.is_empty() {
            "Bottom".to_owned()
        } else {
            cap(&st.v_align)
        };
        let edges: Vec<&str> = ["Top", "Right", "Bottom", "Left"]
            .iter()
            .zip(&st.borders)
            .filter(|(_, b)| !b.is_empty())
            .map(|(n, _)| *n)
            .collect();
        let rows = [
            ("incNumber", t("sheetGallery.number", "Number"), number),
            (
                "incAlign",
                t("sheetGallery.alignment", "Alignment"),
                format!("{h}, {v} {}", t("sheetGallery.aligned", "Aligned")),
            ),
            (
                "incFont",
                t("sheetGallery.font", "Font"),
                format!("{} {}", st.font_name, st.font_size),
            ),
            (
                "incBorder",
                t("sheetGallery.border", "Border"),
                if edges.is_empty() {
                    t("sheetGallery.noBorders", "No Borders")
                } else {
                    format!(
                        "{} {}",
                        edges.join(", "),
                        t("sheetGallery.borders", "Borders")
                    )
                },
            ),
            (
                "incFill",
                t("sheetGallery.fill", "Fill"),
                if st.fill_color.is_empty() {
                    t("sheetGallery.noShading", "No Shading")
                } else {
                    t("sheetGallery.shaded", "Shaded")
                },
            ),
            (
                "incProtect",
                t("sheetGallery.protection", "Protection"),
                if st.locked {
                    t("sheetGallery.locked", "Locked")
                } else {
                    t("sheetGallery.unlocked", "Unlocked")
                },
            ),
        ];
        for (i, (id, label, desc)) in rows.into_iter().enumerate() {
            f.add(
                id,
                None,
                [33.0, 147.0 + 41.0 * i as f32, 340.0, 20.0],
                Kind::Check {
                    label: format!("{label}: {desc}"),
                    on: true,
                },
            );
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("name");
        self.form = Some(f);
        self.form_kind = FormKind::CellStyle;
        true
    }

    pub(super) fn cell_style_event(&mut self, ev: &Event) {
        match ev {
            Event::Cancel => self.form = None,
            Event::Ok => {
                let name = self
                    .form
                    .as_ref()
                    .map(|f| f.text("name"))
                    .unwrap_or_default();
                let (r, c) = self.view.active();
                if self.view.run(json!({
                    "cmd": "newCellStyle", "row": r, "col": c, "name": name.trim(),
                })) {
                    self.form = None;
                } else {
                    let _ = self.view.take_error();
                    self.status = self.t(
                        "sheetGallery.nameTaken",
                        "A style with this name already exists.",
                    );
                }
            }
            _ => {}
        }
    }

    /// Format as Table from the gallery with the selection's region (tests).
    #[cfg(test)]
    pub(super) fn gallery_choice_at(&self, s: usize, i: usize) -> Option<String> {
        self.gallery.as_ref()?.choice((s, i))
    }
}
