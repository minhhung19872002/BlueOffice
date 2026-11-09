//! Expected layouts and levels below are worked out by hand.

use super::*;
use crate::formula::BoFormula;

fn rng(s: &str) -> CellRange {
    CellRange::parse(s).unwrap()
}

fn level(ws: &Worksheet, r: u32) -> u8 {
    ws.rows.get(&r).map_or(0, |i| i.outline_level)
}

fn levels(ws: &Worksheet, n: u32) -> Vec<u8> {
    (0..n).map(|r| level(ws, r)).collect()
}

fn put_formula(ws: &mut Worksheet, a1: &str, f: &str) {
    let r = CellRef::parse(a1).unwrap();
    let mut c = Cell::new(CellValue::Empty);
    c.formula = Some(Formula::normal(f));
    ws.set_cell(r.row, r.col, c);
}

fn formula(ws: &Worksheet, r: u32, c: u32) -> Option<String> {
    ws.cell(r, c)
        .and_then(|x| x.formula.as_ref())
        .map(|f| f.text.clone())
}

fn text(ws: &Worksheet, r: u32, c: u32) -> Option<String> {
    ws.cell(r, c).and_then(|x| x.value.as_text())
}

#[test]
fn groups_and_ungroups_rows_and_columns() {
    let mut ws = Worksheet::new("Sheet1");
    assert!(group(&mut ws, true, 1, 3, false));
    assert!(group(&mut ws, true, 2, 2, false));
    assert_eq!(levels(&ws, 5), vec![0, 1, 2, 1, 0]);
    assert_eq!(ws.format.outline_level_row, 2);
    assert!(group(&mut ws, true, 1, 3, true));
    assert_eq!(levels(&ws, 5), vec![0, 0, 1, 0, 0]);
    // Rows with nothing left are dropped from the row map.
    assert!(!ws.rows.contains_key(&1));
    assert!(!group(&mut ws, true, 5, 6, true));
    assert!(group(&mut ws, false, 1, 2, false));
    assert_eq!(ws.col_info(1).map(|c| c.outline_level), Some(1));
    assert_eq!(ws.col_info(2).map(|c| c.outline_level), Some(1));
    assert_eq!(ws.col_info(3), None);
    assert_eq!(ws.format.outline_level_col, 1);
    for _ in 0..10 {
        group(&mut ws, true, 0, 0, false);
    }
    assert_eq!(level(&ws, 0), MAX_LEVEL);
    clear_outline(&mut ws, None);
    assert_eq!(ws.format.outline_level_row, 0);
    assert_eq!(ws.format.outline_level_col, 0);
    assert_eq!(ws.cols.len(), 0);
}

#[test]
fn auto_outline_finds_nested_row_groups() {
    let mut ws = Worksheet::new("Sheet1");
    for r in [1, 2, 4, 5] {
        ws.set_cell(r, 1, Cell::new(CellValue::Number(1.0)));
    }
    put_formula(&mut ws, "B4", "SUM(B2:B3)");
    put_formula(&mut ws, "B7", "SUM(B5:B6)");
    put_formula(&mut ws, "B8", "SUBTOTAL(9,B2:B7)");
    // Not one block next to B9: no group.
    put_formula(&mut ws, "B9", "B4+B7");
    let o = auto_outline(&mut ws, rng("A1:C9"), true, true);
    assert_eq!(
        o,
        AutoOutline {
            row_levels: 2,
            col_levels: 0
        }
    );
    assert_eq!(levels(&ws, 9), vec![0, 2, 2, 1, 2, 2, 1, 0, 0]);
    assert_eq!(ws.format.outline_level_row, 2);
    assert_eq!(ws.props.summary_below, None);
}

