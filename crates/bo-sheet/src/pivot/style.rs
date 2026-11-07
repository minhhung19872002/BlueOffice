//! Pivot table styles (`pivotTableStyleInfo`, ECMA-376 Part 1, §18.10.1;
//! table style elements §18.8.40 `tableStyleElement`).
//!
//! The built-in style *names* of §18.8.40 (`PivotStyleLight1`–`28`,
//! `PivotStyleMedium1`–`28`, `PivotStyleDark1`–`28`) are kept in the file;
//! BlueSheet draws them with its own colour palette: the style family sets
//! how strong the header / total fills are, the number picks one of seven
//! BlueSheet accent colours. The formatting is written into the report cells
//! (header row, subtotal and grand total lines, banded rows / columns).

use super::compute::{ColKind, Layout, RowKind};
use crate::style::{apply_borders, apply_patch, BorderPreset, BorderSpec, StylePatch};
use bo_model::sheet::{CellRange, CellRef, PivotStyleInfo, Workbook};

/// BlueSheet accent colours (`RRGGBB`): neutral, blue, amber, green, violet, teal, brick.
const ACCENTS: [&str; 7] = [
    "5B6472", "2F6FB3", "C9822B", "3F8F5A", "7457A8", "23878A", "B04A3C",
];

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
    pub header_fill: Option<String>,
    pub header_font: Option<String>,
    pub header_border: String,
    pub subtotal_fill: Option<String>,
    pub grand_fill: Option<String>,
    pub grand_font: Option<String>,
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

/// The palette of a style name; unknown names use the light family.
pub fn palette(name: &str) -> Palette {
    let (family, number) = ["Light", "Medium", "Dark"]
        .iter()
        .find_map(|f| {
            let rest = name.strip_prefix("PivotStyle")?.strip_prefix(f)?;
            Some((*f, rest.parse::<usize>().ok()?))
        })
        .unwrap_or(("Light", 16));
    let accent = ACCENTS[number.saturating_sub(1) % ACCENTS.len()];
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
    let p = palette(&info.name);
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
                if info.show_row_headers && lay.first_data_col > 0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_follow_family_and_number() {
        let light = palette("PivotStyleLight16");
        assert_eq!(light.header_border, ACCENTS[1]);
        assert_eq!(light.header_font, None);
        let medium = palette("PivotStyleMedium9");
        assert_eq!(medium.header_fill.as_deref(), Some(ACCENTS[1]));
        assert_eq!(medium.header_font.as_deref(), Some("FFFFFF"));
        let dark = palette("PivotStyleDark2");
        assert_eq!(dark.grand_font.as_deref(), Some("FFFFFF"));
        assert_eq!(palette("Custom"), light);
        assert_eq!(tint("000000", 0.5), "808080");
        assert_eq!(shade("FFFFFF", 0.5), "808080");
    }
}
