//! The look of tables (ECMA-376 Part 1, §18.5.1.5 `tableStyleInfo`: style name, banded rows /
//! columns, first / last column).
//!
//! The built-in style names (`TableStyleLight1`, `TableStyleMedium2`, … §18.8.40) are kept in the
//! file; BlueSheet draws them with its own palette ([`crate::pivot::style::palette`]): the family
//! sets how strong the header and totals fills are, the number picks one of seven BlueSheet
//! accent colours. Unlike pivot tables the look is not written into the cells: the renderer
//! overlays it ([`overlay`]), under conditional formatting and the cells' own fills; converting a
//! table to a range writes it as direct formatting ([`bake`]).

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::ops::Range;

use bo_model::sheet::{
    BorderEdge, CellRange, CellRef, Dxf, SheetColor, Table, TableStyleParts, ThemeColors, Workbook,
};
use bo_render::Rgba;
use serde::Serialize;

use crate::cf::CellFormat;
use crate::color::{cell_fill_rgb, parse_hex};
use crate::pivot::style::palette;
use crate::style::{apply_patch, StylePatch};
use crate::table::{has_header, has_totals};

/// A border line of a table style: colour `RRGGBB`, width (1 thin, 2 medium, 3 thick), double.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Edge {
    /// Colour `RRGGBB`.
    pub color: String,
    /// 1 thin, 2 medium, 3 thick.
    pub width: u8,
    /// Two thin lines (§18.18.3 `double`).
    pub double: bool,
}

/// The look of one table cell: fill and font colours `RRGGBB`, bold, borders.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Look {
    /// Fill; `None` = none.
    pub fill: Option<String>,
    /// Font colour; `None` = automatic.
    pub font: Option<String>,
    /// Bold text.
    pub bold: bool,
    /// Top, bottom, left, right borders.
    pub borders: [Option<Edge>; 4],
}

/// Measured looks of the built-in styles (`table_styles.txt`): (style, sample) → look.
fn measured() -> &'static HashMap<(String, String), Look> {
    static DATA: std::sync::OnceLock<HashMap<(String, String), Look>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let mut out = HashMap::new();
        for line in include_str!("table_styles.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() != 9 {
                continue;
            }
            let hex = |s: &str| (s != "-").then(|| s.to_owned());
            let edge = |s: &str| -> Option<Edge> {
                let mut it = s.split('/');
                let color = it.next().filter(|c| *c != "-")?.to_owned();
                let width = it.next().and_then(|w| w.parse().ok()).unwrap_or(1);
                let double = it.next() == Some("d");
                Some(Edge {
                    color,
                    width,
                    double,
                })
            };
            out.insert(
                (f[0].to_owned(), f[1].to_owned()),
                Look {
                    fill: hex(f[2]),
                    font: hex(f[3]),
                    bold: f[4] == "1",
                    borders: [edge(f[5]), edge(f[6]), edge(f[7]), edge(f[8])],
                },
            );
        }
        out
    })
}

/// Whether `name` is one of the built-in styles drawn as the reference does.
#[must_use]
pub fn is_builtin(name: &str) -> bool {
    measured().contains_key(&(builtin_key(name).to_owned(), "B2".to_owned()))
}

fn builtin_key(name: &str) -> &str {
    name.strip_prefix("TableStyle").unwrap_or(name)
}

