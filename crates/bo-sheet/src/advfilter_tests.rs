//! Advanced filter tests. Expected rows are worked out by hand from the data below
//! (zero-based rows; row 0 holds the labels).

use super::*;
use bo_model::sheet::{Cell, CellRange, CellValue, Formula, Workbook};

const DATA: [(&str, &str, f64); 8] = [
    ("Bắc", "An", 120.0),    // 1
    ("Nam", "Bình", 80.0),   // 2
    ("Bắc", "Bảo", 200.0),   // 3
    ("Trung", "Chi", 150.0), // 4
    ("Nam", "Bình", 80.0),   // 5 (same as row 2)
    ("Bắc", "an", 50.0),     // 6
    ("Nam", "Dũng", 300.0),  // 7
    ("Trung", "Bích", 95.0), // 8
];

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn wb() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.set_a1("A1", text("Khu vực"));
    ws.set_a1("B1", text("Nhân viên"));
    ws.set_a1("C1", text("Doanh số"));
    for (i, (a, b, c)) in DATA.iter().enumerate() {
        let r = i as u32 + 1;
        ws.set_cell(r, 0, Cell::new(text(a)));
        ws.set_cell(r, 1, Cell::new(text(b)));
        ws.set_cell(r, 2, Cell::new(CellValue::Number(*c)));
    }
    wb
}

/// Write a criteria block at column F (labels in row 0).
fn criteria(wb: &mut Workbook, rows: &[&[(&str, &str)]]) -> CellRange {
    let ws = &mut wb.sheets[0];
    let labels: Vec<&str> = rows[0].iter().map(|(l, _)| *l).collect();
    for (j, l) in labels.iter().enumerate() {
        ws.set_cell(0, 5 + j as u32, Cell::new(text(l)));
    }
    for (i, row) in rows.iter().enumerate() {
        for (j, (_, v)) in row.iter().enumerate() {
            if !v.is_empty() {
                ws.set_cell(i as u32 + 1, 5 + j as u32, Cell::new(text(v)));
            }
        }
    }
    CellRange::new(
        CellRef::new(0, 5),
        CellRef::new(rows.len() as u32, 5 + labels.len() as u32 - 1),
    )
}

fn spec(criteria: CellRange) -> AdvancedFilter {
    AdvancedFilter {
        list: CellRange::parse("A1:C9").unwrap(),
        criteria_sheet: 0,
        criteria,
        copy_to: None,
        unique: false,
    }
}

fn rows_of(wb: &Workbook, s: &AdvancedFilter) -> Vec<u32> {
    matching_rows(wb, 0, s, &Locale::VI_VN, None).unwrap()
}

#[test]
fn and_within_a_row_or_across_rows() {
    let mut wb = wb();
    // (Bắc AND > 100) OR Nam.
    let c = criteria(
        &mut wb,
        &[
            &[("Khu vực", "Bắc"), ("Doanh số", ">100")],
            &[("Khu vực", "Nam"), ("Doanh số", "")],
        ],
    );
    let s = spec(c);
    assert_eq!(rows_of(&wb, &s), vec![1, 2, 3, 5, 7]);
    let unique = AdvancedFilter {
        unique: true,
        ..s.clone()
    };
    assert_eq!(rows_of(&wb, &unique), vec![1, 2, 3, 7]);
    // In place: the other rows are hidden; names and filter mode recorded.
    let out = apply(&mut wb, 0, &unique, &Locale::VI_VN, None).unwrap();
    assert_eq!((out.matched, out.hidden), (4, 4));
    let hidden: Vec<u32> = wb.sheets[0]
        .rows
        .iter()
        .filter(|(_, i)| i.hidden)
        .map(|(r, _)| *r)
        .collect();
    assert_eq!(hidden, vec![4, 5, 6, 8]);
    let name = |n: &str| {
        wb.defined_names
            .iter()
            .find(|d| d.name == n)
            .map(|d| (d.formula.clone(), d.local_sheet, d.hidden))
    };
    let (f, local, hid) = name(FILTER_DATABASE).unwrap();
    assert!(f.ends_with("!$A$1:$C$9"), "{f}");
    assert_eq!((local, hid), (Some(0), true));
    assert!(name(CRITERIA).unwrap().0.ends_with("!$F$1:$G$3"));
    assert!(wb.sheets[0]
        .props
        .attrs
        .contains(&("filterMode".into(), "1".into())));
    // Show everything again.
    assert_eq!(show_all(&mut wb, 0), 4);
    assert!(wb.sheets[0].rows.values().all(|i| !i.hidden));
    assert!(!wb.sheets[0]
        .props
        .attrs
        .iter()
        .any(|(k, _)| k == "filterMode"));
}

