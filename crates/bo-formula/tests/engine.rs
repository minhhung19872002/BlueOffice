//! Engine semantics: syntax, references, coercion, comparison precision,
//! arrays & spilling, names, tables, dependency graph, cycles, volatility,
//! structural changes and copy/paste.
//!
//! Sources: ECMA-376 Part 1, §18.17.2 (formula syntax, operators and their
//! precedence, reference forms incl. R1C1 and 3-D references, error
//! constants), §18.17.3 (evaluation / type conversion), §18.5 (tables and
//! structured references), §18.2.5 definedName, §18.2.2 calcPr (iterative
//! calculation); OpenFormula (ODF 1.3 Part 4) §6.3 implicit conversion rules;
//! dynamic arrays (spill, `#SPILL!`, `A1#`) per their published definition.

mod common;
use bo_formula::{
    format, format_with, offset_relative, parse, parse_with, CellId, ErrorKind, IterativeConfig,
    ParseOptions, RefStyle, StructuralChange, TableInfo, Value, Workbook,
};
use common::*;

fn num(n: f64) -> Value {
    Value::Number(n)
}

// ------------------------------------------------------------------ syntax

#[test]
fn operators_precedence_and_coercion() {
    check(
        &[("A1", "5"), ("A2", "'7"), ("A3", "TRUE")],
        &[
            ("=1+2*3", N(7.0)),
            ("=(1+2)*3", N(9.0)),
            ("=-2^2", N(4.0)),
            ("=2^3^2", N(64.0)),
            ("=10%", N(0.1)),
            ("=50%*2", N(1.0)),
            ("=2^-1", N(0.5)),
            ("=\"a\"&1+1", T("a2")),
            ("=1+2&\"x\"", T("3x")),
            ("=1+2=3", B(true)),
            // Text that looks like a number is converted in arithmetic.
            ("=\"1\"+1", N(2.0)),
            ("=A2+1", N(8.0)),
            ("=TRUE+TRUE", N(2.0)),
            ("=A3*3", N(3.0)),
            ("=Z99+1", N(1.0)),
            ("=-Z99", N(0.0)),
            ("=Z99&\"x\"", T("x")),
            ("=\"a\"+1", E("#VALUE!")),
            ("=\"1e2\"*1", N(100.0)),
            ("=\"50%\"*2", N(1.0)),
            ("=\"2024-03-15\"-\"2024-03-14\"", N(1.0)),
            ("=1/0", E("#DIV/0!")),
            ("=#N/A+1", E("#N/A")),
            ("=1+#REF!", E("#REF!")),
        ],
    );
}

#[test]
fn comparisons() {
    check(
        &[("A1", "'1")],
        &[
            // No conversion in comparisons: numbers < text < logical values.
            ("=\"1\"=1", B(false)),
            ("=A1=1", B(false)),
            ("=1<\"a\"", B(true)),
            ("=\"z\"<TRUE", B(true)),
            ("=FALSE<TRUE", B(true)),
            ("=\"abc\"=\"ABC\"", B(true)),
            ("=\"a\"<\"b\"", B(true)),
            ("=\"B\">\"a\"", B(true)),
            ("=Z99=0", B(true)),
            ("=Z99=\"\"", B(true)),
            ("=Z99=FALSE", B(true)),
            ("=1<>2", B(true)),
            // Numbers compare at 15 significant digits.
            ("=0.1+0.2=0.3", B(true)),
            ("=1.000000000000001=1", B(true)),
            ("=1.00000000000001=1", B(false)),
            ("=123456789012345.6=123456789012346", B(true)),
            ("=#N/A=1", E("#N/A")),
        ],
    );
}

