//! Information functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (ISBLANK, ISERR, ISERROR, ISLOGICAL, ISNA,
//! ISNONTEXT, ISNUMBER, ISREF, ISTEXT, ISEVEN, ISODD, TYPE, N, NA, CELL,
//! ERROR.TYPE) and OpenFormula (ODF 1.3 Part 4) Information functions
//! (ISFORMULA, FORMULATEXT, SHEET, SHEETS). TYPE codes: 1 number, 2 text,
//! 4 logical, 16 error, 64 array.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    ("A1", "1"),
    ("F1", "text"),
    ("H1", "=A1*2"),
    ("Sheet2!A1", "9"),
];

#[test]
fn is_functions() {
    check(
        DATA,
        &[
            ("=ISBLANK(Z1)", B(true), "ECMA-376 §18.17.7 ISBLANK"),
            ("=ISBLANK(A1)", B(false), "ECMA-376 §18.17.7 ISBLANK"),
            ("=ISBLANK(\"\")", B(false), "ECMA-376 §18.17.7 ISBLANK"),
            ("=ISNUMBER(4)", B(true), "ECMA-376 §18.17.7 ISNUMBER"),
            ("=ISNUMBER(\"4\")", B(false), "ECMA-376 §18.17.7 ISNUMBER"),
            ("=ISNUMBER(A1)", B(true), "ECMA-376 §18.17.7 ISNUMBER"),
            ("=ISNUMBER(Z1)", B(false), "ECMA-376 §18.17.7 ISNUMBER"),
            ("=ISTEXT(\"a\")", B(true), "ECMA-376 §18.17.7 ISTEXT"),
            ("=ISTEXT(1)", B(false), "ECMA-376 §18.17.7 ISTEXT"),
            ("=ISTEXT(F1)", B(true), "ECMA-376 §18.17.7 ISTEXT"),
            ("=ISNONTEXT(1)", B(true), "ECMA-376 §18.17.7 ISNONTEXT"),
            ("=ISNONTEXT(Z1)", B(true), "ECMA-376 §18.17.7 ISNONTEXT"),
            ("=ISNONTEXT(F1)", B(false), "ECMA-376 §18.17.7 ISNONTEXT"),
            ("=ISERROR(1/0)", B(true), "ECMA-376 §18.17.7 ISERROR"),
            ("=ISERROR(#N/A)", B(true), "ECMA-376 §18.17.7 ISERROR"),
            ("=ISERROR(1)", B(false), "ECMA-376 §18.17.7 ISERROR"),
            ("=ISERR(#N/A)", B(false), "ECMA-376 §18.17.7 ISERR"),
            ("=ISERR(1/0)", B(true), "ECMA-376 §18.17.7 ISERR"),
            ("=ISNA(#N/A)", B(true), "ECMA-376 §18.17.7 ISNA"),
            ("=ISNA(1/0)", B(false), "ECMA-376 §18.17.7 ISNA"),
            ("=ISLOGICAL(TRUE)", B(true), "ECMA-376 §18.17.7 ISLOGICAL"),
            (
                "=ISLOGICAL(\"TRUE\")",
                B(false),
                "ECMA-376 §18.17.7 ISLOGICAL",
            ),
            ("=ISREF(A1)", B(true), "ECMA-376 §18.17.7 ISREF"),
            ("=ISREF(A1:B2)", B(true), "ECMA-376 §18.17.7 ISREF"),
            ("=ISREF(\"A1\")", B(false), "ECMA-376 §18.17.7 ISREF"),
            ("=ISFORMULA(H1)", B(true), "OpenFormula ISFORMULA"),
            ("=ISFORMULA(A1)", B(false), "OpenFormula ISFORMULA"),
            ("=ISEVEN(-1)", B(false), "ECMA-376 §18.17.7 ISEVEN"),
            ("=ISEVEN(2.5)", B(true), "ECMA-376 §18.17.7 ISEVEN"),
            ("=ISEVEN(5)", B(false), "ECMA-376 §18.17.7 ISEVEN"),
            ("=ISEVEN(0)", B(true), "ECMA-376 §18.17.7 ISEVEN"),
            ("=ISEVEN(\"a\")", E("#VALUE!"), "ECMA-376 §18.17.7 ISEVEN"),
            ("=ISODD(-1)", B(true), "ECMA-376 §18.17.7 ISODD"),
            ("=ISODD(2.5)", B(false), "ECMA-376 §18.17.7 ISODD"),
            ("=ISODD(5)", B(true), "ECMA-376 §18.17.7 ISODD"),
            (
                "=ISNUMBER({1,\"a\"})",
                A("{TRUE,FALSE}"),
                "ECMA-376 §18.17.7 ISNUMBER",
            ),
        ],
    );
}

