//! Property test: incremental recalculation gives the same values as a full
//! recalculation of a fresh workbook with the same inputs, for random sheets
//! (references, ranges, exact lookups through the hashed index, cycles) and
//! random edit sequences.

// Property bodies take the generated value by value (TestRunner signature).
#![allow(clippy::needless_pass_by_value)]

use bo_formula::Workbook;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

const ROWS: u32 = 40;
const COLS: u32 = 5;

fn col(c: u32) -> char {
    char::from(b'A' + c as u8)
}

fn cell_ref() -> impl Strategy<Value = String> {
    (0..COLS, 1..=ROWS).prop_map(|(c, r)| format!("{}{r}", col(c)))
}

fn input() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => (0i32..12).prop_map(|n| n.to_string()),
        1 => prop_oneof![Just("a".to_owned()), Just("A".to_owned()), Just(String::new())],
        1 => (cell_ref(), cell_ref()).prop_map(|(a, b)| format!("={a}+{b}")),
        1 => (cell_ref(), cell_ref()).prop_map(|(a, b)| format!("=SUM({a}:{b})")),
        1 => (0i32..12).prop_map(|n| format!("=VLOOKUP({n},$A$1:$C${ROWS},2,FALSE)")),
        1 => (0i32..12).prop_map(|n| format!("=MATCH({n},$A$1:$A${ROWS},0)")),
        1 => (0i32..12).prop_map(|n| format!("=XLOOKUP({n},$A$1:$A${ROWS},$C$1:$C${ROWS},-1,0,-1)")),
        1 => (cell_ref(), cell_ref(), cell_ref())
            .prop_map(|(a, b, c)| format!("=IF({a}>5,{b},{c})")),
        1 => (0i32..12).prop_map(|n| format!("=COUNTIF($A$1:$B${ROWS},{n})")),
    ]
}

fn edit() -> impl Strategy<Value = (u32, u32, String)> {
    (0..ROWS, 0..COLS, input())
}

fn full(inputs: &[(u32, u32, String)]) -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    for (r, c, s) in inputs {
        let _ = wb.set_input(0, *r, *c, s);
    }
    wb.recalculate_all();
    wb
}

#[test]
fn incremental_matches_full_recalc() {
    let mut runner = TestRunner::new(Config {
        cases: 64,
        ..Config::default()
    });
    let strategy = (
        proptest::collection::vec(edit(), 20..120),
        proptest::collection::vec(edit(), 1..20),
    );
    runner
        .run(&strategy, |(initial, edits)| {
            let mut wb = Workbook::with_sheets(&["Sheet1"]);
            let mut inputs: Vec<(u32, u32, String)> = Vec::new();
            for (r, c, s) in &initial {
                let _ = wb.set_input(0, *r, *c, s);
                inputs.push((*r, *c, s.clone()));
            }
            wb.recalculate();
            for (r, c, s) in &edits {
                let _ = wb.set_input(0, *r, *c, s);
                inputs.push((*r, *c, s.clone()));
                wb.recalculate();
                let reference = full(&inputs);
                for row in 0..ROWS {
                    for cc in 0..COLS {
                        let got = wb.value(0, row, cc);
                        let want = reference.value(0, row, cc);
                        if got != want {
                            return Err(TestCaseError::fail(format!(
                                "{}{}: incremental {got:?} != full {want:?}",
                                col(cc),
                                row + 1
                            )));
                        }
                    }
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Levels large enough for parallel evaluation (B, C, E) mixed with volatile
/// dynamic references (D, evaluated in order) and lookups into a computed table.
#[test]
fn large_levels_with_dynamic_references() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    let n = 3000u32;
    for r in 0..n {
        wb.set_value(0, r, 0, f64::from(r));
        // B = A * 2 and C = B + 1: two large levels.
        wb.set_formula(0, r, 1, &format!("=A{}*2", r + 1)).unwrap();
        wb.set_formula(0, r, 2, &format!("=B{}+1", r + 1)).unwrap();
        // E looks C up through the hashed index (another large level).
        wb.set_formula(0, r, 4, &format!("=MATCH({},$C$1:$C${n},0)", 2 * r + 1))
            .unwrap();
        // D reads C of the next row through INDIRECT (volatile).
        let next = (r + 1) % n + 1;
        wb.set_formula(0, r, 3, &format!("=INDIRECT(\"C{next}\")+C{}", r + 1))
            .unwrap();
    }
    wb.recalculate();
    let check = |wb: &Workbook| {
        for r in 0..n {
            let next = (r + 1) % n;
            let c = |row: u32| f64::from(row) * 2.0 + 1.0;
            assert_eq!(
                wb.value(0, r, 3),
                bo_formula::Value::Number(c(next) + c(r)),
                "D{}",
                r + 1
            );
            assert_eq!(
                wb.value(0, r, 4),
                bo_formula::Value::Number(f64::from(r + 1)),
                "E{}",
                r + 1
            );
        }
    };
    check(&wb);
    wb.set_value(0, 10, 0, 1000.0);
    wb.recalculate();
    assert_eq!(wb.value(0, 9, 3), bo_formula::Value::Number(2001.0 + 19.0));
    assert_eq!(wb.value(0, 10, 3), bo_formula::Value::Number(23.0 + 2001.0));
}
