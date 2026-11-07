//! Shared harness for the table-driven function tests.
//!
//! Every test table is a list of `(formula, expected)` rows evaluated against a
//! workbook prepared from `(cell, input)` rows. Each formula is stored in its
//! own cell (so dynamic arrays really spill) and the workbook is recalculated
//! once; all mismatches of a table are reported together.
#![allow(dead_code)]

use bo_formula::{parse, DateSystem, ErrorKind, Value, Workbook};

/// Expected result of a formula.
#[derive(Clone, Copy, Debug)]
pub enum X {
    /// Number, compared with a relative tolerance of 1e-9.
    N(f64),
    /// Number with an explicit absolute tolerance.
    Near(f64, f64),
    /// Text.
    T(&'static str),
    /// Boolean.
    B(bool),
    /// Error literal such as `"#N/A"`.
    E(&'static str),
    /// Array result, written as an array constant (`"{1,2;3,4}"`).
    A(&'static str),
}

#[allow(unused_imports)]
pub use X::{Near, A, B, E, N, T};

/// Column used for the formulas under test (far right of the inputs).
const FORMULA_COL: u32 = 200;
/// Rows reserved for each formula (room for spilled arrays).
const ROW_STRIDE: u32 = 40;
/// First row used for formulas.
const FIRST_ROW: u32 = 2000;

/// Options for a table run.
#[derive(Clone, Copy)]
pub struct Opts {
    /// Date system of the workbook.
    pub date_system: DateSystem,
    /// Fixed `NOW()` serial.
    pub now: Option<f64>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            date_system: DateSystem::Date1900,
            // 2024-03-15 12:00:00 (serial 45366.5, ECMA-376 §18.17.4.1 1900 system).
            now: Some(45_366.5),
        }
    }
}

/// Build a workbook with sheets `Sheet1`, `Sheet2`, `Sheet3` and the inputs.
///
/// A cell may be qualified (`"Sheet2!B3"`). Inputs follow cell entry rules:
/// `=...` is a formula, numbers/booleans/errors are parsed, `'text` forces text.
pub fn workbook(setup: &[(&str, &str)], opts: Opts) -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1", "Sheet2", "Sheet3"]);
    wb.set_date_system(opts.date_system);
    wb.set_now(opts.now);
    for (cell, input) in setup {
        let (sheet, a1) = match cell.split_once('!') {
            Some((s, a)) => (
                match s {
                    "Sheet1" => 0,
                    "Sheet2" => 1,
                    "Sheet3" => 2,
                    _ => panic!("unknown sheet {s}"),
                },
                a,
            ),
            None => (0, *cell),
        };
        if let Some(text) = input.strip_prefix('\'') {
            let addr = bo_formula::reference::parse_a1_cell(a1).expect("valid cell");
            wb.set_value(sheet, addr.row, addr.col, Value::Text(text.to_owned()));
        } else {
            wb.set_a1(sheet, a1, input).expect("valid setup input");
        }
    }
    wb
}

fn expected_array(src: &str) -> Value {
    let wb = Workbook::with_sheets(&["Sheet1"]);
    wb.evaluate(0, 0, 0, src).expect("valid expected array")
}

fn close(a: f64, b: f64, tol: Option<f64>) -> bool {
    match tol {
        Some(t) => (a - b).abs() <= t,
        None => a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-300),
    }
}

fn values_match(got: &Value, want: &Value, tol: Option<f64>) -> bool {
    match (got, want) {
        (Value::Number(a), Value::Number(b)) => close(*a, *b, tol),
        (Value::Array(a), Value::Array(b)) => {
            a.rows() == b.rows()
                && a.cols() == b.cols()
                && a.data().iter().zip(b.data()).all(|(x, y)| values_match(x, y, tol))
        }
        _ => got == want,
    }
}

fn want_value(x: X) -> (Value, Option<f64>) {
    match x {
        X::N(n) => (Value::Number(n), None),
        X::Near(n, t) => (Value::Number(n), Some(t)),
        X::T(s) => (Value::Text(s.to_owned()), None),
        X::B(b) => (Value::Bool(b), None),
        X::E(e) => (
            Value::Error(ErrorKind::parse(e).unwrap_or_else(|| panic!("bad error literal {e}"))),
            None,
        ),
        X::A(src) => (expected_array(src), None),
    }
}

/// Collect the full result of the formula at (row, col): the anchor value or
/// the whole spilled array.
fn result_at(wb: &Workbook, row: u32, col: u32) -> Value {
    match wb.spill_range(0, row, col) {
        Some(area) => {
            let mut data = Vec::new();
            for r in area.r1..=area.r2 {
                for c in area.c1..=area.c2 {
                    data.push(wb.value(0, r, c));
                }
            }
            Value::from(bo_formula::Array::new(
                area.rows() as usize,
                area.cols() as usize,
                data,
            ))
        }
        None => wb.value(0, row, col),
    }
}

/// Run a table with custom options.
pub fn check_with(setup: &[(&str, &str)], cases: &[(&str, X)], opts: Opts) {
    let mut wb = workbook(setup, opts);
    for (i, (formula, _)) in cases.iter().enumerate() {
        let row = FIRST_ROW + i as u32 * ROW_STRIDE;
        if let Err(e) = wb.set_formula(0, row, FORMULA_COL, formula) {
            panic!("parse error in {formula}: {e}");
        }
    }
    wb.recalculate();
    let mut failures = Vec::new();
    for (i, (formula, want)) in cases.iter().enumerate() {
        let row = FIRST_ROW + i as u32 * ROW_STRIDE;
        let got = result_at(&wb, row, FORMULA_COL);
        let (want_v, tol) = want_value(*want);
        if !values_match(&got, &want_v, tol) {
            failures.push(format!("  {formula}\n      got  {got:?}\n      want {want:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// Run a table with default options (1900 date system, fixed now).
pub fn check(setup: &[(&str, &str)], cases: &[(&str, X)]) {
    check_with(setup, cases, Opts::default());
}

/// Assert that every formula parses (helper for syntax tables).
pub fn parses(formula: &str) -> bool {
    parse(formula).is_ok()
}
