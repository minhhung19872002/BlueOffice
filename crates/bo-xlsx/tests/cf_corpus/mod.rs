//! Conditional formatting corpus files written by BlueSheet (SHEET-CF-001): data bars with
//! negative values and an axis ([MS-XLSX] x14 `CT_DataBar`), every icon set family
//! (§18.18.42 `ST_IconSetType` + x14 sets), custom icons (`x14:cfIcon`), Stop If True
//! and priority order (§18.3.1.10), and rules on pivot table values
//! (§18.3.1.18 `pivot`, §18.10.1.15 `conditionalFormat`).

use super::m;
use bo_sheet::cf::{
    add_rule, CfIconSpec, CfRuleSpec, CfStyle, CfvoSpec, DataBarOptions, IconSetOptions,
};

fn rng(s: &str) -> m::CellRange {
    m::CellRange::parse(s).unwrap()
}

fn col(c: u32) -> char {
    char::from(b'A' + u8::try_from(c).unwrap())
}

/// `values` down column `c` from row 1, with a header row above when `header` is set.
fn column(wb: &mut m::Workbook, c: u32, header: Option<&str>, values: &[f64]) -> String {
    let ws = &mut wb.sheets[0];
    let top = u32::from(header.is_some());
    if let Some(h) = header {
        ws.set_cell(0, c, m::Cell::new(m::CellValue::Text(h.into())));
    }
    for (i, v) in values.iter().enumerate() {
        ws.set_cell(top + i as u32, c, m::Cell::new(m::CellValue::Number(*v)));
    }
    let n = values.len() as u32;
    format!("{0}{1}:{0}{2}", col(c), top + 1, top + n)
}

fn cfvo(kind: &str, value: Option<&str>) -> CfvoSpec {
    CfvoSpec {
        kind: kind.into(),
        value: value.map(str::to_owned),
        gte: true,
    }
}

fn bar_opts() -> DataBarOptions {
    DataBarOptions {
        min: cfvo("autoMin", None),
        max: cfvo("autoMax", None),
        min_length: 0,
        max_length: 100,
        gradient: true,
        border: false,
        border_color: None,
        negative_color: None,
        negative_border_color: None,
        negative_same_color: false,
        negative_same_border: true,
        axis_position: "automatic".into(),
        axis_color: None,
        direction: "context".into(),
        show_value: true,
    }
}

fn bar(wb: &mut m::Workbook, range: &str, color: &str, options: DataBarOptions) {
    assert!(add_rule(
        wb,
        0,
        rng(range),
        &CfRuleSpec::DataBar {
            color: color.into(),
            options: Some(options),
        }
    ));
}

const MIXED: [f64; 10] = [-30.0, -12.0, -5.0, 0.0, 4.0, 9.0, 15.0, 22.0, 30.0, 45.0];

/// Data bars with negative values: automatic axis + gradient; middle axis, solid fill,
/// borders and own negative colours; right-to-left, no axis and every threshold type.
pub fn writer_cf_databars(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    wb.sheets[0].name = "Thanh dữ liệu".into();
    match i {
        1 => {
            let a = column(&mut wb, 0, Some("Lãi/lỗ"), &MIXED);
            bar(&mut wb, &a, "4A7BB7", bar_opts());
            let neg: Vec<f64> = MIXED.iter().map(|v| -v.abs() - 1.0).collect();
            let b = column(&mut wb, 1, Some("Toàn âm"), &neg);
            bar(&mut wb, &b, "2FA36B", bar_opts());
            let pos: Vec<f64> = MIXED.iter().map(|v| v.abs()).collect();
            let c = column(&mut wb, 2, Some("Toàn dương"), &pos);
            bar(&mut wb, &c, "E07B39", bar_opts());
        }
        2 => {
            let a = column(&mut wb, 0, Some("Giữa"), &MIXED);
            bar(
                &mut wb,
                &a,
                "2E6DB4",
                DataBarOptions {
                    axis_position: "middle".into(),
                    gradient: false,
                    border: true,
                    border_color: Some("1F3F66".into()),
                    negative_color: Some("8E3B8E".into()),
                    negative_border_color: Some("5A1E5A".into()),
                    negative_same_border: false,
                    axis_color: Some("7F7F7F".into()),
                    ..bar_opts()
                },
            );
            let b = column(&mut wb, 1, Some("Cùng màu"), &MIXED);
            bar(
                &mut wb,
                &b,
                "5AA454",
                DataBarOptions {
                    min: cfvo("num", Some("-20")),
                    max: cfvo("num", Some("40")),
                    negative_same_color: true,
                    border: true,
                    ..bar_opts()
                },
            );
        }
        _ => {
            let a = column(&mut wb, 0, Some("Phải sang trái"), &MIXED);
            bar(
                &mut wb,
                &a,
                "4BA3DB",
                DataBarOptions {
                    direction: "rightToLeft".into(),
                    ..bar_opts()
                },
            );
            let b = column(&mut wb, 1, Some("Không trục"), &MIXED);
            bar(
                &mut wb,
                &b,
                "F2B632",
                DataBarOptions {
                    axis_position: "none".into(),
                    min: cfvo("percent", Some("10")),
                    max: cfvo("percentile", Some("90")),
                    ..bar_opts()
                },
            );
            let c = column(&mut wb, 2, Some("Công thức"), &MIXED);
            bar(
                &mut wb,
                &c,
                "9E9E9E",
                DataBarOptions {
                    min: cfvo("formula", Some("MIN($C$2:$C$11)")),
                    max: cfvo("formula", Some("MAX($C$2:$C$11)*1.2")),
                    min_length: 5,
                    max_length: 95,
                    show_value: false,
                    ..bar_opts()
                },
            );
        }
    }
    wb
}