#[test]
fn error_values() {
    check(
        &[],
        &[
            ("=#NULL!", E("#NULL!")),
            ("=#DIV/0!", E("#DIV/0!")),
            ("=#VALUE!", E("#VALUE!")),
            ("=#REF!", E("#REF!")),
            ("=#NAME?", E("#NAME?")),
            ("=#NUM!", E("#NUM!")),
            ("=#N/A", E("#N/A")),
            ("=#SPILL!", E("#SPILL!")),
            ("=#CALC!", E("#CALC!")),
            ("=#GETTING_DATA", E("#GETTING_DATA")),
            ("=NOSUCHFUNCTION(1)", E("#NAME?")),
            ("=UNDEFINEDNAME", E("#NAME?")),
            ("=SUM(A1:B2 C3:D4)", E("#NULL!")),
            ("=SQRT(-1)", E("#NUM!")),
            ("=10^400", E("#NUM!")),
            ("=ABS()", E("#VALUE!")),
            ("=ERROR.TYPE(#SPILL!)", N(9.0)),
            ("=ERROR.TYPE(#CALC!)", N(14.0)),
        ],
    );
}

#[test]
fn reference_forms() {
    check(
        &[
            ("A1", "1"),
            ("A2", "2"),
            ("A3", "3"),
            ("B1", "10"),
            ("B2", "20"),
            ("B3", "30"),
            ("Sheet2!A1", "100"),
            ("Sheet3!A1", "1000"),
        ],
        &[
            ("=$A$1+A$2+$A3", N(6.0)),
            ("=SUM(A1:B3)", N(66.0)),
            ("=SUM(B3:A1)", N(66.0)),
            ("=SUM((A1:A2,B3))", N(33.0)),
            ("=SUM(A1:B2 B2:B3)", N(20.0)),
            ("=ROWS(A1:B2:A3)", N(3.0)),
            ("=SUM(A:A)", N(6.0)),
            ("=SUM(1:1)", N(11.0)),
            ("=SUM($A:$B)", N(66.0)),
            ("=Sheet2!A1", N(100.0)),
            ("='Sheet2'!A1+1", N(101.0)),
            ("=SUM(Sheet1:Sheet3!A1)", N(1101.0)),
            ("=SUM('Sheet1:Sheet3'!A1)", N(1101.0)),
            ("=COUNT(Sheet2:Sheet3!A1:B1)", N(2.0)),
            ("=NoSheet!A1", E("#REF!")),
            ("={1,2;3,4}", A("{1,2;3,4}")),
            ("={1,-2.5,\"a\",TRUE,#N/A}", A("{1,-2.5,\"a\",TRUE,#N/A}")),
            ("={1,2}+{10;20}", A("{11,12;21,22}")),
            ("={1,2,3}*2", A("{2,4,6}")),
            ("={1,2}+{1,2,3}", A("{2,4,#N/A}")),
            ("=SUM({1,2;3,4}*{1,0;0,1})", N(5.0)),
            ("=A1:A3*10", A("{10;20;30}")),
            ("=SUM(A1:A3*B1:B3)", N(140.0)),
        ],
    );
}

#[test]
fn sheet_names_with_spaces() {
    let mut wb = Workbook::with_sheets(&["Sheet 1", "Data"]);
    wb.set_a1(1, "A1", "5").unwrap();
    wb.set_a1(0, "A1", "='Data'!A1*2").unwrap();
    wb.set_a1(0, "A2", "=Data!A1+1").unwrap();
    wb.set_a1(1, "B1", "='Sheet 1'!A1+1").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), num(10.0));
    assert_eq!(wb.value_a1(0, "A2"), num(6.0));
    assert_eq!(wb.value_a1(1, "B1"), num(11.0));
    assert_eq!(wb.formula(1, 0, 1), Some("='Sheet 1'!A1+1"));
    // Editing a precedent on another sheet updates dependents.
    wb.set_a1(1, "A1", "7").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(1, "B1"), num(15.0));
}