#[test]
fn auto_outline_summary_above_and_columns() {
    let mut ws = Worksheet::new("Bảng 1");
    put_formula(&mut ws, "B1", "SUM(B2:B3)");
    // Sheet-qualified with its own name counts; another sheet does not.
    put_formula(&mut ws, "E2", "SUM('Bảng 1'!B2:D2)");
    put_formula(&mut ws, "E3", "SUM(Khác!B3:D3)");
    let o = auto_outline(&mut ws, rng("A1:E3"), true, true);
    assert_eq!(
        o,
        AutoOutline {
            row_levels: 1,
            col_levels: 1
        }
    );
    assert_eq!(levels(&ws, 4), vec![0, 1, 1, 0]);
    assert_eq!(ws.props.summary_below, Some(false));
    assert_eq!(ws.props.summary_right, None);
    let col_levels: Vec<u8> = (0..6)
        .map(|c| ws.col_info(c).map_or(0, |i| i.outline_level))
        .collect();
    assert_eq!(col_levels, vec![0, 1, 1, 1, 0, 0]);
    assert_eq!(ws.format.outline_level_col, 1);
    // Rows only: the column outline stays.
    let o = auto_outline(&mut ws, rng("A1:E3"), true, false);
    assert_eq!(o.col_levels, 0);
    assert_eq!(ws.format.outline_level_col, 1);
}

/// Region | Month | Revenue — North 10, 20; South 5, 7, 8; Centre 4.
fn sales() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    let rows: [(&str, f64, f64); 6] = [
        ("Bắc", 1.0, 10.0),
        ("Bắc", 2.0, 20.0),
        ("Nam", 1.0, 5.0),
        ("nam", 2.0, 7.0),
        ("Nam", 3.0, 8.0),
        ("Trung", 1.0, 4.0),
    ];
    for (i, h) in ["Khu vực", "Tháng", "Doanh thu"].iter().enumerate() {
        ws.set_cell(0, i as u32, Cell::new(CellValue::Text((*h).into())));
    }
    for (r, (a, b, c)) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        ws.set_cell(r, 0, Cell::new(CellValue::Text((*a).into())));
        ws.set_cell(r, 1, Cell::new(CellValue::Number(*b)));
        ws.set_cell(r, 2, Cell::new(CellValue::Number(*c)));
    }
    wb
}

fn spec(below: bool) -> SubtotalSpec {
    SubtotalSpec {
        range: rng("A1:C7"),
        group_by: 0,
        function: ConsolidateFn::Sum,
        columns: vec![2],
        replace: true,
        page_breaks: true,
        summary_below: below,
        label: None,
        grand_label: None,
    }
}

#[test]
fn subtotal_below_inserts_rows_formulas_and_outline() {
    let mut wb = sales();
    let adj = BoFormula::default();
    let r = subtotal(&mut wb, 0, &spec(true), &adj, &Locale::VI_VN).unwrap();
    assert_eq!(r.groups, 3);
    assert_eq!(r.range, rng("A1:C11"));
    assert_eq!(r.levels, 2);
    let ws = &wb.sheets[0];
    // Header; Bắc ×2; total; Nam ×3 (case-insensitive run); total; Trung; total; grand total.
    let labels: Vec<Option<String>> = (0..11).map(|r| text(ws, r, 0)).collect();
    let s = |x: &str| Some(x.to_owned());
    assert_eq!(
        labels,
        vec![
            s("Khu vực"),
            s("Bắc"),
            s("Bắc"),
            s("Tổng: Bắc"),
            s("Nam"),
            s("nam"),
            s("Nam"),
            s("Tổng: Nam"),
            s("Trung"),
            s("Tổng: Trung"),
            s("Tổng cộng"),
        ]
    );
    assert_eq!(formula(ws, 3, 2), s("SUBTOTAL(9,C2:C3)"));
    assert_eq!(formula(ws, 7, 2), s("SUBTOTAL(9,C5:C7)"));
    assert_eq!(formula(ws, 9, 2), s("SUBTOTAL(9,C9:C9)"));
    assert_eq!(formula(ws, 10, 2), s("SUBTOTAL(9,C2:C10)"));
    assert_eq!(formula(ws, 3, 1), None);
    assert_eq!(levels(ws, 11), vec![0, 2, 2, 1, 2, 2, 2, 1, 2, 1, 0]);
    assert_eq!(ws.format.outline_level_row, 2);
    // Page breaks after the first two subtotal rows.
    assert_eq!(ws.page_setup.row_breaks, vec![3, 7]);
    assert_eq!(ws.props.summary_below, None);

    // Again with "replace": the same result, not nested subtotals.
    let mut again = spec(true);
    again.range = r.range;
    let r2 = subtotal(&mut wb, 0, &again, &adj, &Locale::VI_VN).unwrap();
    assert_eq!(r2.range, rng("A1:C11"));
    let ws = &wb.sheets[0];
    assert_eq!(formula(ws, 10, 2), s("SUBTOTAL(9,C2:C10)"));
    assert_eq!(levels(ws, 12), vec![0, 2, 2, 1, 2, 2, 2, 1, 2, 1, 0, 0]);
    assert_eq!(ws.page_setup.row_breaks, vec![3, 7]);
    assert!(ws.cell(11, 0).is_none());

    // Remove all: back to the list.
    let back = remove_subtotals(&mut wb, 0, r2.range, &adj).unwrap();
    assert_eq!(back, rng("A1:C7"));
    let ws = &wb.sheets[0];
    assert_eq!(wb.sheets[0].cells, sales().sheets[0].cells);
    assert_eq!(ws.format.outline_level_row, 0);
    assert_eq!(ws.page_setup.row_breaks, Vec::<u32>::new());
}