/// Look of a built-in style at (`row`, `col`) of `t`, from the measured samples: the row's kind
/// (header, totals, last data row, first / second band) and the column's place (first, inner,
/// last), with the first / last column and column band options.
fn builtin_look(t: &Table, name: &str, row: u32, col: u32) -> Option<Look> {
    let data = measured();
    let key = builtin_key(name);
    let get = |s: &str| data.get(&(key.to_owned(), s.to_owned()));
    get("B2")?;
    let info = t.style.as_ref()?;
    let header = has_header(t) && row == t.range.first.row;
    let totals = has_totals(t) && row == t.range.last.row;
    let first = col == t.range.first.col;
    let last = col == t.range.last.col && !first;
    let last_data = has_totals(t)
        && !header
        && !totals
        && row + 1 == t.range.last.row
        && row > t.range.first.row + u32::from(has_header(t));
    let i = row.saturating_sub(t.range.first.row + u32::from(has_header(t)));
    let band2 = info.show_row_stripes && !i.is_multiple_of(2);
    // Row sample (inner column) and its first / last column samples.
    let (mid, a, d) = if header {
        ("B1", "A1", "D1")
    } else if totals {
        ("B6", "A6", "D6")
    } else if last_data && band2 {
        ("B5", "A2", "D2")
    } else if band2 || !info.show_row_stripes {
        ("B3", "A2", "D2")
    } else {
        ("B2", "A2", "D2")
    };
    let mut l = get(mid)?.clone();
    if !header && !totals {
        // Lines between data rows: the samples next to the header (B2) and to the totals
        // (B5) carry those rows' lines; inside the body the line between a first and a
        // second band is B2's bottom, between a second and a first band B3's bottom.
        let b2 = get("B2").map(|s| s.borders.clone()).unwrap_or_default();
        let b3 = get("B3").map(|s| s.borders.clone()).unwrap_or_default();
        let first_row = i == 0;
        l.borders[0] = if first_row {
            b2[0].clone()
        } else if !i.is_multiple_of(2) {
            b2[1].clone()
        } else {
            b3[1].clone()
        };
        if !last_data {
            l.borders[1] = if i.is_multiple_of(2) {
                b2[1].clone()
            } else {
                b3[1].clone()
            };
        }
        // The last row of a table without totals closes it with the header's top line.
        if row == t.range.last.row {
            l.borders[1] = get("B1")
                .and_then(|h| h.borders[0].clone())
                .or(l.borders[1].clone());
        }
    }
    if !info.show_row_stripes && !header && !totals {
        // No row bands: the second band's look (no band fill) for every data row.
        l.fill = get("B3").and_then(|b| b.fill.clone());
    }
    if info.show_column_stripes && !header && !totals {
        let k = col - t.range.first.col;
        let s = if k.is_multiple_of(2) {
            "bc_C2"
        } else {
            "bc_B2"
        };
        if let Some(b) = get(s) {
            // Row bands win over column bands.
            if !info.show_row_stripes || band2 || l.fill.is_none() {
                l.fill = b.fill.clone().or(l.fill);
            }
        }
    }
    // Outer edges come from the first / last column samples.
    let side = |s: &str, k: usize| get(s).and_then(|x| x.borders[k].clone());
    if first {
        l.borders[2] = side(a, 2);
    }
    if last || (first && t.range.first.col == t.range.last.col) {
        l.borders[3] = side(d, 3);
    }
    let edge_opt = (first && info.show_first_column) || (last && info.show_last_column);
    if edge_opt {
        let s = match (header, totals, first, band2 || !info.show_row_stripes) {
            (true, _, true, _) => "fl_A1",
            (true, _, false, _) => "fl_D1",
            (_, true, true, _) => "fl_A6",
            (_, true, false, _) => "fl_D6",
            (_, _, true, false) => "fl_A2",
            (_, _, true, true) => "fl_A3",
            (_, _, false, false) => "fl_D2",
            (_, _, false, true) => "fl_D3",
        };
        if let Some(e) = get(s) {
            l.fill = e.fill.clone();
            l.font = e.font.clone();
            l.bold = e.bold;
            for (mine, theirs) in l.borders.iter_mut().zip(&e.borders) {
                if theirs.is_some() {
                    mine.clone_from(theirs);
                }
            }
        }
    }
    // The font colour shown is the automatic one when it is black on a light fill.
    if l.font.as_deref() == Some("000000") {
        l.font = None;
    }
    Some(l)
}

/// The look of (`row`, `col`) inside table `t`; `None` outside or without style.
#[must_use]
pub fn look(t: &Table, row: u32, col: u32) -> Option<Look> {
    if !t.range.contains(CellRef::new(row, col)) {
        return None;
    }
    let info = t.style.as_ref()?;
    let name = info.name.as_deref()?;
    if let Some(l) = builtin_look(t, name, row, col) {
        return (l != Look::default()).then_some(l);
    }
    let p = palette(name);
    let header = has_header(t) && row == t.range.first.row;
    let totals = has_totals(t) && row == t.range.last.row;
    let k = col - t.range.first.col;
    let edge_bold =
        (info.show_first_column && k == 0) || (info.show_last_column && col == t.range.last.col);
    let l = if header {
        Look {
            fill: p.header_fill,
            font: p.header_font,
            bold: true,
            ..Look::default()
        }
    } else if totals {
        Look {
            fill: p.grand_fill,
            font: p.grand_font,
            bold: true,
            ..Look::default()
        }
    } else {
        let i = row - t.range.first.row - u32::from(has_header(t));
        let striped = (info.show_row_stripes && i.is_multiple_of(2))
            || (info.show_column_stripes && k.is_multiple_of(2));
        Look {
            fill: striped.then_some(p.stripe_fill),
            font: None,
            bold: edge_bold,
            ..Look::default()
        }
    };
    (l != Look::default()).then_some(l)
}