#[test]
fn type_n_na_error_type() {
    check(
        DATA,
        &[
            ("=TYPE(1)", N(1.0), "ECMA-376 §18.17.7 TYPE"),
            ("=TYPE(\"a\")", N(2.0), "ECMA-376 §18.17.7 TYPE"),
            ("=TYPE(TRUE)", N(4.0), "ECMA-376 §18.17.7 TYPE"),
            ("=TYPE(#N/A)", N(16.0), "ECMA-376 §18.17.7 TYPE"),
            ("=TYPE({1,2})", N(64.0), "ECMA-376 §18.17.7 TYPE"),
            ("=TYPE(Z1)", N(1.0), "ECMA-376 §18.17.7 TYPE"),
            ("=N(7)", N(7.0), "ECMA-376 §18.17.7 N"),
            ("=N(\"7\")", N(0.0), "ECMA-376 §18.17.7 N"),
            ("=N(TRUE)", N(1.0), "ECMA-376 §18.17.7 N"),
            ("=N(#N/A)", E("#N/A"), "ECMA-376 §18.17.7 N"),
            ("=N(A1)", N(1.0), "ECMA-376 §18.17.7 N"),
            ("=NA()", E("#N/A"), "ECMA-376 §18.17.7 NA"),
            (
                "=ERROR.TYPE(#NULL!)",
                N(1.0),
                "ECMA-376 §18.17.7 ERROR.TYPE",
            ),
            ("=ERROR.TYPE(1/0)", N(2.0), "ECMA-376 §18.17.7 ERROR.TYPE"),
            (
                "=ERROR.TYPE(#VALUE!)",
                N(3.0),
                "ECMA-376 §18.17.7 ERROR.TYPE",
            ),
            ("=ERROR.TYPE(#REF!)", N(4.0), "ECMA-376 §18.17.7 ERROR.TYPE"),
            (
                "=ERROR.TYPE(#NAME?)",
                N(5.0),
                "ECMA-376 §18.17.7 ERROR.TYPE",
            ),
            ("=ERROR.TYPE(#NUM!)", N(6.0), "ECMA-376 §18.17.7 ERROR.TYPE"),
            ("=ERROR.TYPE(#N/A)", N(7.0), "ECMA-376 §18.17.7 ERROR.TYPE"),
            ("=ERROR.TYPE(1)", E("#N/A"), "ECMA-376 §18.17.7 ERROR.TYPE"),
        ],
    );
}

#[test]
fn cell_sheet_formulatext() {
    check(
        DATA,
        &[
            ("=CELL(\"row\",B7)", N(7.0), "ECMA-376 §18.17.7 CELL"),
            ("=CELL(\"col\",C3)", N(3.0), "ECMA-376 §18.17.7 CELL"),
            ("=CELL(\"address\",B2)", T("$B$2"), "ECMA-376 §18.17.7 CELL"),
            ("=CELL(\"contents\",A1)", N(1.0), "ECMA-376 §18.17.7 CELL"),
            ("=CELL(\"type\",F1)", T("l"), "ECMA-376 §18.17.7 CELL"),
            ("=CELL(\"type\",Z1)", T("b"), "ECMA-376 §18.17.7 CELL"),
            ("=CELL(\"type\",A1)", T("v"), "ECMA-376 §18.17.7 CELL"),
            (
                "=CELL(\"bogus\",A1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 CELL",
            ),
            ("=SHEET()", N(1.0), "OpenFormula SHEET"),
            ("=SHEET(Sheet2!A1)", N(2.0), "OpenFormula SHEET"),
            ("=SHEET(\"Sheet3\")", N(3.0), "OpenFormula SHEET"),
            ("=SHEETS()", N(3.0), "OpenFormula SHEETS"),
            ("=SHEETS(Sheet1:Sheet3!A1)", N(3.0), "OpenFormula SHEETS"),
            ("=FORMULATEXT(H1)", T("=A1*2"), "OpenFormula FORMULATEXT"),
            ("=FORMULATEXT(A1)", E("#N/A"), "OpenFormula FORMULATEXT"),
        ],
    );
}