fn icons(
    wb: &mut m::Workbook,
    range: &str,
    set: &str,
    reverse: bool,
    options: Option<IconSetOptions>,
) {
    assert!(add_rule(
        wb,
        0,
        rng(range),
        &CfRuleSpec::IconSet {
            set: set.into(),
            reverse,
            options,
        }
    ));
}

/// Every icon set family: the 3-icon sets; the 4-icon sets (plain and reversed); the
/// 5-icon sets with number / percentile / formula thresholds and icon-only display.
pub fn writer_cf_icons(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    wb.sheets[0].name = "Bộ biểu tượng".into();
    let values: Vec<f64> = (1..=10).map(f64::from).collect();
    let sets: &[&str] = match i {
        1 => &[
            "3Arrows",
            "3ArrowsGray",
            "3Flags",
            "3TrafficLights1",
            "3TrafficLights2",
            "3Signs",
            "3Symbols",
            "3Symbols2",
            "3Stars",
            "3Triangles",
        ],
        2 => &[
            "4Arrows",
            "4ArrowsGray",
            "4RedToBlack",
            "4Rating",
            "4TrafficLights",
        ],
        _ => &["5Arrows", "5ArrowsGray", "5Rating", "5Quarters", "5Boxes"],
    };
    for (c, set) in sets.iter().enumerate() {
        let r = column(&mut wb, c as u32, Some(set), &values);
        icons(&mut wb, &r, set, false, None);
    }
    if i == 2 {
        for (k, set) in sets.iter().enumerate() {
            let c = (sets.len() + k) as u32;
            let r = column(&mut wb, c, Some(&format!("{set} ↓")), &values);
            icons(&mut wb, &r, set, true, None);
        }
    }
    if i == 3 {
        let th = |kind: &str, v: &str, gte: bool| CfvoSpec {
            kind: kind.into(),
            value: Some(v.into()),
            gte,
        };
        let r = column(&mut wb, 5, Some("Số"), &values);
        icons(
            &mut wb,
            &r,
            "5Quarters",
            false,
            Some(IconSetOptions {
                thresholds: vec![
                    th("num", "2", true),
                    th("num", "4", false),
                    th("percentile", "60", true),
                    th("formula", "$F$2*9", true),
                ],
                show_value: false,
                icons: vec![],
            }),
        );
    }
    wb
}

fn fill(hex: &str) -> CfStyle {
    CfStyle {
        fill: Some(hex.into()),
        ..CfStyle::default()
    }
}