fn rgba(hex: &str) -> Option<Rgba> {
    parse_hex(hex).map(|(r, g, b)| Rgba(r, g, b, 255))
}

/// Theme the styles were measured with (the reference's default theme), in `clrScheme` order:
/// dk1, lt1, dk2, lt2, accent1–6.
const MEASURED_THEME: [&str; 10] = [
    "000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5",
    "70AD47",
];

/// The theme colour and tint each measured colour is with the measured theme (the reference
/// defines its table styles on theme colours, §18.8.40): `RRGGBB` → (theme index, tint).
fn theme_slots() -> &'static HashMap<String, (u32, f64)> {
    static DATA: std::sync::OnceLock<HashMap<String, (u32, f64)>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let mut colours: Vec<&str> = Vec::new();
        for l in measured().values() {
            colours.extend(l.fill.as_deref());
            colours.extend(l.font.as_deref());
            colours.extend(l.borders.iter().flatten().map(|e| e.color.as_str()));
        }
        // Untinted first, then the smallest tint, as a style would use them.
        let mut tints: Vec<f64> = (1..=19).map(|i| f64::from(i) * 0.05).collect();
        tints.extend(tints.clone().iter().map(|t| -t));
        tints.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
        tints.insert(0, 0.0);
        let mut out = HashMap::new();
        for hex in colours {
            if out.contains_key(hex) {
                continue;
            }
            let Some(want) = parse_hex(hex) else { continue };
            let measured_theme = ThemeColors {
                colors: MEASURED_THEME.iter().map(|s| (*s).to_owned()).collect(),
            };
            // Accents, then dark 2 / light 2, then text / background.
            // Within three steps per channel: the reference rounds tints its own way.
            let near = |a: (u8, u8, u8)| {
                a.0.abs_diff(want.0) <= 3 && a.1.abs_diff(want.1) <= 3 && a.2.abs_diff(want.2) <= 3
            };
            let found = tints.iter().find_map(|&tint| {
                [4, 5, 6, 7, 8, 9, 3, 2, 1, 0].into_iter().find_map(|i| {
                    near(crate::color::apply_tint(
                        crate::color::theme_rgb(i, Some(&measured_theme)),
                        tint,
                    ))
                    .then_some((i, tint))
                })
            });
            if let Some(slot) = found {
                out.insert(hex.to_owned(), slot);
            }
        }
        out
    })
}

/// `l` with its colours taken from `theme` instead of the default theme (a workbook whose
/// theme changed restyles its tables, as the reference). Colours that are not theme colours
/// stay.
#[must_use]
pub fn themed(mut l: Look, theme: Option<&ThemeColors>) -> Look {
    let Some(theme) = theme else {
        return l;
    };
    let slots = theme_slots();
    let map = |hex: &mut String| {
        if let Some(&(i, tint)) = slots.get(hex.as_str()) {
            let (r, g, b) = crate::color::apply_tint(crate::color::theme_rgb(i, Some(theme)), tint);
            *hex = format!("{r:02X}{g:02X}{b:02X}");
        }
    };
    if let Some(f) = l.fill.as_mut() {
        map(f);
    }
    if let Some(f) = l.font.as_mut() {
        map(f);
    }
    for e in l.borders.iter_mut().flatten() {
        map(&mut e.color);
    }
    l
}

/// The elements of a custom table style in the reference's list order (§18.18.83
/// ST_TableStyleType, table styles only).
pub const TABLE_ELEMENTS: [&str; 13] = [
    "wholeTable",
    "firstColumnStripe",
    "secondColumnStripe",
    "firstRowStripe",
    "secondRowStripe",
    "lastColumn",
    "firstColumn",
    "headerRow",
    "totalRow",
    "firstHeaderCell",
    "lastHeaderCell",
    "firstTotalCell",
    "lastTotalCell",
];

fn hex_color(s: Option<&str>) -> Option<SheetColor> {
    let h = s?.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| SheetColor::rgb(&h.to_ascii_uppercase()))
}

