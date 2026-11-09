//! Expected values below are worked out by hand from the inputs.

use super::*;
use crate::formula::BoFormula;
use crate::validation::{set_rule, ValidationSpec};
use bo_model::sheet::{CellRef, Formula};

fn rng(s: &str) -> CellRange {
    CellRange::parse(s).unwrap()
}

fn num(n: f64) -> CellValue {
    CellValue::Number(n)
}

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn delim(d: &[char], consecutive: bool, q: Option<char>) -> SplitMode {
    SplitMode::Delimited {
        delimiters: d.to_vec(),
        consecutive,
        qualifier: q,
    }
}

#[test]
fn splits_quoted_vietnamese_fields() {
    let m = delim(&[','], false, Some('"'));
    assert_eq!(
        split_text("Nguyễn Văn A,\"Hà Nội, Việt Nam\",30", &m),
        vec!["Nguyễn Văn A", "Hà Nội, Việt Nam", "30"]
    );
    // A doubled qualifier inside a qualified field is one quote.
    assert_eq!(split_text("\"a \"\"b\"\"\",c", &m), vec!["a \"b\"", "c"]);
    // Without a qualifier the quotes are plain text.
    assert_eq!(
        split_text("\"x,y\"", &delim(&[','], false, None)),
        vec!["\"x", "y\""]
    );
    // A trailing delimiter makes an empty last field.
    assert_eq!(split_text("a,", &m), vec!["a", ""]);
}

#[test]
fn several_delimiters_and_consecutive_runs() {
    assert_eq!(
        split_text("a;;b", &delim(&[';'], false, None)),
        vec!["a", "", "b"]
    );
    assert_eq!(
        split_text("a;;b", &delim(&[';'], true, None)),
        vec!["a", "b"]
    );
    assert_eq!(
        split_text(
            "Lê Thị;Hoa\tĐà Nẵng",
            &delim(&[';', '\t', ' '], false, None)
        ),
        vec!["Lê", "Thị", "Hoa", "Đà", "Nẵng"]
    );
    assert_eq!(
        split_text("1  2   3", &delim(&[' '], true, None)),
        vec!["1", "2", "3"]
    );
}

#[test]
fn fixed_width_counts_characters() {
    // "Lê  Thị Hoa 25": 14 characters; breaks at 4 and 12.
    let m = SplitMode::Fixed {
        breaks: vec![12, 4, 4, 40],
    };
    assert_eq!(
        split_text("Lê  Thị Hoa 25", &m),
        vec!["Lê", "Thị Hoa", "25"]
    );
    // A line shorter than a break has fewer fields.
    assert_eq!(split_text("ab", &m), vec!["ab"]);
}

#[test]
fn dates_in_each_order() {
    // 2024-01-01 is serial 45292; + 31 (Jan) + 29 (Feb 2024) + 14 = 45366 for 15 Mar 2024.
    let d = 45_366.0;
    assert_eq!(
        parse_date_order("15/03/2024", DateOrder::Dmy, false),
        Some(d)
    );
    assert_eq!(parse_date_order("03/15/24", DateOrder::Mdy, false), Some(d));
    assert_eq!(
        parse_date_order("2024-3-15", DateOrder::Ymd, false),
        Some(d)
    );
    assert_eq!(
        parse_date_order("15.2024.3", DateOrder::Dym, false),
        Some(d)
    );
    assert_eq!(
        parse_date_order("2024 15 3", DateOrder::Ydm, false),
        Some(d)
    );
    assert_eq!(
        parse_date_order("3/2024/15", DateOrder::Myd, false),
        Some(d)
    );
    assert_eq!(parse_date_order("31/02/2024", DateOrder::Dmy, false), None);
    assert_eq!(parse_date_order("15/03/2024", DateOrder::Mdy, false), None);
    assert_eq!(parse_date_order("15/03", DateOrder::Dmy, false), None);
    // Two-digit years: 30–99 → 19xx. 1999-12-31 is serial 36525.
    assert_eq!(
        parse_date_order("31-12-99", DateOrder::Dmy, false),
        Some(36_525.0)
    );
}

fn dedupe_sheet() -> Worksheet {
    let mut ws = Worksheet::new("Sheet1");
    let rows: [(&str, f64, &str); 6] = [
        ("Tỉnh", 0.0, "Ghi chú"),
        ("Hà Nội", 1.0, "x"),
        ("HÀ NỘI", 1.0, "y"),
        ("Huế", 2.0, "z"),
        ("Hà Nội", 1.0, "w"),
        ("Huế", 3.0, "v"),
    ];
    for (r, (a, b, c)) in rows.iter().enumerate() {
        let r = r as u32;
        ws.set_cell(r, 0, Cell::new(text(a)));
        if r > 0 {
            ws.set_cell(r, 1, Cell::new(num(*b)));
        } else {
            ws.set_cell(r, 1, Cell::new(text("Mã")));
        }
        ws.set_cell(r, 2, Cell::new(text(c)));
        // Outside the range: must not move.
        ws.set_cell(r, 4, Cell::new(num(f64::from(r))));
    }
    ws
}

