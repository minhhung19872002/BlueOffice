//! Formula auditing (SHEET-AUDIT-001): direct precedents / dependents,
//! step-by-step evaluation, "precision as displayed" rounding and iterative
//! calculation limits (ECMA-376 Part 1, §18.2.2 calcPr `iterateCount`,
//! `iterateDelta`, `fullPrecision`). Expected values are worked out by hand.

use bo_formula::{Area, CellId, ErrorKind, IterativeConfig, Value, Workbook};

fn num(n: f64) -> Value {
    Value::Number(n)
}

#[test]
fn precedents_and_dependents_are_direct_links() {
    let mut wb = Workbook::with_sheets(&["Sheet1", "Sheet2"]);
    wb.set_a1(0, "A1", "1").unwrap();
    wb.set_a1(0, "C1", "2").unwrap();
    wb.set_a1(0, "D2", "3").unwrap();
    wb.set_a1(0, "B1", "=A1+SUM(C1:D2)").unwrap();
    wb.set_a1(0, "E1", "=B1*2").unwrap();
    wb.set_a1(1, "A1", "=Sheet1!B1").unwrap();
    wb.recalculate();
    // B1 reads A1 and the block C1:D2.
    assert_eq!(
        wb.direct_precedents(0, 0, 1),
        vec![Area::cell(0, 0, 0), Area::new(0, 0, 2, 1, 3)]
    );
    // B1 is read by E1 and by Sheet2!A1.
    assert_eq!(
        wb.direct_dependents(0, 0, 1),
        vec![CellId::new(0, 0, 4), CellId::new(1, 0, 0)]
    );
    // A cell inside the block (D2) has B1 as its only dependent.
    assert_eq!(wb.direct_dependents(0, 1, 3), vec![CellId::new(0, 0, 1)]);
    // Second level from A1: A1 → B1 → {E1, Sheet2!A1}.
    assert_eq!(wb.direct_dependents(0, 0, 0), vec![CellId::new(0, 0, 1)]);
    // Constants have no precedents; E1 has no dependents.
    assert_eq!(wb.direct_precedents(0, 0, 0), Vec::<Area>::new());
    assert_eq!(wb.direct_dependents(0, 0, 4), Vec::<CellId>::new());
    // A whole-column reference is clipped to the used rows (1–2: D2 is used).
    wb.set_a1(0, "F1", "=SUM(A:A)").unwrap();
    assert_eq!(
        wb.direct_precedents(0, 0, 5),
        vec![Area::new(0, 0, 0, 1, 0)]
    );
}

fn steps(wb: &Workbook, a1: (u32, u32)) -> Vec<(String, String, String, String)> {
    let t = wb.trace(0, a1.0, a1.1).expect("formula cell");
    t.steps
        .into_iter()
        .map(|s| (s.before, s.part, s.after, s.shown))
        .collect()
}

fn st(b: &str, p: &str, a: &str, v: &str) -> (String, String, String, String) {
    (b.into(), p.into(), a.into(), v.into())
}

#[test]
fn evaluate_formula_steps_follow_the_taken_branch() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "5").unwrap();
    wb.set_a1(0, "A2", "2").unwrap();
    wb.set_a1(0, "A3", "1").unwrap();
    wb.set_a1(0, "B1", "3").unwrap();
    wb.set_a1(0, "C1", "=IF(A1>B1,SUM(A1:A3)*2,\"x\")").unwrap();
    wb.recalculate();
    // A1 = 5, B1 = 3: 5>3 is TRUE, SUM(5,2,1) = 8, 8*2 = 16; "x" is never evaluated.
    assert_eq!(
        steps(&wb, (0, 2)),
        vec![
            st("IF(", "A1", ">B1,SUM(A1:A3)*2,\"x\")", "5"),
            st("IF(5>", "B1", ",SUM(A1:A3)*2,\"x\")", "3"),
            st("IF(", "5>3", ",SUM(A1:A3)*2,\"x\")", "TRUE"),
            st("IF(TRUE,", "SUM(A1:A3)", "*2,\"x\")", "8"),
            st("IF(TRUE,", "8*2", ",\"x\")", "16"),
            st("", "IF(TRUE,16,\"x\")", "", "16"),
        ]
    );
    let t = wb.trace(0, 0, 2).unwrap();
    assert_eq!(t.result, num(16.0));
    assert_eq!(t.shown, "16");
    // The false branch: only "x" is left to evaluate.
    wb.set_a1(0, "B1", "9").unwrap();
    wb.recalculate();
    assert_eq!(
        steps(&wb, (0, 2)),
        vec![
            st("IF(", "A1", ">B1,SUM(A1:A3)*2,\"x\")", "5"),
            st("IF(5>", "B1", ",SUM(A1:A3)*2,\"x\")", "9"),
            st("IF(", "5>9", ",SUM(A1:A3)*2,\"x\")", "FALSE"),
            st("", "IF(FALSE,SUM(A1:A3)*2,\"x\")", "", "\"x\""),
        ]
    );
}

