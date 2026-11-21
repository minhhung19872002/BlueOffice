//! Pivot table styles (`pivotTableStyleInfo`, ECMA-376 Part 1, §18.10.1;
//! table style elements §18.8.40 `tableStyleElement`).
//!
//! The built-in style *names* of §18.8.40 (`PivotStyleLight1`–`28`,
//! `PivotStyleMedium1`–`28`, `PivotStyleDark1`–`28`) are kept in the file. The
//! built-in styles are defined on the workbook's theme colours (§18.8.40,
//! §18.8.41): the number picks dark 1 or accent 1–6 in turn (1, 8, 15, 22 →
//! dark 1; 2, 9, 16, 23 → accent 1; …), so a workbook with another theme
//! restyles its reports, as the reference. The family sets how strong the header
//! / total fills are (BlueSheet's own tints). The formatting is written into the
//! report cells (header row, subtotal and grand total lines, banded rows /
//! columns).

use super::compute::{ColKind, Layout, RowKind};
use crate::style::{apply_borders, apply_patch, BorderPreset, BorderSpec, StylePatch};
use bo_model::sheet::{CellRange, CellRef, PivotStyleInfo, ThemeColors, Workbook};

/// The theme colour of a style number (`RRGGBB`): dark 1, then accent 1–6 in turn
/// (SpreadsheetML theme indexes: 1 = dk1, 4–9 = accent1–6).
fn accent(number: usize, theme: Option<&ThemeColors>) -> String {
    let i = number.saturating_sub(1) % 7;
    let index = if i == 0 { 1 } else { 3 + i as u32 };
    crate::color::hex(crate::color::theme_rgb(index, theme))
}

/// Default style of a new pivot table.
pub const DEFAULT_STYLE: &str = "PivotStyleLight16";

/// Style names offered by the UI (a selection of the built-in names).
pub const STYLE_NAMES: &[&str] = &[
    "PivotStyleLight1",
    "PivotStyleLight8",
    "PivotStyleLight9",
    "PivotStyleLight16",
    "PivotStyleLight17",
    "PivotStyleLight20",
    "PivotStyleMedium2",
    "PivotStyleMedium3",
    "PivotStyleMedium4",
    "PivotStyleMedium9",
    "PivotStyleMedium11",
    "PivotStyleDark2",
    "PivotStyleDark4",
    "PivotStyleDark6",
];

/// Colours of one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// Fill `RRGGBB` of the header row; `None` = no fill.
    pub header_fill: Option<String>,
    /// Font colour `RRGGBB` of the header row; `None` = automatic.
    pub header_font: Option<String>,
    /// Colour `RRGGBB` of the line under the header row.
    pub header_border: String,
    /// Fill `RRGGBB` of subtotal lines; `None` = no fill.
    pub subtotal_fill: Option<String>,
    /// Fill `RRGGBB` of the grand total line; `None` = no fill.
    pub grand_fill: Option<String>,
    /// Font colour `RRGGBB` of the grand total line; `None` = automatic.
    pub grand_font: Option<String>,
    /// Fill `RRGGBB` of banded (striped) rows / columns.
    pub stripe_fill: String,
}

/// Mix `hex` with white (`t` = share of white, 0–1).
fn tint(hex: &str, t: f64) -> String {
    let c = u32::from_str_radix(hex, 16).unwrap_or(0);
    let ch = |shift: u32| {
        let v = f64::from((c >> shift) & 0xFF);
        (v + (255.0 - v) * t).round() as u32
    };
    format!("{:02X}{:02X}{:02X}", ch(16), ch(8), ch(0))
}

/// Darken `hex` (`t` = share of black).
fn shade(hex: &str, t: f64) -> String {
    let c = u32::from_str_radix(hex, 16).unwrap_or(0);
    let ch = |shift: u32| (f64::from((c >> shift) & 0xFF) * (1.0 - t)).round() as u32;
    format!("{:02X}{:02X}{:02X}", ch(16), ch(8), ch(0))
}

/// The palette of a pivot or table style name (`PivotStyleMedium2`, `TableStyleMedium2`)
/// with the default theme; unknown names use the light family.
pub fn palette(name: &str) -> Palette {
    palette_themed(name, None)
}

