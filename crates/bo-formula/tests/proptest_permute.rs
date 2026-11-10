//! Property test (SHEET-PERF-002): permuting rows of a block in place
//! (`Workbook::permute_rows`, a sort) and recalculating gives the same values as a
//! fresh workbook holding the permuted inputs — with and without formulas inside
//! the moved block.

#![allow(clippy::needless_pass_by_value, clippy::unwrap_used)]

use bo_formula::Workbook;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

const ROWS: u32 = 30;

fn block_input() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => (0i32..20).prop_map(|n| n.to_string()),
        1 => prop_oneof![Just("x".to_owned()), Just(String::new())],
        1 => (1u32..=ROWS).prop_map(|r| format!("=$G$1+{r}")),
    ]
}

/// Formulas outside the block (columns E:F) reading it.
fn reader() -> impl Strategy<Value = String> {
    prop_oneof![
        (1u32..=ROWS, 0u32..3).prop_map(|(r, c)| format!("={}{r}*2", char::from(b'A' + c as u8))),
        Just(format!("=SUM(A1:C{ROWS})")),
        (0i32..20).prop_map(|n| format!("=MATCH({n},$A$1:$A${ROWS},0)")),
        (1u32..=ROWS).prop_map(|r| format!("=INDEX(B1:B{ROWS},{r})&\"!\"")),
    ]
}

fn build(block: &[Vec<String>], readers: &[String]) -> Workbook {
    let mut wb = Workbook::with_sheets(&["S"]);
    wb.set_input(0, 0, 6, "5").unwrap();
    for (r, row) in block.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            wb.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    for (i, f) in readers.iter().enumerate() {
        wb.set_input(0, i as u32, 4 + (i as u32 % 2), f).unwrap();
    }
    wb
}

#[test]
fn permuted_rows_match_a_fresh_workbook() {
    // Constants only (values move through the map's entries), then with formulas.
    check(false);
    check(true);
}

fn check(formulas: bool) {
    let mut runner = TestRunner::new(Config {
        cases: 96,
        ..Config::default()
    });
    let strategy = (
        prop::collection::vec(prop::collection::vec(block_input(), 3), ROWS as usize),
        prop::collection::vec(reader(), 1..12),
        Just(()).prop_perturb(|(), mut rng| {
            let mut p: Vec<u32> = (0..ROWS - 1).collect();
            for i in (1..p.len()).rev() {
                let j = (rng.next_u32() as usize) % (i + 1);
                p.swap(i, j);
            }
            p
        }),
    );
    runner
        .run(&strategy, |(mut block, readers, perm)| {
            if !formulas {
                for v in block.iter_mut().flatten().filter(|v| v.starts_with('=')) {
                    *v = "7".into();
                }
            }
            let mut wb = build(&block, &readers);
            wb.recalculate_all();
            // Rows 1..ROWS (0-based) move: row 1 + n receives row 1 + perm[n].
            let start = 1u32;
            assert!(wb.permute_rows(0, start, 0, 2, &perm));
            let mut moved = block.clone();
            for (n, &from) in perm.iter().enumerate() {
                moved[start as usize + n].clone_from(&block[(start + from) as usize]);
            }
            // The caller sets the moved formulas again (here: same text).
            for (n, &from) in perm.iter().enumerate() {
                if n as u32 == from {
                    continue;
                }
                for c in 0..3u32 {
                    let v = &moved[start as usize + n][c as usize];
                    if v.starts_with('=') {
                        wb.set_input(0, start + n as u32, c, v).unwrap();
                    }
                }
            }
            wb.recalculate();
            let mut fresh = build(&moved, &readers);
            fresh.recalculate_all();
            for r in 0..ROWS {
                for c in 0..7 {
                    let (a, b) = (wb.value(0, r, c), fresh.value(0, r, c));
                    if a != b {
                        return Err(TestCaseError::fail(format!("({r},{c}): {a:?} != {b:?}")));
                    }
                }
            }
            Ok(())
        })
        .unwrap();
}