#[test]
fn removes_duplicates_on_chosen_columns() {
    let mut ws = dedupe_sheet();
    let mut f = Cell::new(num(6.0));
    f.formula = Some(Formula::normal("B6*2"));
    ws.set_cell(5, 3, f);
    let r = remove_duplicates(&mut ws, rng("A1:D6"), &[0, 1], true, &BoFormula::default());
    // Keys (A, B): hà nội|1, hà nội|1 (dup), huế|2, hà nội|1 (dup), huế|3.
    assert_eq!(
        r,
        DedupeResult {
            removed: 2,
            remaining: 3
        }
    );
    let col = |c: u32| -> Vec<Option<CellValue>> {
        (0..6)
            .map(|r| ws.cell(r, c).map(|x| x.value.clone()))
            .collect()
    };
    assert_eq!(
        col(2),
        vec![
            Some(text("Ghi chú")),
            Some(text("x")),
            Some(text("z")),
            Some(text("v")),
            None,
            None
        ]
    );
    // The formula of row 6 moved to row 4 and follows its row.
    assert_eq!(
        ws.cell(3, 3).unwrap().formula.as_ref().unwrap().text,
        "B4*2"
    );
    // Column E is outside the range.
    assert_eq!(ws.cell(5, 4).unwrap().value, num(5.0));
}

#[test]
fn removes_duplicates_on_all_columns_without_header() {
    let mut ws = dedupe_sheet();
    // All of A:C: every row differs by column C → nothing removed; header counted as data.
    let r = remove_duplicates(&mut ws, rng("A1:C6"), &[], false, &BoFormula::default());
    assert_eq!(
        r,
        DedupeResult {
            removed: 0,
            remaining: 6
        }
    );
    // Only column B: 0 ("Mã" is text), 1, 1, 2, 1, 3 → text, 1, 2, 3 stay.
    let r = remove_duplicates(&mut ws, rng("A1:C6"), &[1], false, &BoFormula::default());
    assert_eq!(
        r,
        DedupeResult {
            removed: 2,
            remaining: 4
        }
    );
}

fn consolidate_book() -> Workbook {
    let mut wb = Workbook::new();
    let mut s2 = Worksheet::new("Chi nhánh 2");
    s2.sheet_id = 2;
    wb.sheets.push(s2);
    let put = |ws: &mut Worksheet, rows: &[[CellValue; 3]]| {
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                if *v != CellValue::Empty {
                    ws.set_cell(r as u32, c as u32, Cell::new(v.clone()));
                }
            }
        }
    };
    put(
        &mut wb.sheets[0],
        &[
            [CellValue::Empty, text("Q1"), text("Q2")],
            [text("Táo"), num(10.0), num(20.0)],
            [text("Cam"), num(5.0), num(7.0)],
        ],
    );
    put(
        &mut wb.sheets[1],
        &[
            [CellValue::Empty, text("q2"), text("Q3")],
            [text("cam"), num(1.0), num(2.0)],
            [text("Lê"), num(3.0), num(4.0)],
        ],
    );
    wb
}

fn values(c: &Consolidated) -> Vec<Vec<Option<CellValue>>> {
    c.cells
        .iter()
        .map(|r| {
            r.iter()
                .map(|x| x.as_ref().map(|x| x.value.clone()))
                .collect()
        })
        .collect()
}

#[test]
fn consolidates_by_labels_across_sheets() {
    let wb = consolidate_book();
    let src = [
        ConsolidateSource {
            sheet: 0,
            range: rng("A1:C3"),
        },
        ConsolidateSource {
            sheet: 1,
            range: rng("A1:C3"),
        },
    ];
    let vi = Locale::VI_VN;
    let c = consolidate(&wb, &src, ConsolidateFn::Sum, true, true, &vi);
    assert_eq!(
        c.row_labels,
        Some(vec![text("Táo"), text("Cam"), text("Lê")])
    );
    assert_eq!(c.col_labels, Some(vec![text("Q1"), text("Q2"), text("Q3")]));
    // Cam/Q2 = 7 + 1; labels match without regard to case.
    assert_eq!(
        values(&c),
        vec![
            vec![Some(num(10.0)), Some(num(20.0)), None],
            vec![Some(num(5.0)), Some(num(8.0)), Some(num(2.0))],
            vec![None, Some(num(3.0)), Some(num(4.0))],
        ]
    );
    assert_eq!(
        c.cells[1][1].as_ref().unwrap().sources,
        vec![(0, 2, 2), (1, 1, 1)]
    );
    let avg = consolidate(&wb, &src, ConsolidateFn::Average, true, true, &vi);
    assert_eq!(values(&avg)[1][1], Some(num(4.0)));
    let n = consolidate(&wb, &src, ConsolidateFn::Count, true, true, &vi);
    assert_eq!(values(&n)[1][1], Some(num(2.0)));
}