/// Custom icon sets, Stop If True and rule priority order.
pub fn writer_cf_custom(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    wb.sheets[0].name = "Quy tắc".into();
    let values: Vec<f64> = (1..=12).map(|v| f64::from(v) * 5.0 - 20.0).collect();
    let a = column(&mut wb, 0, Some("Giá trị"), &values);
    let icon = |set: &str, id: u32| CfIconSpec {
        set: set.into(),
        id,
    };
    let th = |v: &str| CfvoSpec {
        kind: "percent".into(),
        value: Some(v.into()),
        gte: true,
    };
    match i {
        1 => {
            icons(
                &mut wb,
                &a,
                "3TrafficLights1",
                false,
                Some(IconSetOptions {
                    thresholds: vec![th("33"), th("67")],
                    show_value: true,
                    icons: vec![icon("3Flags", 0), icon("NoIcons", 0), icon("3Stars", 2)],
                }),
            );
            // Highest priority, Stop If True: negative values get only the red fill.
            assert!(add_rule(
                &mut wb,
                0,
                rng(&a),
                &CfRuleSpec::CellIs {
                    operator: "lessThan".into(),
                    formula1: "0".into(),
                    formula2: None,
                    style: fill("F4CCCC"),
                }
            ));
            let r = &mut wb.sheets[0].conditional_formats;
            let last = r.len() - 1;
            r[last].rules[0].stop_if_true = true;
        }
        2 => {
            for (f, color) in [("10", "D9EAD3"), ("20", "FFF2CC"), ("30", "F4CCCC")] {
                assert!(add_rule(
                    &mut wb,
                    0,
                    rng(&a),
                    &CfRuleSpec::CellIs {
                        operator: "greaterThan".into(),
                        formula1: f.into(),
                        formula2: None,
                        style: fill(color),
                    }
                ));
            }
            // "> 20" stops; then "> 10" is moved to the top.
            let ws = &mut wb.sheets[0];
            ws.conditional_formats[1].rules[0].stop_if_true = true;
            bo_sheet::cf_manage::apply(&mut wb, 0, 0, 0, &bo_sheet::cf_manage::CfOp::MoveUp)
                .unwrap();
            bo_sheet::cf_manage::apply(&mut wb, 0, 0, 0, &bo_sheet::cf_manage::CfOp::MoveUp)
                .unwrap();
        }
        _ => {
            icons(
                &mut wb,
                &a,
                "5Rating",
                false,
                Some(IconSetOptions {
                    thresholds: vec![th("20"), th("40"), th("60"), th("80")],
                    show_value: true,
                    icons: vec![
                        icon("3Symbols", 0),
                        icon("4TrafficLights", 1),
                        icon("3Signs", 1),
                        icon("5Boxes", 3),
                        icon("3Symbols2", 2),
                    ],
                }),
            );
            let b = column(&mut wb, 1, Some("Thang màu"), &values);
            assert!(add_rule(
                &mut wb,
                0,
                rng(&b),
                &CfRuleSpec::ColorScale {
                    min_color: "F8D7DA".into(),
                    mid_color: Some("FFF3CD".into()),
                    max_color: "D4EDDA".into(),
                }
            ));
            assert!(add_rule(
                &mut wb,
                0,
                rng(&b),
                &CfRuleSpec::Top10 {
                    rank: 2,
                    bottom: false,
                    percent: false,
                    style: CfStyle {
                        bold: true,
                        font_color: Some("1F3F66".into()),
                        ..CfStyle::default()
                    },
                }
            ));
            let r = &mut wb.sheets[0].conditional_formats;
            let last = r.len() - 1;
            r[last].rules[0].stop_if_true = true;
        }
    }
    wb
}

/// Rules on the values of a pivot table (`pivot="1"` + pivot `conditionalFormat scope="data"`).
pub fn writer_cf_pivot(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, PivotSpec};
    let mut wb = super::pivot_book();
    let spec = PivotSpec {
        rows: vec![0],
        cols: if i == 2 { vec![1] } else { vec![] },
        values: vec![super::pivot_value(
            3,
            m::PivotFunction::Sum,
            "Tổng Doanh thu",
        )],
        ..PivotSpec::default()
    };
    create_pivot(
        &mut wb,
        0,
        rng("A1:E31"),
        1,
        m::CellRef::new(2, 0),
        &spec,
        &super::pivot_labels(),
    )
    .unwrap();
    let (r, c) = (4, 1);
    let rules: Vec<CfRuleSpec> = match i {
        1 => vec![CfRuleSpec::DataBar {
            color: "4A7BB7".into(),
            options: None,
        }],
        2 => vec![CfRuleSpec::IconSet {
            set: "3Arrows".into(),
            reverse: false,
            options: None,
        }],
        _ => vec![
            CfRuleSpec::ColorScale {
                min_color: "FFFFFF".into(),
                mid_color: None,
                max_color: "5AA454".into(),
            },
            CfRuleSpec::Top10 {
                rank: 1,
                bottom: false,
                percent: false,
                style: CfStyle {
                    bold: true,
                    ..CfStyle::default()
                },
            },
        ],
    };
    for s in &rules {
        bo_sheet::cf_manage::add_pivot_rule(&mut wb, 1, r, c, s).unwrap();
    }
    wb
}