/// Additional value, edge and error rows for the Information list (gap #37).
#[test]
fn info_edge_cases() {
    check(
        DATA,
        &[
            ("=ISBLANK(0)", B(false), "hand: a number is not blank"),
            (
                "=ISBLANK(Sheet2!B1)",
                B(true),
                "hand: empty cell on another sheet",
            ),
            (
                "=ISNUMBER(TRUE)",
                B(false),
                "ECMA-376 §18.17.7 ISNUMBER; hand: logical is not a number",
            ),
            ("=ISTEXT(\"\")", B(true), "hand: empty text is text"),
            (
                "=ISTEXT(Z1)",
                B(false),
                "ECMA-376 §18.17.7 ISTEXT; hand: blank is not text",
            ),
            ("=ISERROR(\"#N/A\")", B(false), "hand: text, not an error"),
            ("=ISERROR(#VALUE!)", B(true), "hand: error value"),
            ("=ISERR(1)", B(false), "hand: not an error"),
            (
                "=ISERR(#VALUE!)",
                B(true),
                "ECMA-376 §18.17.7 ISERR; hand: any error except #N/A",
            ),
            ("=ISNA(\"#N/A\")", B(false), "hand: text, not an error"),
            (
                "=ISNA(NA())",
                B(true),
                "ECMA-376 §18.17.7 ISNA; hand: NA() is #N/A",
            ),
            ("=ISLOGICAL(1)", B(false), "hand: number"),
            (
                "=ISLOGICAL(1=1)",
                B(true),
                "ECMA-376 §18.17.7 ISLOGICAL; hand: comparison result",
            ),
            ("=ISREF(Sheet2!A1)", B(true), "hand: 3-D style reference"),
            (
                "=ISREF(1)",
                B(false),
                "ECMA-376 §18.17.7 ISREF; hand: number",
            ),
            ("=TYPE(\"\")", N(2.0), "hand: empty text is text"),
            ("=N(\"\")", N(0.0), "ECMA-376 §18.17.7 N; hand: text → 0"),
            ("=N(F1)", N(0.0), "hand: text cell → 0"),
            (
                "=NA()+1",
                E("#N/A"),
                "ECMA-376 §18.17.7 NA; hand: error propagates",
            ),
            (
                "=IFERROR(NA(),\"x\")",
                T("x"),
                "ECMA-376 §18.17.7 NA; hand: #N/A is an error",
            ),
        ],
    );
}

#[test]
fn info_and_isomitted() {
    check(
        DATA,
        &[
            (
                "=INFO(\"recalc\")",
                T("Automatic"),
                "ECMA-376 §18.17.7 INFO",
            ),
            ("=INFO(\"system\")", T("pcdos"), "ECMA-376 §18.17.7 INFO"),
            ("=INFO(\"origin\")", T("$A:$A$1"), "ECMA-376 §18.17.7 INFO"),
            (
                "=INFO(\"numfile\")",
                N(3.0),
                "hand: the harness has Sheet1, Sheet2, Sheet3",
            ),
            ("=INFO(\"colour\")", E("#VALUE!"), "ECMA-376 §18.17.7 INFO"),
            (
                "=LAMBDA(a,b,ISOMITTED(a))(,2)",
                B(true),
                "MS-XLSX _xlfn.ISOMITTED: first argument empty",
            ),
            (
                "=LAMBDA(a,b,ISOMITTED(b))(1,)",
                B(true),
                "MS-XLSX _xlfn.ISOMITTED: empty argument",
            ),
            (
                "=LAMBDA(a,b,ISOMITTED(b))(1,2)",
                B(false),
                "MS-XLSX _xlfn.ISOMITTED",
            ),
            (
                "=LAMBDA(a,b,IF(ISOMITTED(b),a,a+b))(5,)",
                N(5.0),
                "hand: default when b is left out",
            ),
            (
                "=LAMBDA(a,b,IF(ISOMITTED(b),a,a+b))(5,2)",
                N(7.0),
                "hand: 5+2",
            ),
            (
                "=LAMBDA(a,a)(1,2)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.LAMBDA: too many arguments",
            ),
        ],
    );
}