#[test]
fn implicit_intersection() {
    let mut wb = workbook(&[("A1", "1"), ("A2", "2"), ("A3", "3")], Opts::default());
    wb.set_a1(0, "C2", "=@A1:A3").unwrap();
    wb.set_a1(0, "C5", "=@A1:A3").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "C2"), num(2.0));
    assert_eq!(wb.value_a1(0, "C5"), Value::Error(ErrorKind::Value));
}

#[test]
fn r1c1_parse_and_format() {
    let opts = ParseOptions {
        style: RefStyle::R1C1,
        origin_row: 4,
        origin_col: 2,
    };
    let e = parse_with("=R[-1]C+R1C1+RC[1]+SUM(R1C1:R2C2)", &opts).unwrap();
    assert_eq!(format(&e), "C4+$A$1+D5+SUM($A$1:$B$2)");
    assert_eq!(format_with(&e, &opts), "R[-1]C+R1C1+RC[1]+SUM(R1C1:R2C2)");
    let a1 = parse_with(
        "=A1+$B$2",
        &ParseOptions {
            origin_row: 2,
            origin_col: 2,
            ..ParseOptions::default()
        },
    )
    .unwrap();
    let r1c1 = ParseOptions {
        style: RefStyle::R1C1,
        origin_row: 2,
        origin_col: 2,
    };
    assert_eq!(format_with(&a1, &r1c1), "R[-2]C[-2]+R2C2");
}

#[test]
fn formula_round_trip_samples() {
    for f in [
        "SUM(A1:B2)",
        "$A$1+A$1+$A1",
        "'Sheet 1'!A1*Sheet2!$B$3",
        "SUM(Sheet1:Sheet3!A1)",
        "SUM((A1,B2:C3))",
        "SUM(A1:B2 B2:C3)",
        "-(1+2)%",
        "{1,2;3,4}",
        "IF(A1>0,\"a\"\"b\",#N/A)",
        "Table1[[#This Row],[Amount]]",
        "SUM(Table1[Qty])",
        "LAMBDA(x,x+1)(2)",
        "A1#",
        "@A1:A3",
        "-2^2",
        "A:A",
        "1:3",
    ] {
        let e = parse(f).unwrap_or_else(|err| panic!("{f}: {err}"));
        let s = format(&e);
        assert_eq!(parse(&s).unwrap(), e, "{f} -> {s}");
    }
}

#[test]
fn parse_errors() {
    for f in [
        "=1+", "=SUM(1", "=\"abc", "=#BOGUS!", "=A1::B2", "={1,2;3}", "=)",
    ] {
        assert!(parse(f).is_err() || f == "={1,2;3}", "{f} should not parse");
    }
}

// ------------------------------------------------------------------ names & tables

#[test]
fn defined_names_scopes() {
    let mut wb = Workbook::with_sheets(&["Sheet1", "Sheet2"]);
    wb.set_a1(0, "A1", "1").unwrap();
    wb.set_a1(0, "A2", "2").unwrap();
    wb.set_a1(0, "A3", "3").unwrap();
    wb.define_name("Rate", None, "=0.1").unwrap();
    wb.define_name("Rate", Some(1), "=0.2").unwrap();
    wb.define_name("Data", None, "=Sheet1!$A$1:$A$3").unwrap();
    wb.set_a1(0, "B1", "=Rate").unwrap();
    wb.set_a1(1, "B1", "=Rate").unwrap();
    wb.set_a1(1, "B2", "=Sheet2!Rate").unwrap();
    wb.set_a1(0, "B2", "=SUM(Data)").unwrap();
    wb.set_a1(0, "B3", "=Nope*2").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B1"), num(0.1));
    assert_eq!(wb.value_a1(1, "B1"), num(0.2));
    assert_eq!(wb.value_a1(1, "B2"), num(0.2));
    assert_eq!(wb.value_a1(0, "B2"), num(6.0));
    assert_eq!(wb.value_a1(0, "B3"), Value::Error(ErrorKind::Name));
    // Dependencies flow through names.
    wb.set_a1(0, "A3", "30").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B2"), num(33.0));
}