/// A differential format (§18.8.14 `dxf`) of a table style element: fill, text colour (hex
/// `RRGGBB`, `#` optional), bold / italic, a thin border of a colour around the element and
/// between its cells. `None` when it sets nothing.
#[must_use]
pub fn element_dxf(
    fill: Option<&str>,
    text: Option<&str>,
    bold: bool,
    italic: bool,
    border: Option<&str>,
) -> Option<Dxf> {
    use bo_model::sheet::{Border, Fill, Font, PatternFill};
    let text = hex_color(text);
    let font = (text.is_some() || bold || italic).then(|| Font {
        bold,
        italic,
        color: text,
        ..Font::default()
    });
    // §18.8.32 patternFill: a solid differential fill keeps its colour in bgColor.
    let fill = hex_color(fill).map(|c| {
        Fill::Pattern(PatternFill {
            pattern: Some("solid".into()),
            bg: Some(c),
            ..PatternFill::default()
        })
    });
    let border = hex_color(border).map(|c| {
        let edge = BorderEdge {
            style: Some("thin".into()),
            color: Some(c),
        };
        Border {
            left: edge.clone(),
            right: edge.clone(),
            top: edge.clone(),
            bottom: edge.clone(),
            vertical: Some(edge.clone()),
            horizontal: Some(edge),
            ..Border::default()
        }
    });
    (font.is_some() || fill.is_some() || border.is_some()).then(|| Dxf {
        font,
        fill,
        border,
        ..Dxf::default()
    })
}

fn color_hex(c: &SheetColor, theme: Option<&ThemeColors>) -> Option<String> {
    crate::color::resolve_color(c, theme).map(|x| crate::color::hex((x.0, x.1, x.2)))
}

fn edge_of(e: &BorderEdge, theme: Option<&ThemeColors>) -> Option<Edge> {
    let style = e.style.as_deref().filter(|s| *s != "none")?;
    let color = e
        .color
        .as_ref()
        .and_then(|c| color_hex(c, theme))
        .unwrap_or_else(|| "000000".into());
    Some(Edge {
        color,
        width: match style {
            "medium" | "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" => 2,
            "thick" => 3,
            _ => 1,
        },
        double: style == "double",
    })
}

/// Look of (`row`, `col`) of `t` drawn in custom style `style` (§18.8.40): the elements that
/// apply to the cell from the lowest to the highest precedence (whole table, column stripes,
/// row stripes, last / first column, header / total row, the corner cells), each one's fill,
/// text and borders over the ones below. Borders: an element's outer edges on the edges of the
/// area it covers, its inner lines (`horizontal` / `vertical`) inside it.
#[must_use]
pub fn custom_look(
    style: &TableStyleParts,
    t: &Table,
    row: u32,
    col: u32,
    theme: Option<&ThemeColors>,
) -> Option<Look> {
    if !t.range.contains(CellRef::new(row, col)) {
        return None;
    }
    let info = t.style.clone().unwrap_or_default();
    let r = t.range;
    let header = has_header(t);
    let totals = has_totals(t);
    let is_header = header && row == r.first.row;
    let is_total = totals && row == r.last.row;
    let d0 = r.first.row + u32::from(header);
    let d1 = r.last.row.saturating_sub(u32::from(totals));
    let data = !is_header && !is_total && row >= d0 && row <= d1;
    let el = |kind: &str| style.elements.iter().find(|e| e.kind == kind);
    let size = |kind: &str| el(kind).map_or(1, |e| e.size.max(1));
    // (element, area: first row, last row, first col, last col).
    let mut apply: Vec<(&str, [u32; 4])> = vec![(
        "wholeTable",
        [r.first.row, r.last.row, r.first.col, r.last.col],
    )];
    if data && info.show_column_stripes {
        let (a, b) = (size("firstColumnStripe"), size("secondColumnStripe"));
        let at = (col - r.first.col) % (a + b);
        let start = col - at;
        let (kind, c0, c1) = if at < a {
            ("firstColumnStripe", start, start + a - 1)
        } else {
            ("secondColumnStripe", start + a, start + a + b - 1)
        };
        apply.push((kind, [d0, d1, c0, c1.min(r.last.col)]));
    }
    if data && info.show_row_stripes {
        let (a, b) = (size("firstRowStripe"), size("secondRowStripe"));
        let at = (row - d0) % (a + b);
        let start = row - at;
        let (kind, r0, r1) = if at < a {
            ("firstRowStripe", start, start + a - 1)
        } else {
            ("secondRowStripe", start + a, start + a + b - 1)
        };
        apply.push((kind, [r0, r1.min(d1), r.first.col, r.last.col]));
    }
    let (first, last) = (col == r.first.col, col == r.last.col);
    if info.show_last_column && last {
        apply.push(("lastColumn", [r.first.row, r.last.row, col, col]));
    }
    if info.show_first_column && first {
        apply.push(("firstColumn", [r.first.row, r.last.row, col, col]));
    }
    for (on, kind, corner_first, corner_last) in [
        (is_header, "headerRow", "firstHeaderCell", "lastHeaderCell"),
        (is_total, "totalRow", "firstTotalCell", "lastTotalCell"),
    ] {
        if !on {
            continue;
        }
        apply.push((kind, [row, row, r.first.col, r.last.col]));
        if info.show_first_column && first {
            apply.push((corner_first, [row, row, col, col]));
        }
        if info.show_last_column && last {
            apply.push((corner_last, [row, row, col, col]));
        }
    }
    let mut l = Look::default();
    for (kind, [r0, r1, c0, c1]) in apply {
        let Some(d) = el(kind).and_then(|e| e.dxf.as_ref()) else {
            continue;
        };
        if let Some(f) = &d.fill {
            let c = match f {
                bo_model::sheet::Fill::Pattern(p) => p.bg.as_ref().or(p.fg.as_ref()),
                bo_model::sheet::Fill::Gradient(g) => g.stops.first().map(|s| &s.1),
            };
            if let Some(h) = c.and_then(|c| color_hex(c, theme)) {
                l.fill = Some(h);
            }
        }
        if let Some(f) = &d.font {
            if let Some(h) = f.color.as_ref().and_then(|c| color_hex(c, theme)) {
                l.font = Some(h);
            }
            l.bold |= f.bold;
        }
        if let Some(b) = &d.border {
            let (inner_h, inner_v) = (b.horizontal.as_ref(), b.vertical.as_ref());
            let sides = [
                if row == r0 { Some(&b.top) } else { inner_h },
                if row == r1 { Some(&b.bottom) } else { inner_h },
                if col == c0 { Some(&b.left) } else { inner_v },
                if col == c1 { Some(&b.right) } else { inner_v },
            ];
            for (mine, e) in l.borders.iter_mut().zip(sides) {
                if let Some(e) = e.and_then(|e| edge_of(e, theme)) {
                    *mine = Some(e);
                }
            }
        }
    }
    (l != Look::default()).then_some(l)
}

