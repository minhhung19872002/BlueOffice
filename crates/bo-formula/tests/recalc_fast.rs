//! The fast paths of recalculation give the general evaluator's results: plain arithmetic
//! programs (`simple.rs`), the cached evaluation order of a repeated edit, the column
//! cell store.

use bo_formula::{ErrorKind, Value, Workbook};

fn num(n: f64) -> Value {
    Value::Number(n)
}

#[test]
fn arithmetic_programs_match_the_evaluator() {
    let mut wb = Workbook::with_sheets(&["S"]);
    let f = |wb: &mut Workbook, a1: &str, input: &str| wb.set_a1(0, a1, input).expect("input");
    f(&mut wb, "A1", "abc");
    wb.set_value(0, 1, 0, Value::Text("3".into()));
    f(&mut wb, "A3", "TRUE");
    f(&mut wb, "A4", "#N/A");
    f(&mut wb, "A6", "=SEQUENCE(3)");
    f(&mut wb, "B1", "=A1*2");
    f(&mut wb, "B2", "=A2*2");
    f(&mut wb, "B3", "=A3*2");
    f(&mut wb, "B4", "=A4*2");
    f(&mut wb, "B5", "=A5*2+1");
    f(&mut wb, "B6", "=A7*10");
    f(&mut wb, "B7", "=1/A5");
    f(&mut wb, "B8", "=-(A5)%");
    f(&mut wb, "B9", "=1E308*10-1");
    f(&mut wb, "B10", "=B10+1");
    f(&mut wb, "B11", "=B3/4%");
    wb.recalculate();
    let v = |wb: &Workbook, a1: &str| wb.value_a1(0, a1);
    assert_eq!(v(&wb, "B1"), Value::Error(ErrorKind::Value));
    assert_eq!(v(&wb, "B2"), num(6.0));
    assert_eq!(v(&wb, "B3"), num(2.0));
    assert_eq!(v(&wb, "B4"), Value::Error(ErrorKind::NA));
    assert_eq!(v(&wb, "B5"), num(1.0));
    assert_eq!(v(&wb, "B6"), num(20.0));
    assert_eq!(v(&wb, "B7"), Value::Error(ErrorKind::Div0));
    assert_eq!(v(&wb, "B8"), num(0.0));
    assert_eq!(v(&wb, "B9"), Value::Error(ErrorKind::Num));
    assert_eq!(v(&wb, "B10"), Value::Error(ErrorKind::Calc));
    assert_eq!(v(&wb, "B11"), num(50.0));
    // Spilled values change: the reader follows.
    f(&mut wb, "A6", "=SEQUENCE(3,1,5)");
    wb.recalculate();
    assert_eq!(v(&wb, "B6"), num(60.0));
}

#[test]
fn a_repeated_edit_follows_formula_changes() {
    let mut wb = Workbook::with_sheets(&["S"]);
    wb.set_value(0, 0, 3, 1.0);
    for r in 0..3000u32 {
        wb.set_value(0, r, 0, f64::from(r));
        wb.set_formula(0, r, 1, &format!("=A{}*2+$D$1", r + 1))
            .expect("formula");
    }
    wb.recalculate();
    for i in 0..3 {
        wb.set_value(0, 0, 3, f64::from(i * 10));
        wb.recalculate();
        assert_eq!(wb.last_recalculated().len(), 3000);
        assert_eq!(wb.value(0, 2999, 1), num(2.0 * 2999.0 + f64::from(i * 10)));
    }
    // One formula stops reading D1, another starts: the next edit of D1 sees both.
    wb.set_formula(0, 5, 1, "=A6*3").expect("formula");
    wb.set_formula(0, 0, 4, "=$D$1-1").expect("formula");
    wb.recalculate();
    wb.set_value(0, 0, 3, 100.0);
    wb.recalculate();
    assert_eq!(wb.last_recalculated().len(), 3000);
    assert_eq!(wb.value(0, 5, 1), num(15.0));
    assert_eq!(wb.value(0, 0, 4), num(99.0));
    assert_eq!(wb.value(0, 6, 1), num(112.0));
    // Removing the input's readers.
    for r in 0..3000u32 {
        wb.clear(0, r, 1);
    }
    wb.recalculate();
    wb.set_value(0, 0, 3, 7.0);
    wb.recalculate();
    assert_eq!(wb.last_recalculated().len(), 1);
    assert_eq!(wb.value(0, 0, 4), num(6.0));
}

#[test]
fn far_apart_cells_and_structural_changes() {
    let mut wb = Workbook::with_sheets(&["S"]);
    wb.set_value(0, 999_999, 16_383, 4.0);
    wb.set_value(0, 300, 2, 3.0);
    wb.set_formula(0, 0, 0, "=XFD1000000*C301").expect("formula");
    wb.recalculate();
    assert_eq!(wb.value(0, 0, 0), num(12.0));
    wb.apply_structural_change(
        0,
        bo_formula::StructuralChange::InsertRows { at: 100, count: 2 },
    );
    wb.recalculate();
    assert_eq!(wb.value(0, 302, 2), num(3.0));
    assert_eq!(wb.formula(0, 0, 0), Some("=XFD1000002*C303"));
    wb.set_value(0, 302, 2, 5.0);
    wb.recalculate();
    assert_eq!(wb.value(0, 0, 0), num(20.0));
}
