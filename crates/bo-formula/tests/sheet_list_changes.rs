//! Sheets appended / removed at the end and defined names replaced without
//! reloading the workbook (SHEET-PERF-002): the formulas naming them are
//! recalculated, and the results equal a workbook built from scratch.
#![allow(clippy::unwrap_used)]

use bo_formula::{Value, Workbook};

fn num(wb: &Workbook, sheet: usize, a1: &str) -> Value {
    wb.value_a1(sheet, a1)
}

// source: ECMA-376 Part 1, §18.17.2.3 references to other sheets
#[test]
fn appended_sheet_is_read_by_formulas_naming_it() {
    let mut wb = Workbook::with_sheets(&["Data"]);
    wb.set_a1(0, "A1", "=New!B2*2").unwrap();
    wb.set_a1(0, "A2", "=SUM(New!B1:B3)").unwrap();
    wb.set_a1(0, "A3", "=5").unwrap();
    wb.recalculate();
    assert_eq!(num(&wb, 0, "A1"), Value::Error(bo_formula::ErrorKind::Ref));
    let s = wb.add_sheet("New");
    wb.set_a1(s, "B2", "21").unwrap();
    wb.set_a1(s, "B3", "1").unwrap();
    wb.refresh_sheet_references(&["New".to_owned()]);
    wb.recalculate();
    assert_eq!(num(&wb, 0, "A1"), Value::Number(42.0));
    assert_eq!(num(&wb, 0, "A2"), Value::Number(22.0));
    // Dependencies are registered: editing the new sheet recalculates.
    wb.set_a1(s, "B2", "1").unwrap();
    wb.recalculate();
    assert_eq!(num(&wb, 0, "A1"), Value::Number(2.0));
    // Removed again: back to #REF!.
    wb.truncate_sheets(1);
    wb.recalculate();
    assert_eq!(wb.sheet_count(), 1);
    assert_eq!(num(&wb, 0, "A1"), Value::Error(bo_formula::ErrorKind::Ref));
    assert_eq!(num(&wb, 0, "A3"), Value::Number(5.0));
}

// source: ECMA-376 Part 1, §18.2.5 definedName
#[test]
fn replaced_names_recalculate_their_users_only() {
    let mut wb = Workbook::with_sheets(&["S"]);
    wb.define_name("Rate", None, "S!$B$1").unwrap();
    wb.set_a1(0, "B1", "2").unwrap();
    wb.set_a1(0, "B2", "10").unwrap();
    wb.set_a1(0, "A1", "=Rate*100").unwrap();
    wb.set_a1(0, "A2", "=ROWS(Extract)").unwrap();
    wb.set_a1(0, "A3", "=B2+1").unwrap();
    wb.recalculate();
    assert_eq!(num(&wb, 0, "A1"), Value::Number(200.0));
    wb.replace_names(&[
        (None, "Rate".into(), "S!$B$2".into()),
        (Some(0), "Extract".into(), "S!$C$1:$C$4".into()),
    ]);
    wb.recalculate();
    assert_eq!(num(&wb, 0, "A1"), Value::Number(1000.0));
    assert_eq!(num(&wb, 0, "A2"), Value::Number(4.0));
    // The formula not using a name was not recalculated.
    assert!(!wb
        .last_recalculated()
        .iter()
        .any(|id| (id.row, id.col) == (2, 0)));
    // New dependencies: B2 now feeds A1.
    wb.set_a1(0, "B2", "3").unwrap();
    wb.recalculate();
    assert_eq!(num(&wb, 0, "A1"), Value::Number(300.0));
}