/// The look of (`row`, `col`) of `t` in workbook `wb`: its custom style when the workbook
/// defines one of that name, else the built-in style in the workbook's theme.
#[must_use]
pub fn look_in(wb: &Workbook, t: &Table, row: u32, col: u32) -> Option<Look> {
    let name = t.style.as_ref()?.name.as_deref()?;
    if let Some(s) = wb
        .styles
        .table_styles
        .iter()
        .find(|s| s.name.to_lowercase() == name.to_lowercase())
    {
        return custom_look(s, t, row, col, wb.theme.as_ref());
    }
    look(t, row, col).map(|l| themed(l, wb.theme.as_ref()))
}

/// A font colour that is the default one (automatic, text theme colour, black).
fn automatic(c: Option<&SheetColor>) -> bool {
    c.is_none_or(|c| {
        c.auto
            || c.theme == Some(1)
            || c.indexed == Some(64)
            || c.rgb
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("FF000000"))
    })
}

/// Add the look of the tables of `sheet` in `rows` × `cols` to the conditional formats of the
/// pane (conditional formatting wins; a cell's own fill and font colour win over the table's).
pub fn overlay<S: BuildHasher>(
    wb: &Workbook,
    sheet: usize,
    rows: Range<u32>,
    cols: Range<u32>,
    cf: &mut HashMap<(u32, u32), CellFormat, S>,
) {
    let Some(ws) = wb.sheets.get(sheet) else {
        return;
    };
    for t in &ws.tables {
        let r0 = t.range.first.row.max(rows.start);
        let r1 = t.range.last.row.min(rows.end.saturating_sub(1));
        let c0 = t.range.first.col.max(cols.start);
        let c1 = t.range.last.col.min(cols.end.saturating_sub(1));
        if r0 > r1 || c0 > c1 || rows.is_empty() || cols.is_empty() {
            continue;
        }
        for r in r0..=r1 {
            for c in c0..=c1 {
                let Some(l) = look_in(wb, t, r, c) else {
                    continue;
                };
                let cell = ws.cells.get(&(r, c));
                let own_fill = cell.is_some_and(|x| cell_fill_rgb(wb, x).is_some());
                let own_font = cell.is_some_and(|x| {
                    let font = wb
                        .styles
                        .xf(x.style)
                        .and_then(|xf| wb.styles.fonts.get(xf.font_id as usize));
                    !automatic(font.and_then(|f| f.color.as_ref()))
                });
                let e = cf.entry((r, c)).or_default();
                if !own_fill && e.fill.is_none() {
                    e.fill = l.fill.as_deref().and_then(rgba);
                }
                if !own_font && e.font_color.is_none() {
                    e.font_color = l.font.as_deref().and_then(rgba);
                }
                if l.bold && e.bold.is_none() {
                    e.bold = Some(true);
                }
            }
        }
    }
}