#[test]
fn evaluate_formula_offers_step_in_for_formula_cells() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "=3+4").unwrap();
    wb.set_a1(0, "B1", "2").unwrap();
    wb.set_a1(0, "C1", "=A1*B1").unwrap();
    wb.recalculate();
    let t = wb.trace(0, 0, 2).unwrap();
    assert_eq!(t.steps.len(), 3);
    // A1 holds a formula: stepping in is possible; B1 is a constant.
    assert_eq!(t.steps[0].cell, Some(CellId::new(0, 0, 0)));
    assert_eq!(t.steps[0].shown, "7");
    assert_eq!(t.steps[1].cell, None);
    assert_eq!(
        (t.steps[2].part.as_str(), t.steps[2].shown.as_str()),
        ("7*2", "14")
    );
    // Stepping in is tracing the referenced cell.
    let inner = wb.trace(0, 0, 0).unwrap();
    assert_eq!(inner.steps.len(), 1);
    assert_eq!(inner.steps[0].part, "3+4");
    assert_eq!(inner.result, num(7.0));
    // A constant has nothing to trace.
    assert!(wb.trace(0, 0, 1).is_none());
}

#[test]
fn evaluate_formula_error_branches_and_parentheses() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "=IFERROR(1/0,\"bad\")").unwrap();
    wb.set_a1(0, "A2", "=(1+2)*3").unwrap();
    wb.recalculate();
    assert_eq!(
        steps(&wb, (0, 0)),
        vec![
            st("IFERROR(", "1/0", ",\"bad\")", "#DIV/0!"),
            st("", "IFERROR(#DIV/0!,\"bad\")", "", "\"bad\""),
        ]
    );
    // The parentheses around 1+2 stay outside the underlined part.
    assert_eq!(
        steps(&wb, (1, 0)),
        vec![st("(", "1+2", ")*3", "3"), st("", "3*3", "", "9")]
    );
    let t = wb.trace(0, 0, 0).unwrap();
    assert_eq!(t.result, Value::Text("bad".into()));
    assert_eq!(t.steps[0].value, Value::Error(ErrorKind::Div0));
}

#[test]
fn precision_as_displayed_rounds_what_formulas_read() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "1.234").unwrap();
    wb.set_a1(0, "B1", "=A1*10").unwrap();
    wb.set_a1(0, "C1", "2.5").unwrap();
    wb.set_a1(0, "D1", "-2.5").unwrap();
    wb.set_a1(0, "E1", "1250").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B1"), num(12.34));
    // A1 shown with two decimals: formulas read 1.23, so B1 = 12.3.
    wb.set_display_precision(0, 0, 0, Some(2));
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), num(1.23));
    assert_eq!(wb.value_a1(0, "B1"), num(12.3));
    // B1 shown without decimals reads 12; halves round away from zero.
    wb.set_display_precision(0, 0, 1, Some(0));
    wb.set_display_precision(0, 0, 2, Some(0));
    wb.set_display_precision(0, 0, 3, Some(0));
    wb.set_display_precision(0, 0, 4, Some(-2));
    assert_eq!(wb.value_a1(0, "B1"), num(12.0));
    assert_eq!(wb.value_a1(0, "C1"), num(3.0));
    assert_eq!(wb.value_a1(0, "D1"), num(-3.0));
    assert_eq!(wb.value_a1(0, "E1"), num(1300.0));
    // Back to full precision.
    wb.clear_display_precision();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B1"), num(12.34));
}

#[test]
fn iteration_stops_at_max_change_or_count() {
    // x = (x + 10) / 2 from 0: after k iterations x = 10 - 10/2^k and the
    // last change is 10/2^k. 10/2^13 ≈ 0.00122 ≥ 0.001 > 10/2^14 ≈ 0.00061,
    // so the default settings stop after 14 iterations.
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_iterative(Some(IterativeConfig::default()));
    wb.set_a1(0, "A1", "=(A1+10)/2").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), num(10.0 - 10.0 / 16384.0));
    assert_eq!(wb.circular_references(), vec![CellId::new(0, 0, 0)]);
    // Three iterations from 0: 5, 7.5, 8.75.
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_iterative(Some(IterativeConfig {
        max_iterations: 3,
        max_change: 0.001,
    }));
    wb.set_a1(0, "A1", "=(A1+10)/2").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), num(8.75));
    // A large max change stops after the first iteration: 5.
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_iterative(Some(IterativeConfig {
        max_iterations: 100,
        max_change: 10.0,
    }));
    wb.set_a1(0, "A1", "=(A1+10)/2").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), num(5.0));
    // Without iteration the cycle is an error.
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "=(A1+10)/2").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), Value::Error(ErrorKind::Calc));
}