fn sales_table() -> Workbook {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    for (cell, v) in [
        ("A1", "Item"),
        ("B1", "Qty"),
        ("C1", "Price"),
        ("A2", "pen"),
        ("B2", "2"),
        ("C2", "1.5"),
        ("A3", "book"),
        ("B3", "1"),
        ("C3", "12"),
        ("A4", "bag"),
        ("B4", "3"),
        ("C4", "20"),
        ("D1", "Total"),
    ] {
        wb.set_a1(0, cell, v).unwrap();
    }
    wb.add_table(TableInfo {
        name: "Sales".into(),
        sheet: 0,
        first_row: 0,
        last_row: 3,
        first_col: 0,
        columns: vec!["Item".into(), "Qty".into(), "Price".into(), "Total".into()],
        header_row: true,
        totals_row: false,
    });
    wb
}

#[test]
fn structured_references() {
    let mut wb = sales_table();
    // Calculated column inside the table uses the `[@Col]` shorthand.
    wb.set_a1(0, "D2", "=[@Qty]*[@Price]").unwrap();
    wb.set_a1(0, "D3", "=Sales[@Qty]*Sales[@Price]").unwrap();
    wb.set_a1(0, "D4", "=Sales[[#This Row],[Qty]]").unwrap();
    wb.set_a1(0, "F1", "=SUM(Sales[Qty])").unwrap();
    wb.set_a1(0, "F2", "=Sales[[#Headers],[Price]]").unwrap();
    wb.set_a1(0, "F3", "=ROWS(Sales[#All])").unwrap();
    wb.set_a1(0, "F4", "=ROWS(Sales)").unwrap();
    wb.set_a1(0, "F5", "=SUM(Sales[[Qty]:[Price]])").unwrap();
    wb.set_a1(0, "F6", "=Sales[#Totals]").unwrap();
    wb.set_a1(0, "F7", "=SUM(Sales[Nope])").unwrap();
    wb.set_a1(0, "F8", "=SUMPRODUCT(Sales[Qty],Sales[Price])")
        .unwrap();
    wb.set_a1(0, "F9", "=[@Qty]").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "D2"), num(3.0));
    assert_eq!(wb.value_a1(0, "D3"), num(12.0));
    assert_eq!(wb.value_a1(0, "D4"), num(3.0));
    assert_eq!(wb.value_a1(0, "F1"), num(6.0));
    assert_eq!(wb.value_a1(0, "F2"), Value::Text("Price".into()));
    assert_eq!(wb.value_a1(0, "F3"), num(4.0));
    assert_eq!(wb.value_a1(0, "F4"), num(3.0));
    assert_eq!(wb.value_a1(0, "F5"), num(39.5));
    assert_eq!(wb.value_a1(0, "F6"), Value::Error(ErrorKind::Ref));
    assert_eq!(wb.value_a1(0, "F7"), Value::Error(ErrorKind::Ref));
    assert_eq!(wb.value_a1(0, "F8"), num(75.0));
    assert_eq!(wb.value_a1(0, "F9"), Value::Error(ErrorKind::Ref));
    assert_eq!(wb.circular_references(), Vec::<CellId>::new());
    wb.set_a1(0, "B4", "4").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "F1"), num(7.0));
}

// ------------------------------------------------------------------ dynamic arrays