/// Write the look of `t` (on `sheet`) into its cells as direct formatting; cells with their own
/// fill keep it.
pub fn bake(wb: &mut Workbook, sheet: usize, t: &Table) {
    for r in t.range.first.row..=t.range.last.row {
        let mut runs: Vec<(u32, u32, StylePatch)> = Vec::new();
        for c in t.range.first.col..=t.range.last.col {
            let Some(l) = look_in(wb, t, r, c) else {
                continue;
            };
            let own_fill = wb
                .sheets
                .get(sheet)
                .and_then(|ws| ws.cells.get(&(r, c)))
                .is_some_and(|x| cell_fill_rgb(wb, x).is_some());
            let patch = StylePatch {
                fill_color: if own_fill { None } else { l.fill },
                font_color: l.font,
                bold: l.bold.then_some(true),
                ..StylePatch::default()
            };
            match runs.last_mut() {
                Some((_, end, p)) if *end + 1 == c && *p == patch => *end = c,
                _ => runs.push((c, c, patch)),
            }
        }
        for (a, b, p) in runs {
            if p != StylePatch::default() {
                apply_patch(
                    wb,
                    sheet,
                    CellRange::new(CellRef::new(r, a), CellRef::new(r, b)),
                    &p,
                );
            }
        }
    }
}

/// One style of the gallery with its preview colours (`RRGGBB`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryEntry {
    /// Style name written to the file (`TableStyleMedium2`).
    pub name: String,
    /// `light`, `medium` or `dark`.
    pub family: &'static str,
    /// Accent colour number 1–7 (the UI names it).
    pub accent: usize,
    /// Header fill.
    pub header: Option<String>,
    /// Header font colour.
    pub header_font: Option<String>,
    /// Banded row fill.
    pub stripe: String,
    /// Totals row fill.
    pub totals: Option<String>,
}

/// The styles offered by the table gallery, as the reference's: Light 1–21, Medium 1–28,
/// Dark 1–11 (the accent number cycles through none / Accent 1–6).
#[must_use]
pub fn gallery() -> Vec<GalleryEntry> {
    let mut out = Vec::new();
    for (family, key, count) in [
        ("Light", "light", 21),
        ("Medium", "medium", 28),
        ("Dark", "dark", 11),
    ] {
        for n in 1..=count {
            let name = format!("TableStyle{family}{n}");
            let get = |s: &str| measured().get(&(format!("{family}{n}"), s.to_owned()));
            let fill = |s: &str| get(s).and_then(|l| l.fill.clone());
            out.push(GalleryEntry {
                family: key,
                accent: (n - 1) % 7 + 1,
                header: fill("B1"),
                header_font: get("B1").and_then(|l| l.font.clone()),
                stripe: fill("B2").unwrap_or_default(),
                totals: fill("B6"),
                name,
            });
        }
    }
    out
}

