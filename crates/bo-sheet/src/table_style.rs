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

use bo_model::sheet::{CellRange, CellRef, SheetColor, Table, Workbook};
use bo_render::Rgba;
use serde::Serialize;

use crate::cf::CellFormat;
use crate::color::{cell_fill_rgb, parse_hex};
use crate::pivot::style::palette;
use crate::style::{apply_patch, StylePatch};
use crate::table::{has_header, has_totals};

/// The look of one table cell: fill and font colours `RRGGBB`, bold.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Look {
    /// Fill; `None` = none.
    pub fill: Option<String>,
    /// Font colour; `None` = automatic.
    pub font: Option<String>,
    /// Bold text.
    pub bold: bool,
}

/// The look of (`row`, `col`) inside table `t`; `None` outside or without style.
#[must_use]
pub fn look(t: &Table, row: u32, col: u32) -> Option<Look> {
    if !t.range.contains(CellRef::new(row, col)) {
        return None;
    }
    let info = t.style.as_ref()?;
    let p = palette(info.name.as_deref()?);
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
        }
    } else if totals {
        Look {
            fill: p.grand_fill,
            font: p.grand_font,
            bold: true,
        }
    } else {
        let i = row - t.range.first.row - u32::from(has_header(t));
        let striped = (info.show_row_stripes && i.is_multiple_of(2))
            || (info.show_column_stripes && k.is_multiple_of(2));
        Look {
            fill: striped.then_some(p.stripe_fill),
            font: None,
            bold: edge_bold,
        }
    };
    (l != Look::default()).then_some(l)
}

fn rgba(hex: &str) -> Option<Rgba> {
    parse_hex(hex).map(|(r, g, b)| Rgba(r, g, b, 255))
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

/// The styles offered by the table gallery: three families × seven accents.
#[must_use]
pub fn gallery() -> Vec<GalleryEntry> {
    let mut out = Vec::new();
    for (family, key) in [("Light", "light"), ("Medium", "medium"), ("Dark", "dark")] {
        for n in 1..=7 {
            let name = format!("TableStyle{family}{n}");
            let p = palette(&name);
            out.push(GalleryEntry {
                name,
                family: key,
                accent: n,
                header: p.header_fill,
                header_font: p.header_font,
                stripe: p.stripe_fill,
                totals: p.grand_fill,
            });
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
        let t = table("TableStyleMedium2");
        let p = palette("TableStyleMedium2");
        // Header B2: accent fill, white bold text.
        let h = look(&t, 1, 1).unwrap();
        assert_eq!(h.fill, p.header_fill);
        assert_eq!(h.font.as_deref(), Some("FFFFFF"));
        assert!(h.bold);
        // Data rows 3..5: the first (row 3) and third (row 5) are striped, row 4 is not.
        assert_eq!(look(&t, 2, 1).unwrap().fill, Some(p.stripe_fill.clone()));
        assert_eq!(look(&t, 3, 1), None);
        assert_eq!(look(&t, 4, 1).unwrap().fill, Some(p.stripe_fill));
        // Last column bold on an unstriped row.
        assert!(look(&t, 3, 3).unwrap().bold);
        // Totals row D6 (row 5): grand fill, bold.
        let tot = look(&t, 5, 2).unwrap();
        assert_eq!(tot.fill, p.grand_fill);
        assert!(tot.bold);
        // Outside the table.
        assert_eq!(look(&t, 0, 0), None);
    }

    #[test]
    fn medium2_uses_the_blue_accent_and_gallery_has_21_styles() {
        // Number 2 → second accent (blue 2F6FB3); medium header = accent itself.
        assert_eq!(
            palette("TableStyleMedium2").header_fill.as_deref(),
            Some("2F6FB3")
        );
        let g = gallery();
        assert_eq!(g.len(), 21);
        assert_eq!(g[8].name, "TableStyleMedium2");
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
        // C3 (empty, striped row): stripe fill.
        let s = rgba(&palette("TableStyleMedium2").stripe_fill);
        assert_eq!(cf[&(2, 2)].fill, s);
        // Header text white and bold.
        assert_eq!(cf[&(1, 1)].font_color, Some(Rgba(255, 255, 255, 255)));
        assert_eq!(cf[&(1, 1)].bold, Some(true));
    }
}