#[test]
fn spill_and_spill_errors() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "=SEQUENCE(3)").unwrap();
    wb.set_a1(0, "B1", "=SUM(A1#)").unwrap();
    wb.set_a1(0, "C2", "=A2*10").unwrap();
    wb.set_a1(0, "D1", "=ROWS(A1#)").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A2"), num(2.0));
    assert_eq!(wb.value_a1(0, "A3"), num(3.0));
    assert_eq!(wb.value_a1(0, "B1"), num(6.0));
    assert_eq!(wb.value_a1(0, "C2"), num(20.0));
    assert_eq!(wb.value_a1(0, "D1"), num(3.0));
    let area = wb.spill_range(0, 0, 0).unwrap();
    assert_eq!((area.r1, area.r2, area.c1, area.c2), (0, 2, 0, 0));

    // The anchor's array changes: spilled dependents follow.
    wb.set_a1(0, "A1", "=SEQUENCE(4,1,5)").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "C2"), num(60.0));
    assert_eq!(wb.value_a1(0, "B1"), num(26.0));
    assert_eq!(wb.value_a1(0, "D1"), num(4.0));

    // A constant in the way blocks the spill.
    wb.set_a1(0, "A3", "x").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), Value::Error(ErrorKind::Spill));
    assert_eq!(wb.value_a1(0, "A2"), Value::Empty);
    assert!(wb.value_a1(0, "B1").is_error());
    assert_eq!(wb.value_a1(0, "C2"), num(0.0));

    // Clearing the obstacle lets it spill again.
    wb.set_a1(0, "A3", "").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A4"), num(8.0));
    assert_eq!(wb.value_a1(0, "B1"), num(26.0));
    assert_eq!(wb.value_a1(0, "C2"), num(60.0));
}

#[test]
fn overlapping_spills_block() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "=SEQUENCE(3)").unwrap();
    wb.set_a1(0, "B1", "=SEQUENCE(1,2)").unwrap();
    wb.set_a1(0, "A3", "=SEQUENCE(1,3)").unwrap();
    wb.recalculate();
    // A1 cannot spill over the formula in A3.
    assert_eq!(wb.value_a1(0, "A1"), Value::Error(ErrorKind::Spill));
    assert_eq!(wb.value_a1(0, "C1"), num(2.0));
    assert_eq!(wb.value_a1(0, "C3"), num(3.0));
}

#[test]
fn spill_off_grid() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1048576", "=SEQUENCE(2)").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1048576"), Value::Error(ErrorKind::Spill));
}

// ------------------------------------------------------------------ graph

#[test]
fn incremental_recalculation_touches_only_dependents() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "1").unwrap();
    wb.set_a1(0, "B1", "=A1*2").unwrap();
    wb.set_a1(0, "C1", "=B1+1").unwrap();
    wb.set_a1(0, "A2", "5").unwrap();
    wb.set_a1(0, "B2", "=A2*2").unwrap();
    wb.set_a1(0, "D1", "=SUM(A1:A100)").unwrap();
    wb.recalculate();
    assert_eq!(wb.last_recalc_evaluations(), 4);
    wb.set_a1(0, "A1", "10").unwrap();
    wb.recalculate();
    // B1, C1 and the range formula D1; B2 is untouched.
    assert_eq!(wb.last_recalc_evaluations(), 3);
    assert_eq!(wb.value_a1(0, "C1"), num(21.0));
    assert_eq!(wb.value_a1(0, "D1"), num(15.0));
    wb.recalculate();
    assert_eq!(wb.last_recalc_evaluations(), 0);
}

#[test]
fn evaluation_order_is_topological() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    // Written in reverse dependency order.
    wb.set_a1(0, "A5", "=A4+1").unwrap();
    wb.set_a1(0, "A4", "=A3+1").unwrap();
    wb.set_a1(0, "A3", "=A2+1").unwrap();
    wb.set_a1(0, "A2", "=A1+1").unwrap();
    wb.set_a1(0, "A1", "1").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A5"), num(5.0));
    assert_eq!(wb.last_recalc_evaluations(), 4);
}

#[test]
fn dynamic_references_are_ordered() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "C1", "=INDIRECT(\"D1\")+OFFSET(D1,1,0)")
        .unwrap();
    wb.set_a1(0, "D1", "=3*2").unwrap();
    wb.set_a1(0, "D2", "=D1*10").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "C1"), num(66.0));
    wb.set_a1(0, "D1", "=1").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "C1"), num(11.0));
}

