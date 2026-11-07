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
            ("=ISBLANK(Z1)", B(true)),
            ("=ISBLANK(A1)", B(false)),
            ("=ISBLANK(\"\")", B(false)),
            ("=ISNUMBER(4)", B(true)),
            ("=ISNUMBER(\"4\")", B(false)),
            ("=ISNUMBER(A1)", B(true)),
            ("=ISNUMBER(Z1)", B(false)),
            ("=ISTEXT(\"a\")", B(true)),
            ("=ISTEXT(1)", B(false)),
            ("=ISTEXT(F1)", B(true)),
            ("=ISNONTEXT(1)", B(true)),
            ("=ISNONTEXT(Z1)", B(true)),
            ("=ISNONTEXT(F1)", B(false)),
            ("=ISERROR(1/0)", B(true)),
            ("=ISERROR(#N/A)", B(true)),
            ("=ISERROR(1)", B(false)),
            ("=ISERR(#N/A)", B(false)),
            ("=ISERR(1/0)", B(true)),
            ("=ISNA(#N/A)", B(true)),
            ("=ISNA(1/0)", B(false)),
            ("=ISLOGICAL(TRUE)", B(true)),
            ("=ISLOGICAL(\"TRUE\")", B(false)),
            ("=ISREF(A1)", B(true)),
            ("=ISREF(A1:B2)", B(true)),
            ("=ISREF(\"A1\")", B(false)),
            ("=ISFORMULA(H1)", B(true)),
            ("=ISFORMULA(A1)", B(false)),
            ("=ISEVEN(-1)", B(false)),
            ("=ISEVEN(2.5)", B(true)),
            ("=ISEVEN(5)", B(false)),
            ("=ISEVEN(0)", B(true)),
            ("=ISEVEN(\"a\")", E("#VALUE!")),
            ("=ISODD(-1)", B(true)),
            ("=ISODD(2.5)", B(false)),
            ("=ISODD(5)", B(true)),
            ("=ISNUMBER({1,\"a\"})", A("{TRUE,FALSE}")),
        ],
    );
}

#[test]
fn type_n_na_error_type() {
    check(
        DATA,
        &[
            ("=TYPE(1)", N(1.0)),
            ("=TYPE(\"a\")", N(2.0)),
            ("=TYPE(TRUE)", N(4.0)),
            ("=TYPE(#N/A)", N(16.0)),
            ("=TYPE({1,2})", N(64.0)),
            ("=TYPE(Z1)", N(1.0)),
            ("=N(7)", N(7.0)),
            ("=N(\"7\")", N(0.0)),
            ("=N(TRUE)", N(1.0)),
            ("=N(#N/A)", E("#N/A")),
            ("=N(A1)", N(1.0)),
            ("=NA()", E("#N/A")),
            ("=ERROR.TYPE(#NULL!)", N(1.0)),
            ("=ERROR.TYPE(1/0)", N(2.0)),
            ("=ERROR.TYPE(#VALUE!)", N(3.0)),
            ("=ERROR.TYPE(#REF!)", N(4.0)),
            ("=ERROR.TYPE(#NAME?)", N(5.0)),
            ("=ERROR.TYPE(#NUM!)", N(6.0)),
            ("=ERROR.TYPE(#N/A)", N(7.0)),
            ("=ERROR.TYPE(1)", E("#N/A")),
        ],
    );
}

#[test]
fn cell_sheet_formulatext() {
    check(
        DATA,
        &[
            ("=CELL(\"row\",B7)", N(7.0)),
            ("=CELL(\"col\",C3)", N(3.0)),
            ("=CELL(\"address\",B2)", T("$B$2")),
            ("=CELL(\"contents\",A1)", N(1.0)),
            ("=CELL(\"type\",F1)", T("l")),
            ("=CELL(\"type\",Z1)", T("b")),
            ("=CELL(\"type\",A1)", T("v")),
            ("=CELL(\"bogus\",A1)", E("#VALUE!")),
            ("=SHEET()", N(1.0)),
            ("=SHEET(Sheet2!A1)", N(2.0)),
            ("=SHEET(\"Sheet3\")", N(3.0)),
            ("=SHEETS()", N(3.0)),
            ("=SHEETS(Sheet1:Sheet3!A1)", N(3.0)),
            ("=FORMULATEXT(H1)", T("=A1*2")),
            ("=FORMULATEXT(A1)", E("#N/A")),
        ],
    );
}