#[test]
fn subtotal_above_with_average_and_english_labels() {
    let mut wb = sales();
    let mut sp = spec(false);
    sp.function = ConsolidateFn::Average;
    sp.columns = vec![1, 2];
    sp.page_breaks = true;
    let r = subtotal(&mut wb, 0, &sp, &BoFormula::default(), &Locale::EN_US).unwrap();
    assert_eq!(r.range, rng("A1:C11"));
    let ws = &wb.sheets[0];
    let s = |x: &str| Some(x.to_owned());
    assert_eq!(text(ws, 1, 0), s("Total of all"));
    assert_eq!(formula(ws, 1, 2), s("SUBTOTAL(1,C3:C11)"));
    assert_eq!(text(ws, 2, 0), s("Total: Bắc"));
    assert_eq!(formula(ws, 2, 1), s("SUBTOTAL(1,B4:B5)"));
    assert_eq!(formula(ws, 2, 2), s("SUBTOTAL(1,C4:C5)"));
    assert_eq!(text(ws, 3, 0), s("Bắc"));
    assert_eq!(formula(ws, 5, 2), s("SUBTOTAL(1,C7:C9)"));
    assert_eq!(text(ws, 9, 0), s("Total: Trung"));
    assert_eq!(formula(ws, 9, 2), s("SUBTOTAL(1,C11:C11)"));
    assert_eq!(text(ws, 10, 0), s("Trung"));
    assert_eq!(levels(ws, 11), vec![0, 0, 1, 2, 2, 1, 2, 2, 2, 1, 2]);
    assert_eq!(ws.props.summary_below, Some(false));
    // Breaks before the second and third subtotal rows.
    assert_eq!(ws.page_setup.row_breaks, vec![4, 8]);
}

#[test]
fn subtotal_errors() {
    let mut wb = sales();
    let adj = BoFormula::default();
    let mut sp = spec(true);
    sp.group_by = 5;
    assert_eq!(
        subtotal(&mut wb, 0, &sp, &adj, &Locale::VI_VN),
        Err("badColumn".into())
    );
    let mut sp = spec(true);
    sp.range = rng("A1:C1");
    assert_eq!(
        subtotal(&mut wb, 0, &sp, &adj, &Locale::VI_VN),
        Err("noData".into())
    );
}

#[test]
fn show_level_hides_detail_and_marks_collapsed() {
    let mut wb = sales();
    subtotal(
        &mut wb,
        0,
        &spec(true),
        &BoFormula::default(),
        &Locale::VI_VN,
    )
    .unwrap();
    let ws = &mut wb.sheets[0];
    let hidden = |ws: &Worksheet| -> Vec<u32> {
        (0..11)
            .filter(|r| ws.rows.get(r).is_some_and(|i| i.hidden))
            .collect()
    };
    let collapsed = |ws: &Worksheet| -> Vec<u32> {
        (0..11)
            .filter(|r| ws.rows.get(r).is_some_and(|i| i.collapsed))
            .collect()
    };
    show_level(ws, true, 2, true);
    assert_eq!(hidden(ws), vec![1, 2, 4, 5, 6, 8]);
    assert_eq!(collapsed(ws), vec![3, 7, 9]);
    show_level(ws, true, 1, true);
    assert_eq!(hidden(ws), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(collapsed(ws), vec![10]);
    show_level(ws, true, 3, true);
    assert_eq!(hidden(ws), Vec::<u32>::new());
    assert_eq!(collapsed(ws), Vec::<u32>::new());
}