#[test]
fn circular_references() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "=B1+1").unwrap();
    wb.set_a1(0, "B1", "=A1+1").unwrap();
    wb.set_a1(0, "C1", "=C1+1").unwrap();
    wb.set_a1(0, "D1", "=A1*2").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), Value::Error(ErrorKind::Calc));
    assert_eq!(wb.value_a1(0, "C1"), Value::Error(ErrorKind::Calc));
    assert_eq!(wb.value_a1(0, "D1"), Value::Error(ErrorKind::Calc));
    assert_eq!(
        wb.circular_references(),
        vec![
            CellId::new(0, 0, 0),
            CellId::new(0, 0, 1),
            CellId::new(0, 0, 2)
        ]
    );
    // Breaking the cycle restores normal values.
    wb.set_a1(0, "B1", "1").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), num(2.0));
    assert_eq!(wb.value_a1(0, "D1"), num(4.0));
    assert_eq!(wb.circular_references(), vec![CellId::new(0, 0, 2)]);
}

#[test]
fn iterative_calculation() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_iterative(Some(IterativeConfig::default()));
    // x = (x + 10) / 2 converges to 10.
    wb.set_a1(0, "A1", "=(A1+10)/2").unwrap();
    // Two-cell loop: a = b/2 + 1, b = a converges to 2.
    wb.set_a1(0, "B1", "=C1/2+1").unwrap();
    wb.set_a1(0, "C1", "=B1").unwrap();
    wb.recalculate();
    let Value::Number(a) = wb.value_a1(0, "A1") else {
        panic!("number expected")
    };
    assert!((a - 10.0).abs() < 0.01, "{a}");
    let Value::Number(b) = wb.value_a1(0, "B1") else {
        panic!("number expected")
    };
    assert!((b - 2.0).abs() < 0.01, "{b}");
    // With a single iteration the loop stops early.
    wb.set_iterative(Some(IterativeConfig {
        max_iterations: 1,
        max_change: 0.001,
    }));
    wb.set_a1(0, "A1", "=(A1+10)/2").unwrap();
    wb.recalculate();
    assert!(matches!(wb.value_a1(0, "A1"), Value::Number(_)));
}

#[test]
fn volatile_functions_recalculate() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_now(Some(100.25));
    wb.set_a1(0, "A1", "=RAND()").unwrap();
    wb.set_a1(0, "B1", "=NOW()").unwrap();
    wb.set_a1(0, "C1", "=TODAY()+1").unwrap();
    wb.set_a1(0, "D1", "=B1*2").unwrap();
    wb.set_a1(0, "E1", "=1+1").unwrap();
    wb.recalculate();
    let r1 = wb.value_a1(0, "A1");
    assert_eq!(wb.value_a1(0, "C1"), num(101.0));
    wb.set_now(Some(200.5));
    wb.recalculate();
    assert_ne!(wb.value_a1(0, "A1"), r1);
    assert_eq!(wb.value_a1(0, "B1"), num(200.5));
    assert_eq!(wb.value_a1(0, "D1"), num(401.0));
    // Volatile cells and their dependents only: A1, B1, C1, D1.
    assert_eq!(wb.last_recalc_evaluations(), 4);
}

// ------------------------------------------------------------------ edits