#[test]
fn consolidates_by_position() {
    let wb = consolidate_book();
    let src = [
        ConsolidateSource {
            sheet: 0,
            range: rng("B2:C3"),
        },
        ConsolidateSource {
            sheet: 1,
            range: rng("B2:C3"),
        },
    ];
    let c = consolidate(&wb, &src, ConsolidateFn::Sum, false, false, &Locale::EN_US);
    assert_eq!(c.row_labels, None);
    assert_eq!(
        values(&c),
        vec![
            vec![Some(num(11.0)), Some(num(22.0))],
            vec![Some(num(8.0)), Some(num(11.0))],
        ]
    );
    let m = consolidate(&wb, &src, ConsolidateFn::Max, false, false, &Locale::EN_US);
    assert_eq!(
        values(&m),
        vec![
            vec![Some(num(10.0)), Some(num(20.0))],
            vec![Some(num(5.0)), Some(num(7.0))],
        ]
    );
    let p = consolidate(
        &wb,
        &src,
        ConsolidateFn::Product,
        false,
        false,
        &Locale::EN_US,
    );
    assert_eq!(values(&p)[1][1], Some(num(28.0)));
}

#[test]
fn summary_functions() {
    // 2,4,4,4,5,5,7,9: mean 5, squared deviations sum to 32.
    let xs: Vec<CellValue> = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0].map(num).to_vec();
    let mut v: Vec<&CellValue> = xs.iter().collect();
    let t = text("không phải số");
    v.push(&t);
    let n = |f: ConsolidateFn| match f.apply(&v) {
        CellValue::Number(n) => n,
        other => panic!("{other:?}"),
    };
    assert_eq!(n(ConsolidateFn::Sum), 40.0);
    assert_eq!(n(ConsolidateFn::Count), 9.0);
    assert_eq!(n(ConsolidateFn::CountNums), 8.0);
    assert_eq!(n(ConsolidateFn::Average), 5.0);
    assert_eq!(n(ConsolidateFn::Max), 9.0);
    assert_eq!(n(ConsolidateFn::Min), 2.0);
    assert_eq!(n(ConsolidateFn::Varp), 4.0);
    assert_eq!(n(ConsolidateFn::StdDevp), 2.0);
    assert!((n(ConsolidateFn::Var) - 32.0 / 7.0).abs() < 1e-12);
    assert!((n(ConsolidateFn::StdDev) - (32.0f64 / 7.0).sqrt()).abs() < 1e-12);
    assert_eq!(n(ConsolidateFn::Product), 2.0 * 64.0 * 25.0 * 63.0);
    let one = [num(3.0)];
    let one: Vec<&CellValue> = one.iter().collect();
    assert_eq!(
        ConsolidateFn::StdDev.apply(&one),
        CellValue::Error(CellError::Div0)
    );
    assert_eq!(
        ConsolidateFn::Average.apply(&[]),
        CellValue::Error(CellError::Div0)
    );
    assert_eq!(ConsolidateFn::Max.apply(&[]), num(0.0));
    assert_eq!(ConsolidateFn::Sum.subtotal_number(), 9);
    assert_eq!(ConsolidateFn::Count.function_name(), "COUNTA");
}

#[test]
fn lists_invalid_cells() {
    let mut wb = Workbook::new();
    let spec = ValidationSpec {
        kind: "whole".into(),
        operator: None,
        formula1: "1".into(),
        formula2: Some("10".into()),
        allow_blank: true,
        error_title: None,
        error: None,
        error_style: None,
        prompt: None,
    };
    assert!(set_rule(&mut wb, 0, rng("A1:A6"), &spec));
    let list = ValidationSpec {
        kind: "list".into(),
        formula1: "Có,Không".into(),
        formula2: None,
        ..spec.clone()
    };
    assert!(set_rule(&mut wb, 0, rng("C1:C3"), &list));
    let ws = &mut wb.sheets[0];
    ws.set_a1("A1", num(5.0));
    ws.set_a1("A2", num(11.0));
    ws.set_a1("A3", text("abc"));
    ws.set_a1("A5", num(0.0));
    ws.set_a1("A6", num(2.5));
    ws.set_a1("B2", num(99.0)); // no rule
    ws.set_a1("C1", text("có"));
    ws.set_a1("C2", text("Chưa"));
    ws.set_a1("C3", text(""));
    let got = invalid_cells(&wb, 0, &Locale::VI_VN, None, 100);
    // A2 > 10, A3 text, A5 < 1, A6 not whole, C2 not in the list; C1 matches case-insensitively.
    // In row order.
    let want: Vec<(u32, u32)> = ["A2", "C2", "A3", "A5", "A6"]
        .iter()
        .map(|a| {
            let r = CellRef::parse(a).unwrap();
            (r.row, r.col)
        })
        .collect();
    assert_eq!(got, want);
    assert_eq!(invalid_cells(&wb, 0, &Locale::VI_VN, None, 2).len(), 2);
}