/// One measured element of a built-in style (see `style_table`).
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Element {
    pub fill: Option<String>,
    /// Font colour; `None` = automatic.
    pub font: Option<String>,
    pub bold: bool,
    /// Top / bottom border: colour, `thin` / `medium` / `thick` / `double`.
    pub top: Option<(String, &'static str)>,
    pub bottom: Option<(String, &'static str)>,
}

/// The measured elements of a built-in style: header (label, value column), row subheading,
/// plain data row, banded data row, grand total row, last data row — label / value column
/// each; `None` for other names.
pub(crate) fn elements(name: &str, theme: Option<&ThemeColors>) -> Option<Vec<Element>> {
    let spec = super::style_table::TABLE
        .iter()
        .find(|(n, _)| *n == name)?
        .1;
    let number = name
        .trim_start_matches("PivotStyle")
        .trim_start_matches(|c: char| c.is_ascii_alphabetic())
        .parse::<usize>()
        .ok()?;
    let i = number.saturating_sub(1) % 7;
    let own = if i == 0 { 1 } else { 3 + i as u32 };
    let color = |s: &str| -> Option<String> {
        if s == "-" {
            return None;
        }
        if let Some(hex) = s.strip_prefix('#') {
            return Some(hex.to_owned());
        }
        let (base, tint) = s.split_at(1);
        let index = match base {
            "a" => own,
            "d" => 1,
            _ => 0,
        };
        let tint = tint.parse::<f64>().unwrap_or(0.0);
        let rgb = crate::color::theme_rgb(index, theme);
        Some(crate::color::hex(crate::color::apply_tint(rgb, tint)))
    };
    let border = |s: &str| -> Option<(String, &'static str)> {
        let (c, w) = s.split_once('/')?;
        let style = match w {
            "m" => "medium",
            "k" => "thick",
            "d" => "double",
            _ => "thin",
        };
        Some((color(c)?, style))
    };
    Some(
        spec.split(" | ")
            .map(|cell| {
                let p: Vec<&str> = cell.split_whitespace().collect();
                let font = p.get(1).and_then(|f| color(f));
                Element {
                    fill: p.first().and_then(|f| color(f)),
                    // Dark 1 text is the automatic colour.
                    font: font.filter(|_| p.get(1) != Some(&"d")),
                    bold: p.get(2) == Some(&"1"),
                    top: p.get(3).and_then(|b| border(b)),
                    bottom: p.get(4).and_then(|b| border(b)),
                }
            })
            .collect(),
    )
}

/// [`palette`] on the colours of `theme` (`None`: the default theme): the built-in
/// definitions (measured), else the family's tints.
pub fn palette_themed(name: &str, theme: Option<&ThemeColors>) -> Palette {
    if let Some(e) = elements(name, theme).filter(|e| e.len() >= 10) {
        let border = e[0]
            .bottom
            .clone()
            .or_else(|| e[8].top.clone())
            .map_or_else(|| accent(2, theme), |b| b.0);
        return Palette {
            header_fill: e[0].fill.clone(),
            header_font: e[0].font.clone(),
            header_border: border,
            subtotal_fill: e[2].fill.clone(),
            grand_fill: e[8].fill.clone(),
            grand_font: e[8].font.clone(),
            stripe_fill: e[6].fill.clone().unwrap_or_else(|| "FFFFFF".to_owned()),
        };
    }
    family_palette(name, theme)
}

/// BlueSheet's own tints of the light / medium / dark families (custom style names).
fn family_palette(name: &str, theme: Option<&ThemeColors>) -> Palette {
    let (family, number) = ["Light", "Medium", "Dark"]
        .iter()
        .find_map(|f| {
            let rest = name
                .strip_prefix("PivotStyle")
                .or_else(|| name.strip_prefix("TableStyle"))?
                .strip_prefix(f)?;
            Some((*f, rest.parse::<usize>().ok()?))
        })
        .unwrap_or(("Light", 16));
    let accent = accent(number, theme);
    let accent = accent.as_str();
    match family {
        "Medium" => Palette {
            header_fill: Some(accent.to_owned()),
            header_font: Some("FFFFFF".into()),
            header_border: accent.to_owned(),
            subtotal_fill: Some(tint(accent, 0.8)),
            grand_fill: Some(tint(accent, 0.6)),
            grand_font: None,
            stripe_fill: tint(accent, 0.9),
        },
        "Dark" => Palette {
            header_fill: Some(shade(accent, 0.35)),
            header_font: Some("FFFFFF".into()),
            header_border: shade(accent, 0.35),
            subtotal_fill: Some(tint(accent, 0.55)),
            grand_fill: Some(shade(accent, 0.35)),
            grand_font: Some("FFFFFF".into()),
            stripe_fill: tint(accent, 0.75),
        },
        _ => Palette {
            header_fill: Some(tint(accent, 0.85)),
            header_font: None,
            header_border: accent.to_owned(),
            subtotal_fill: None,
            grand_fill: Some(tint(accent, 0.85)),
            grand_font: None,
            stripe_fill: tint(accent, 0.92),
        },
    }
}

/// Format the report of `lay` placed with its body's top-left at `body`.
pub(crate) fn apply(
    wb: &mut Workbook,
    sheet: usize,
    body: CellRef,
    lay: &Layout,
    info: Option<&PivotStyleInfo>,
) {
    let row_range = |r: u32| {
        CellRange::new(
            CellRef::new(body.row + r, body.col),
            CellRef::new(body.row + r, body.col + lay.width - 1),
        )
    };
    let bold = StylePatch {
        bold: Some(true),
        ..StylePatch::default()
    };
    let Some(info) = info else {
        for r in &lay.bold_rows {
            apply_patch(wb, sheet, row_range(*r), &bold);
        }
        return;
    };
    if let Some(e) = elements(&info.name, wb.theme.as_ref()).filter(|e| e.len() >= 10) {
        apply_measured(wb, sheet, body, lay, info, &e);
        return;
    }
    let p = palette_themed(&info.name, wb.theme.as_ref());
    let fill = |color: &Option<String>, font: &Option<String>, bold: bool| StylePatch {
        fill_color: color.clone(),
        font_color: font.clone(),
        bold: Some(bold),
        ..StylePatch::default()
    };
    let mut stripe = 0usize;
    for (r, kind) in lay.row_kinds.iter().enumerate() {
        let r = r as u32;
        let range = row_range(r);
        match kind {
            RowKind::Header => {
                if info.show_col_headers {
                    apply_patch(
                        wb,
                        sheet,
                        range,
                        &fill(&p.header_fill, &p.header_font, true),
                    );
                } else {
                    apply_patch(wb, sheet, range, &bold);
                }
                if r + 1 == lay.first_data_row {
                    apply_borders(
                        wb,
                        sheet,
                        range,
                        &BorderSpec {
                            preset: BorderPreset::Bottom,
                            style: "thin".into(),
                            color: Some(p.header_border.clone()),
                        },
                    );
                }
            }
            RowKind::Subtotal | RowKind::Group => {
                if lay.bold_rows.contains(&r) {
                    apply_patch(wb, sheet, range, &fill(&p.subtotal_fill, &None, true));
                }
            }
            RowKind::Grand => {
                apply_patch(wb, sheet, range, &fill(&p.grand_fill, &p.grand_font, true));
                apply_borders(
                    wb,
                    sheet,
                    range,
                    &BorderSpec {
                        preset: BorderPreset::Top,
                        style: "double".into(),
                        color: Some(p.header_border.clone()),
                    },
                );
            }
            RowKind::Data => {
                if info.show_row_stripes && stripe % 2 == 1 {
                    apply_patch(
                        wb,
                        sheet,
                        range,
                        &StylePatch {
                            fill_color: Some(p.stripe_fill.clone()),
                            ..StylePatch::default()
                        },
                    );
                }
                stripe += 1;
                // The default light style keeps the items of the innermost row field in
                // regular weight (the reference shows "North", "South" unbolded; outer items
                // are bold through their group / subtotal rows).
                if info.show_row_headers && lay.first_data_col > 0 && info.name != DEFAULT_STYLE {
                    apply_patch(
                        wb,
                        sheet,
                        CellRange::new(
                            range.first,
                            CellRef::new(range.first.row, body.col + lay.first_data_col - 1),
                        ),
                        &bold,
                    );
                }
            }
            RowKind::Blank => {}
        }
    }
    // Banded columns and the grand total column.
    let top = body.row + lay.first_data_row;
    let bottom = body.row + lay.height - 1;
    if top <= bottom {
        let mut band = 0usize;
        for ((col, _), kind) in lay.data_cols.iter().zip(&lay.col_kinds) {
            let range = CellRange::new(
                CellRef::new(top, body.col + col),
                CellRef::new(bottom, body.col + col),
            );
            match kind {
                ColKind::Grand if info.show_last_column => {
                    apply_patch(wb, sheet, range, &bold);
                }
                ColKind::Subtotal => {
                    apply_patch(wb, sheet, range, &bold);
                }
                ColKind::Data => {
                    if info.show_col_stripes && band % 2 == 1 {
                        apply_patch(
                            wb,
                            sheet,
                            range,
                            &StylePatch {
                                fill_color: Some(p.stripe_fill.clone()),
                                ..StylePatch::default()
                            },
                        );
                    }
                    band += 1;
                }
                ColKind::Grand => {}
            }
        }
    }
}

/// Format the report with a built-in style's measured elements (see [`elements`]).
fn apply_measured(
    wb: &mut Workbook,
    sheet: usize,
    body: CellRef,
    lay: &Layout,
    info: &PivotStyleInfo,
    e: &[Element],
) {
    let labels = lay.first_data_col.max(1).min(lay.width);
    // The label columns take the first element of a pair, the value columns the second.
    let paint = |wb: &mut Workbook, r: u32, pair: usize, parts: &[&Element]| {
        for (side, c0, c1) in [(0, 0, labels - 1), (1, labels, lay.width.saturating_sub(1))] {
            if c0 > c1 || (side == 1 && labels >= lay.width) {
                continue;
            }
            let range = CellRange::new(
                CellRef::new(body.row + r, body.col + c0),
                CellRef::new(body.row + r, body.col + c1),
            );
            let mut patch = StylePatch::default();
            for el in parts.iter().copied() {
                let el = if pair == usize::MAX {
                    el
                } else {
                    &e[pair + side]
                };
                if el.fill.is_some() {
                    patch.fill_color.clone_from(&el.fill);
                }
                if el.font.is_some() {
                    patch.font_color.clone_from(&el.font);
                }
                if el.bold {
                    patch.bold = Some(true);
                }
            }
            if patch != StylePatch::default() {
                apply_patch(wb, sheet, range, &patch);
            }
            let last = parts.last().copied();
            let el = if pair == usize::MAX {
                last
            } else {
                Some(&e[pair + side])
            };
            if let Some(el) = el {
                for (b, preset) in [
                    (&el.top, BorderPreset::Top),
                    (&el.bottom, BorderPreset::Bottom),
                ] {
                    if let Some((color, style)) = b {
                        apply_borders(
                            wb,
                            sheet,
                            range,
                            &BorderSpec {
                                preset,
                                style: (*style).into(),
                                color: Some(color.clone()),
                            },
                        );
                    }
                }
            }
        }
    };
    let bold = StylePatch {
        bold: Some(true),
        ..StylePatch::default()
    };
    for (r, kind) in lay.row_kinds.iter().enumerate() {
        let r = r as u32;
        let i = r.saturating_sub(lay.first_data_row) as usize;
        let banded = info.show_row_stripes && i.is_multiple_of(2);
        match kind {
            RowKind::Header => {
                if info.show_col_headers {
                    let mut el = e[0].clone();
                    if r + 1 != lay.first_data_row {
                        el.bottom = None;
                    }
                    let mut v = e[1].clone();
                    v.bottom.clone_from(&el.bottom);
                    paint(wb, r, usize::MAX, &[&el]);
                    // The value columns of the header take the value-column element.
                    let range = CellRange::new(
                        CellRef::new(body.row + r, body.col + labels),
                        CellRef::new(body.row + r, body.col + lay.width.saturating_sub(1)),
                    );
                    if labels < lay.width {
                        apply_patch(
                            wb,
                            sheet,
                            range,
                            &StylePatch {
                                fill_color: v.fill.clone(),
                                font_color: v.font.clone(),
                                bold: Some(v.bold),
                                ..StylePatch::default()
                            },
                        );
                    }
                } else {
                    apply_patch(
                        wb,
                        sheet,
                        CellRange::new(
                            CellRef::new(body.row + r, body.col),
                            CellRef::new(body.row + r, body.col + lay.width - 1),
                        ),
                        &bold,
                    );
                }
            }
            RowKind::Subtotal | RowKind::Group if lay.bold_rows.contains(&r) => {
                let base = if banded { 6 } else { 4 };
                if info.show_row_headers {
                    paint(wb, r, usize::MAX, &[&e[base], &e[2]]);
                    paint(wb, r, base, &[&e[base]]);
                    paint(wb, r, 2, &[&e[base], &e[2]]);
                } else {
                    paint(wb, r, base, &[&e[base]]);
                    apply_patch(
                        wb,
                        sheet,
                        CellRange::new(
                            CellRef::new(body.row + r, body.col),
                            CellRef::new(body.row + r, body.col + lay.width - 1),
                        ),
                        &bold,
                    );
                }
            }
            RowKind::Data | RowKind::Subtotal | RowKind::Group => {
                paint(wb, r, if banded { 6 } else { 4 }, &[&e[4]]);
            }
            RowKind::Grand => paint(wb, r, 8, &[&e[8]]),
            RowKind::Blank => {}
        }
    }
    // Bold subtotal / grand total columns.
    let top = body.row + lay.first_data_row;
    let bottom = body.row + lay.height - 1;
    if top <= bottom {
        for ((col, _), kind) in lay.data_cols.iter().zip(&lay.col_kinds) {
            let on = match kind {
                ColKind::Grand => info.show_last_column,
                ColKind::Subtotal => true,
                ColKind::Data => false,
            };
            if on {
                apply_patch(
                    wb,
                    sheet,
                    CellRange::new(
                        CellRef::new(top, body.col + col),
                        CellRef::new(bottom, body.col + col),
                    ),
                    &bold,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_styles_use_the_measured_theme_tints() {
        // Office 2007 theme (the measurement's): Light16 header DCE6F1 (accent 1 tint 0.8),
        // border 95B3D7 (tint 0.4), banded rows dark 1 tint 0.85; Medium2 header 366092
        // (accent 1 shade 25 %) with white text; Dark2 header 244062 (shade 50 %).
        let theme = ThemeColors {
            colors: [
                "000000", "FFFFFF", "1F497D", "EEECE1", "4F81BD", "C0504D", "9BBB59", "8064A2",
                "4BACC6", "F79646", "0000FF", "800080",
            ]
            .map(str::to_owned)
            .to_vec(),
        };
        // Within one unit of the reference (its HLS rounding).
        let near = |a: Option<&str>, b: &str| {
            let a = a.unwrap_or("000000");
            (0..3).all(|i| {
                let x = i64::from_str_radix(&a[2 * i..2 * i + 2], 16).unwrap_or(0);
                let y = i64::from_str_radix(&b[2 * i..2 * i + 2], 16).unwrap_or(0);
                (x - y).abs() <= 2
            })
        };
        let p = palette_themed("PivotStyleLight16", Some(&theme));
        assert!(near(p.header_fill.as_deref(), "DCE6F1"), "{p:?}");
        assert!(near(Some(&p.header_border), "95B3D7"), "{p:?}");
        assert!(near(Some(&p.stripe_fill), "D9D9D9"), "{p:?}");
        let m = palette_themed("PivotStyleMedium2", Some(&theme));
        assert!(near(m.header_fill.as_deref(), "366092"), "{m:?}");
        assert_eq!(m.header_font.as_deref(), Some("FFFFFF"));
        assert!(near(m.subtotal_fill.as_deref(), "95B3D7"), "{m:?}");
        let d = palette_themed("PivotStyleDark2", Some(&theme));
        assert!(near(d.header_fill.as_deref(), "244062"), "{d:?}");
        assert_eq!(super::super::style_table::TABLE.len(), 84);
    }

    fn family_palette_test(name: &str) -> Palette {
        family_palette(name, None)
    }

    #[test]
    fn palettes_follow_family_and_number() {
        let light = family_palette_test("PivotStyleLight16");
        assert_eq!(light.header_border, accent(2, None), "Light16 is accent 1");
        assert_eq!(accent(15, None), accent(1, None), "every 7th is dark 1");
        assert_eq!(light.header_font, None);
        let medium = family_palette_test("PivotStyleMedium9");
        assert_eq!(
            medium.header_fill.as_deref(),
            Some(accent(9, None).as_str())
        );
        assert_eq!(medium.header_font.as_deref(), Some("FFFFFF"));
        let dark = family_palette_test("PivotStyleDark2");
        assert_eq!(dark.grand_font.as_deref(), Some("FFFFFF"));
        assert_eq!(family_palette_test("Custom"), light);
        assert_eq!(tint("000000", 0.5), "808080");
        // Another theme restyles the built-in styles (accent 2 of the scheme).
        let theme = ThemeColors {
            colors: [
                "000000", "FFFFFF", "1F497D", "EEECE1", "4F81BD", "C0504D", "9BBB59", "8064A2",
                "4BACC6", "F79646", "0000FF", "800080",
            ]
            .map(str::to_owned)
            .to_vec(),
        };
        assert_eq!(
            family_palette("PivotStyleLight17", Some(&theme)).header_border,
            "C0504D"
        );
        assert_eq!(
            family_palette("PivotStyleLight14", Some(&theme)).header_border,
            "F79646"
        );
        assert_eq!(shade("FFFFFF", 0.5), "808080");
    }
}
