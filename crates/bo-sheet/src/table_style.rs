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

use bo_model::sheet::{CellRange, CellRef, SheetColor, Table, ThemeColors, Workbook};
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
                let Some(l) = look(t, r, c) else { continue };
                let l = themed(l, wb.theme.as_ref());
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
            let Some(l) = look(t, r, c) else { continue };
            let l = themed(l, wb.theme.as_ref());
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
                if let Some(l) = look(t, r, c) {
                    if l.borders.iter().any(Option::is_some) {
                        out.push((r, c, themed(l, wb.theme.as_ref()).borders));
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
        // Custom names keep BlueSheet's own palette.
        assert!(!is_builtin("MyStyle"));
        assert_eq!(
            palette("TableStyleMedium2").header_fill.as_deref(),
            Some("2F6FB3")
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
}