/// Borders of the table cells of `sheet` in `rows` × `cols`: (row, col, top / bottom / left /
/// right) for cells whose table style draws any.
#[must_use]
pub fn borders(
    wb: &Workbook,
    sheet: usize,
    rows: Range<u32>,
    cols: Range<u32>,
) -> Vec<(u32, u32, [Option<Edge>; 4])> {
    let mut out = Vec::new();
    let Some(ws) = wb.sheets.get(sheet) else {
        return out;
    };
    for t in &ws.tables {
        let r0 = t.range.first.row.max(rows.start);
        let r1 = t.range.last.row.min(rows.end.saturating_sub(1));
        let c0 = t.range.first.col.max(cols.start);
        let c1 = t.range.last.col.min(cols.end.saturating_sub(1));
        if r0 > r1 || c0 > c1 || rows.is_empty() || cols.is_empty() {
            continue;
        }
        for r in r0..=r1 {
            for c in c0..=c1 {
                if let Some(l) = look_in(wb, t, r, c) {
                    if l.borders.iter().any(Option::is_some) {
                        out.push((r, c, l.borders));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{TableColumn, TableStyleInfo};

    fn table(style: &str) -> Table {
        Table {
            range: CellRange::parse("B2:D6").unwrap(),
            totals_row_count: Some(1),
            columns: (1..=3)
                .map(|i| TableColumn {
                    id: i,
                    name: format!("c{i}"),
                    ..TableColumn::default()
                })
                .collect(),
            style: Some(TableStyleInfo {
                name: Some(style.into()),
                show_row_stripes: true,
                show_last_column: true,
                ..TableStyleInfo::default()
            }),
            ..Table::default()
        }
    }

    #[test]
    fn header_stripes_totals_and_last_column() {
        // Measured on the reference (round 27): Medium 2 = accent 1 header with white bold
        // text, first band D9E1F2, lines 8EA9DB, totals with a double accent line on top.
        let t = table("TableStyleMedium2");
        let h = look(&t, 1, 1).unwrap();
        assert_eq!(h.fill.as_deref(), Some("4472C4"));
        assert_eq!(h.font.as_deref(), Some("FFFFFF"));
        assert!(h.bold);
        // Data rows 3..5: the first (row 3) and third (row 5) are banded, row 4 is not.
        assert_eq!(look(&t, 2, 1).unwrap().fill.as_deref(), Some("D9E1F2"));
        assert_eq!(look(&t, 3, 2).unwrap().fill, None);
        assert_eq!(look(&t, 4, 1).unwrap().fill.as_deref(), Some("D9E1F2"));
        let line = look(&t, 3, 2).unwrap().borders[0].clone().unwrap();
        assert_eq!((line.color.as_str(), line.width), ("8EA9DB", 1));
        // Last column bold on an unbanded row.
        assert!(look(&t, 3, 3).unwrap().bold);
        // Totals row (row 5): bold, a double line on top.
        let tot = look(&t, 5, 2).unwrap();
        assert!(tot.bold);
        assert!(tot.borders[0].as_ref().unwrap().double);
        // Outside the table.
        assert_eq!(look(&t, 0, 0), None);
    }

    #[test]
    fn gallery_has_the_60_reference_styles() {
        let g = gallery();
        assert_eq!(g.len(), 60);
        assert_eq!(g[0].name, "TableStyleLight1");
        assert_eq!(g[21 + 1].name, "TableStyleMedium2");
        assert_eq!(g[21 + 1].header.as_deref(), Some("4472C4"));
        assert_eq!(g[59].name, "TableStyleDark11");
        assert!(g.iter().all(|e| is_builtin(&e.name)));
        // Dark 2: black header, dark accent bands.
        assert_eq!(g[49 + 1].header.as_deref(), Some("000000"));
        assert_eq!(g[49 + 1].stripe, "305496");
        // Custom names use the pivot palette, which follows the default theme
        // (Medium2 = accent 1 of BlueSheet's default theme).
        assert!(!is_builtin("MyStyle"));
        assert_eq!(
            palette("TableStyleMedium2").header_fill.as_deref(),
            Some("2F6FB5")
        );
    }

    #[test]
    fn overlay_respects_own_fill_and_conditional_formats() {
        use bo_model::sheet::{CellValue, Fill, Worksheet};
        let mut wb = Workbook::new();
        let mut ws = Worksheet::new("S");
        ws.set_a1("B3", CellValue::Number(1.0));
        ws.tables.push(table("TableStyleMedium2"));
        wb.sheets = vec![ws];
        let red = wb.styles.add_fill(Fill::solid("FF0000"));
        let xf = wb.styles.add_xf(bo_model::sheet::CellXf {
            fill_id: red,
            ..bo_model::sheet::CellXf::default()
        });
        wb.sheets[0].cells.get_mut(&(2, 1)).unwrap().style = xf;
        let mut cf: HashMap<(u32, u32), CellFormat> = HashMap::new();
        cf.insert(
            (4, 1),
            CellFormat {
                fill: Some(Rgba(1, 2, 3, 255)),
                ..CellFormat::default()
            },
        );
        overlay(&wb, 0, 0..100, 0..100, &mut cf);
        // B3 has its own red fill: no table fill.
        assert_eq!(cf.get(&(2, 1)).and_then(|f| f.fill), None);
        // B5: conditional fill kept.
        assert_eq!(cf[&(4, 1)].fill, Some(Rgba(1, 2, 3, 255)));
        // C3 (empty, banded row): band fill.
        assert_eq!(cf[&(2, 2)].fill, rgba("D9E1F2"));
        // Header text white and bold.
        assert_eq!(cf[&(1, 1)].font_color, Some(Rgba(255, 255, 255, 255)));
        assert_eq!(cf[&(1, 1)].bold, Some(true));
    }

    #[test]
    fn table_styles_follow_the_workbook_theme() {
        let t = table("TableStyleMedium2");
        // Header of Medium 2: accent 1 fill with the default theme.
        let l = look(&t, 1, 2).expect("header look");
        assert_eq!(l.fill.as_deref(), Some("4472C4"));
        // A theme whose accent 1 is green restyles the header; a band (accent 1 at +0.8)
        // follows with the same tint.
        let mut colors: Vec<String> = [
            "000000", "FFFFFF", "44546A", "E7E6E6", "00B050", "ED7D31", "A5A5A5", "FFC000",
            "5B9BD5", "70AD47",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        colors.extend(["0563C1".to_owned(), "954F72".to_owned()]);
        let theme = ThemeColors { colors };
        assert_eq!(themed(l, Some(&theme)).fill.as_deref(), Some("00B050"));
        let band = look(&t, 2, 2).expect("band");
        let green = themed(band.clone(), Some(&theme));
        assert_ne!(green.fill, band.fill);
        let (r, g, b) = crate::color::apply_tint(
            (0x00, 0xB0, 0x50),
            theme_slots()[band.fill.as_deref().unwrap()].1,
        );
        assert_eq!(green.fill, Some(format!("{r:02X}{g:02X}{b:02X}")));
        // Without a theme of its own the measured colours stay.
        assert_eq!(themed(band.clone(), None), band);
    }

    #[test]
    fn custom_styles_compose_their_elements() {
        use bo_model::sheet::TableStyleElementParts;
        let el = |kind: &str, size: u32, d: Option<Dxf>| TableStyleElementParts {
            kind: kind.into(),
            size,
            dxf: d,
        };
        let style = TableStyleParts {
            name: "Mine".into(),
            pivot: false,
            elements: vec![
                el(
                    "wholeTable",
                    1,
                    element_dxf(None, None, false, false, Some("#000080")),
                ),
                el(
                    "headerRow",
                    1,
                    element_dxf(Some("#1F4E79"), Some("FFFFFF"), true, false, None),
                ),
                el(
                    "firstRowStripe",
                    2,
                    element_dxf(Some("DDEBF7"), None, false, false, None),
                ),
                el("lastColumn", 1, element_dxf(None, None, true, false, None)),
            ],
        };
        // B2:D6: header row 1, data rows 2–4, totals row 5; columns 1–3.
        let t = table("Mine");
        let h = custom_look(&style, &t, 1, 2, None).expect("header");
        assert_eq!(
            (h.fill.as_deref(), h.font.as_deref(), h.bold),
            (Some("1F4E79"), Some("FFFFFF"), true)
        );
        // The whole table's frame: top edge on the header, inner lines between cells.
        assert_eq!(
            h.borders[0].as_ref().map(|e| e.color.as_str()),
            Some("000080")
        );
        // A first stripe two rows high: rows 2 and 3 shaded, row 4 (second stripe) not.
        assert_eq!(
            custom_look(&style, &t, 2, 2, None)
                .and_then(|l| l.fill)
                .as_deref(),
            Some("DDEBF7")
        );
        assert_eq!(
            custom_look(&style, &t, 3, 2, None)
                .and_then(|l| l.fill)
                .as_deref(),
            Some("DDEBF7")
        );
        assert_eq!(
            custom_look(&style, &t, 4, 2, None).and_then(|l| l.fill),
            None
        );
        // Last column bold (showLastColumn), the others not.
        assert!(custom_look(&style, &t, 4, 3, None).expect("last").bold);
        assert!(!custom_look(&style, &t, 4, 2, None).expect("inner").bold);
        assert_eq!(custom_look(&style, &t, 9, 9, None), None);
        // The workbook's own style wins over the name lookup of built-in styles.
        let mut wb = Workbook::default();
        wb.styles.table_styles.push(style);
        assert_eq!(look_in(&wb, &t, 1, 2), Some(h));
    }
}