#[test]
fn text_operators_and_wildcards() {
    let cases: [(&str, &str, Vec<u32>); 8] = [
        // A plain value matches text that begins with it (case-insensitive).
        ("Nhân viên", "B", vec![2, 3, 5, 8]),
        ("Nhân viên", "An", vec![1, 6]),
        // `=` matches the whole text; `?` is one character.
        ("Nhân viên", "=b?nh", vec![2, 5]),
        ("Nhân viên", "=*n", vec![1, 6]),
        ("Nhân viên", "=*h", vec![2, 5, 8]),
        ("Khu vực", "<>Nam", vec![1, 3, 4, 6, 8]),
        // Text ordering comparison: "Nam" < "O" <= "Trung".
        ("Khu vực", ">=O", vec![4, 8]),
        // Numbers: a plain number compares equal.
        ("Doanh số", "80", vec![2, 5]),
    ];
    for (label, crit, want) in cases {
        let mut wb = wb();
        let c = criteria(&mut wb, &[&[(label, crit)]]);
        assert_eq!(rows_of(&wb, &spec(c)), want, "{label} {crit}");
    }
    // `<=` / `<` on numbers, AND of two conditions on the same column.
    let mut wb = wb();
    let c = criteria(&mut wb, &[&[("Doanh số", ">=80"), ("Doanh số", "<150")]]);
    assert_eq!(rows_of(&wb, &spec(c)), vec![1, 2, 5, 8]);
}

#[test]
fn numeric_criterion_cell_and_blank_row() {
    let mut wb = wb();
    let c = criteria(&mut wb, &[&[("Doanh số", "")]]);
    wb.sheets[0].set_cell(1, 5, Cell::new(CellValue::Number(300.0)));
    assert_eq!(rows_of(&wb, &spec(c)), vec![7]);
    // A criteria row without any criterion keeps every row.
    let mut wb2 = wb.clone();
    let c2 = criteria(&mut wb2, &[&[("Khu vực", "Trung")], &[("Khu vực", "")]]);
    assert_eq!(rows_of(&wb2, &spec(c2)).len(), 8);
}

#[test]
fn formula_criterion() {
    let mut wb = wb();
    // Label not in the list; formula written for the first data row (row 2).
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 7, Cell::new(text("Trên TB")));
    let mut f = Cell::new(CellValue::Bool(false));
    f.formula = Some(Formula {
        text: "C2>AVERAGE($C$2:$C$9)".into(),
        ..Formula::default()
    });
    ws.set_cell(1, 7, f);
    // Sum 1075 / 8 = 134.375: rows with 200, 150 and 300.
    let s = spec(CellRange::parse("H1:H2").unwrap());
    assert_eq!(rows_of(&wb, &s), vec![3, 4, 7]);
}

#[test]
fn copy_to_another_place() {
    let mut wb = wb();
    let c = criteria(&mut wb, &[&[("Khu vực", "Bắc"), ("Doanh số", ">100")]]);
    // Destination labels pick and order the columns.
    let ws = &mut wb.sheets[0];
    ws.set_a1("I1", text("doanh số"));
    ws.set_a1("J1", text("Nhân viên"));
    let s = AdvancedFilter {
        copy_to: Some(CellRange::parse("I1:J1").unwrap()),
        ..spec(c)
    };
    let out = apply(&mut wb, 0, &s, &Locale::VI_VN, None).unwrap();
    assert_eq!(out.extract, CellRange::parse("I1:J3"));
    assert_eq!(out.hidden, 0);
    let ws = &wb.sheets[0];
    let v = |a: &str| ws.cell_a1(a).map(|c| c.value.clone());
    assert_eq!(v("I1"), Some(text("Doanh số")));
    assert_eq!(v("I2"), Some(CellValue::Number(120.0)));
    assert_eq!(v("J2"), Some(text("An")));
    assert_eq!(v("I3"), Some(CellValue::Number(200.0)));
    assert_eq!(v("J3"), Some(text("Bảo")));
    assert_eq!(v("I4"), None);
    assert!(ws.rows.values().all(|i| !i.hidden));
    let ext = wb.defined_names.iter().find(|d| d.name == EXTRACT).unwrap();
    assert!(ext.formula.ends_with("!$I$1:$J$3"));
    // Re-running with a narrower criterion clears the old output first.
    wb.sheets[0].set_a1("G2", text(">150"));
    let out = apply(&mut wb, 0, &s, &Locale::VI_VN, None).unwrap();
    assert_eq!(out.extract, CellRange::parse("I1:J2"));
    assert_eq!(wb.sheets[0].cell_a1("I3"), None);
    // Unknown destination label; destination over the list.
    wb.sheets[0].set_a1("J1", text("Không có"));
    assert_eq!(
        apply(&mut wb, 0, &s, &Locale::VI_VN, None),
        Err(AdvError::BadExtractLabel)
    );
    let over = AdvancedFilter {
        copy_to: Some(CellRange::parse("B5").unwrap()),
        ..s
    };
    assert_eq!(
        apply(&mut wb, 0, &over, &Locale::VI_VN, None),
        Err(AdvError::ExtractOverlapsList)
    );
}

#[test]
fn copy_all_columns_with_a_row_limit() {
    let mut wb = wb();
    let c = criteria(&mut wb, &[&[("Khu vực", "Nam")]]);
    // Two-row destination: room for the labels and one data row.
    let s = AdvancedFilter {
        copy_to: Some(CellRange::parse("A12:A13").unwrap()),
        ..spec(c)
    };
    let out = apply(&mut wb, 0, &s, &Locale::VI_VN, None).unwrap();
    assert_eq!(out.matched, 3);
    assert_eq!(out.extract, CellRange::parse("A12:C13"));
    let ws = &wb.sheets[0];
    assert_eq!(ws.cell_a1("C12").unwrap().value, text("Doanh số"));
    assert_eq!(ws.cell_a1("B13").unwrap().value, text("Bình"));
    assert_eq!(ws.cell_a1("A14"), None);
}