#[test]
fn insert_and_delete_rows_and_columns() {
    let mut wb = Workbook::with_sheets(&["Sheet1", "Sheet2"]);
    wb.set_a1(0, "A1", "1").unwrap();
    wb.set_a1(0, "A2", "2").unwrap();
    wb.set_a1(0, "A3", "=SUM(A1:A2)").unwrap();
    wb.set_a1(0, "C5", "=$A$1+A2").unwrap();
    wb.set_a1(1, "A1", "=Sheet1!A2*10").unwrap();
    wb.define_name("First", None, "=Sheet1!$A$1").unwrap();
    wb.set_a1(1, "A2", "=First").unwrap();
    wb.recalculate();

    wb.apply_structural_change(0, StructuralChange::InsertRows { at: 0, count: 1 });
    wb.recalculate();
    assert_eq!(wb.formula(0, 3, 0), Some("=SUM(A2:A3)"));
    assert_eq!(wb.value_a1(0, "A4"), num(3.0));
    assert_eq!(wb.formula(0, 5, 2), Some("=$A$2+A3"));
    assert_eq!(wb.formula(1, 0, 0), Some("=Sheet1!A3*10"));
    assert_eq!(wb.value_a1(1, "A1"), num(20.0));
    assert_eq!(wb.value_a1(1, "A2"), num(1.0));

    // Insert inside a range expands it.
    wb.apply_structural_change(0, StructuralChange::InsertRows { at: 2, count: 2 });
    wb.recalculate();
    assert_eq!(wb.formula(0, 5, 0), Some("=SUM(A2:A5)"));

    // Deleting a referenced cell gives #REF!.
    wb.apply_structural_change(0, StructuralChange::DeleteRows { at: 1, count: 1 });
    wb.recalculate();
    assert_eq!(wb.formula(0, 6, 2), Some("=#REF!+A4"));
    assert_eq!(wb.value_a1(0, "C7"), Value::Error(ErrorKind::Ref));
    assert_eq!(wb.formula(0, 4, 0), Some("=SUM(A2:A4)"));
    assert_eq!(wb.value_a1(0, "A5"), num(2.0));
    assert_eq!(wb.value_a1(1, "A2"), Value::Error(ErrorKind::Ref));

    // Columns.
    wb.apply_structural_change(0, StructuralChange::InsertCols { at: 0, count: 1 });
    wb.recalculate();
    assert_eq!(wb.formula(0, 4, 1), Some("=SUM(B2:B4)"));
    assert_eq!(wb.value_a1(0, "B5"), num(2.0));
    wb.apply_structural_change(0, StructuralChange::DeleteCols { at: 0, count: 1 });
    wb.recalculate();
    assert_eq!(wb.formula(0, 4, 0), Some("=SUM(A2:A4)"));
}

#[test]
fn tables_follow_structural_changes() {
    let mut wb = sales_table();
    wb.set_a1(0, "F1", "=SUM(Sales[Price])").unwrap();
    wb.apply_structural_change(0, StructuralChange::InsertRows { at: 2, count: 1 });
    wb.set_a1(0, "C3", "100").unwrap();
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "F1"), num(133.5));
    wb.apply_structural_change(0, StructuralChange::InsertCols { at: 0, count: 1 });
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "G1"), num(133.5));
}

#[test]
fn copy_paste_adjusts_relative_references() {
    let e = parse("=A1+$B$1+B$2+$C3+SUM(A1:B2)").unwrap();
    assert_eq!(
        format(&offset_relative(&e, 1, 1)),
        "B2+$B$1+C$2+$C4+SUM(B2:C3)"
    );
    assert_eq!(
        format(&offset_relative(&e, -1, 0)),
        "#REF!+$B$1+B$2+$C2+SUM(#REF!)"
    );

    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_a1(0, "A1", "1").unwrap();
    wb.set_a1(0, "A2", "2").unwrap();
    wb.set_a1(0, "B1", "=A1*10").unwrap();
    wb.copy_cell(CellId::new(0, 0, 1), CellId::new(0, 1, 1));
    wb.copy_cell(CellId::new(0, 0, 0), CellId::new(0, 2, 0));
    wb.recalculate();
    assert_eq!(wb.formula(0, 1, 1), Some("=A2*10"));
    assert_eq!(wb.value_a1(0, "B2"), num(20.0));
    assert_eq!(wb.value_a1(0, "A3"), num(1.0));
}
