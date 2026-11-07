//! Lookup & reference functions and dynamic-array functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (VLOOKUP, HLOOKUP, LOOKUP, INDEX, MATCH,
//! CHOOSE, OFFSET, INDIRECT, ROW, ROWS, COLUMN, COLUMNS, ADDRESS, AREAS,
//! TRANSPOSE) and OpenFormula (ODF 1.3 Part 4) Lookup functions; XLOOKUP,
//! XMATCH, FILTER, SORT, SORTBY, UNIQUE, HSTACK, VSTACK, CHOOSECOLS, CHOOSEROWS,
//! TAKE and DROP follow their published dynamic-array function definitions
//! (match_mode 0 exact / -1 next smaller / 1 next larger / 2 wildcard;
//! search_mode 1 first-to-last / -1 last-to-first / ±2 binary).

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    ("A1", "ID"),
    ("B1", "Name"),
    ("C1", "Price"),
    ("A2", "1"),
    ("B2", "apple"),
    ("C2", "1.5"),
    ("A3", "2"),
    ("B3", "banana"),
    ("C3", "0.25"),
    ("A4", "3"),
    ("B4", "cherry"),
    ("C4", "3"),
    ("A5", "4"),
    ("B5", "date"),
    ("C5", "2"),
    ("Sheet2!A1", "42"),
];

#[test]
fn vlookup_hlookup_lookup() {
    check(
        DATA,
        &[
            ("=VLOOKUP(3,A2:C5,2,FALSE)", T("cherry")),
            ("=VLOOKUP(2.5,A2:C5,2)", T("banana")),
            ("=VLOOKUP(2.5,A2:C5,2,TRUE)", T("banana")),
            ("=VLOOKUP(0,A2:C5,2)", E("#N/A")),
            ("=VLOOKUP(9,A2:C5,2,FALSE)", E("#N/A")),
            ("=VLOOKUP(3,A2:C5,4,FALSE)", E("#REF!")),
            ("=VLOOKUP(3,A2:C5,0,FALSE)", E("#VALUE!")),
            ("=VLOOKUP(\"ch*\",B2:C5,2,FALSE)", N(3.0)),
            ("=VLOOKUP(\"CHERRY\",B2:C5,2,FALSE)", N(3.0)),
            ("=HLOOKUP(\"Price\",A1:C5,3,FALSE)", N(0.25)),
            ("=HLOOKUP(\"Name\",A1:C5,5,FALSE)", T("date")),
            ("=HLOOKUP(\"X\",A1:C5,2,FALSE)", E("#N/A")),
            ("=LOOKUP(3,A2:A5,B2:B5)", T("cherry")),
            ("=LOOKUP(2.5,A2:A5,C2:C5)", N(0.25)),
            ("=LOOKUP(0,A2:A5,B2:B5)", E("#N/A")),
            ("=LOOKUP(3,A2:C5)", N(3.0)),
            ("=LOOKUP(3,{1,2,3},{\"a\",\"b\",\"c\"})", T("c")),
        ],
    );
}

#[test]
fn index_match() {
    check(
        DATA,
        &[
            ("=INDEX(A2:C5,2,2)", T("banana")),
            ("=INDEX(A2:C5,0,3)", A("{1.5;0.25;3;2}")),
            ("=INDEX(A2:C5,3,0)", A("{3,\"cherry\",3}")),
            ("=INDEX({1,2;3,4},2,1)", N(3.0)),
            ("=INDEX({1,2,3},2)", N(2.0)),
            ("=INDEX(A2:C5,5,1)", E("#REF!")),
            ("=INDEX((A2:A3,C2:C3),1,1,2)", N(1.5)),
            ("=SUM(INDEX(C2:C5,1):INDEX(C2:C5,3))", N(4.75)),
            ("=MATCH(3,A2:A5,0)", N(3.0)),
            ("=MATCH(2.5,A2:A5)", N(2.0)),
            ("=MATCH(2.5,A2:A5,1)", N(2.0)),
            ("=MATCH(\"b*\",B2:B5,0)", N(2.0)),
            ("=MATCH(\"BANANA\",B2:B5,0)", N(2.0)),
            ("=MATCH(9,A2:A5,0)", E("#N/A")),
            ("=MATCH(3,{5,4,3,2},-1)", N(3.0)),
            ("=MATCH(4.5,{5,4,3,2},-1)", N(1.0)),
        ],
    );
}

#[test]
fn xlookup_xmatch() {
    check(
        DATA,
        &[
            ("=XLOOKUP(\"cherry\",B2:B5,C2:C5)", N(3.0)),
            ("=XLOOKUP(\"zzz\",B2:B5,C2:C5,\"none\")", T("none")),
            ("=XLOOKUP(\"zzz\",B2:B5,C2:C5)", E("#N/A")),
            ("=XLOOKUP(2.5,A2:A5,B2:B5,,-1)", T("banana")),
            ("=XLOOKUP(2.5,A2:A5,B2:B5,,1)", T("cherry")),
            ("=XLOOKUP(\"c*\",B2:B5,C2:C5,,2)", N(3.0)),
            ("=XLOOKUP(3,A2:A5,A2:C5)", A("{3,\"cherry\",3}")),
            (
                "=XLOOKUP(2,{1,2,2,3},{\"a\",\"b\",\"c\",\"d\"},,0,-1)",
                T("c"),
            ),
            ("=XLOOKUP(3,A2:A5,B2:B5,,0,2)", T("cherry")),
            ("=XLOOKUP(3,A2:A5,B2:B4)", E("#VALUE!")),
            ("=XMATCH(\"cherry\",B2:B5)", N(3.0)),
            ("=XMATCH(2.5,A2:A5,-1)", N(2.0)),
            ("=XMATCH(2.5,A2:A5,1)", N(3.0)),
            ("=XMATCH(9,A2:A5)", E("#N/A")),
            ("=XMATCH(2,{1,2,2,3},0,-1)", N(3.0)),
        ],
    );
}

#[test]
fn choose_offset_indirect() {
    check(
        DATA,
        &[
            ("=CHOOSE(2,\"a\",\"b\",\"c\")", T("b")),
            ("=CHOOSE(4,\"a\",\"b\")", E("#VALUE!")),
            ("=CHOOSE(0,\"a\")", E("#VALUE!")),
            ("=SUM(CHOOSE(2,A2:A5,C2:C5))", N(6.75)),
            ("=CHOOSE({1,2},\"a\",\"b\")", A("{\"a\",\"b\"}")),
            ("=OFFSET(A1,2,1)", T("banana")),
            ("=SUM(OFFSET(C2,0,0,3,1))", N(4.75)),
            ("=OFFSET(A1,-1,0)", E("#REF!")),
            ("=ROWS(OFFSET(A1,0,0,3,2))", N(3.0)),
            ("=COLUMNS(OFFSET(A1,0,0,3,2))", N(2.0)),
            ("=INDIRECT(\"B3\")", T("banana")),
            ("=INDIRECT(\"R3C2\",FALSE)", T("banana")),
            ("=INDIRECT(\"Sheet2!A1\")", N(42.0)),
            ("=INDIRECT(\"'Sheet2'!A1\")", N(42.0)),
            ("=SUM(INDIRECT(\"C2:C5\"))", N(6.75)),
            ("=INDIRECT(\"bogus\")", E("#REF!")),
        ],
    );
}

#[test]
fn row_column_address_areas() {
    check(
        DATA,
        &[
            ("=ROW(C10)", N(10.0)),
            ("=ROW(A2:A4)", A("{2;3;4}")),
            ("=ROWS(A2:C5)", N(4.0)),
            ("=ROWS({1,2;3,4})", N(2.0)),
            ("=COLUMN(D3)", N(4.0)),
            ("=COLUMN(A1:C1)", A("{1,2,3}")),
            ("=COLUMNS(A2:C5)", N(3.0)),
            ("=ADDRESS(2,3)", T("$C$2")),
            ("=ADDRESS(2,3,2)", T("C$2")),
            ("=ADDRESS(2,3,2,FALSE)", T("R2C[3]")),
            (
                "=ADDRESS(2,3,1,FALSE,\"[Book1]Sheet1\")",
                T("[Book1]Sheet1!R2C3"),
            ),
            (
                "=ADDRESS(2,3,1,FALSE,\"EXCEL SHEET\")",
                T("'EXCEL SHEET'!R2C3"),
            ),
            ("=ADDRESS(1,1,4)", T("A1")),
            ("=ADDRESS(0,1)", E("#VALUE!")),
            ("=AREAS((A1,B2:C3))", N(2.0)),
            ("=AREAS(A1:B2)", N(1.0)),
        ],
    );
}

#[test]
fn filter_sort_unique() {
    check(
        DATA,
        &[
            ("=FILTER(B2:B5,C2:C5>1.6)", A("{\"cherry\";\"date\"}")),
            ("=FILTER(B2:B5,C2:C5>10)", E("#CALC!")),
            ("=FILTER(B2:B5,C2:C5>10,\"none\")", T("none")),
            ("=FILTER(A2:C5,A2:A5>3)", A("{4,\"date\",2}")),
            ("=FILTER(B2:B5,C2:C4>1)", E("#VALUE!")),
            ("=SORT({3;1;2})", A("{1;2;3}")),
            ("=SORT({3;1;2},1,-1)", A("{3;2;1}")),
            (
                "=SORT(B2:C5,2,-1)",
                A("{\"cherry\",3;\"date\",2;\"apple\",1.5;\"banana\",0.25}"),
            ),
            ("=SORT({3,1,2},1,1,TRUE)", A("{1,2,3}")),
            ("=SORT({\"b\";\"A\";\"c\"})", A("{\"A\";\"b\";\"c\"}")),
            (
                "=SORTBY(B2:B5,C2:C5)",
                A("{\"banana\";\"apple\";\"date\";\"cherry\"}"),
            ),
            (
                "=SORTBY(B2:B5,C2:C5,-1)",
                A("{\"cherry\";\"date\";\"apple\";\"banana\"}"),
            ),
            ("=UNIQUE({1;2;2;3;1})", A("{1;2;3}")),
            ("=UNIQUE({1;2;2;3;1},FALSE,TRUE)", A("{3}")),
            ("=UNIQUE({\"a\";\"A\";\"b\"})", A("{\"a\";\"b\"}")),
            ("=UNIQUE({1,1,2},TRUE)", A("{1,2}")),
        ],
    );
}

#[test]
fn array_shaping() {
    check(
        DATA,
        &[
            ("=TRANSPOSE({1,2,3})", A("{1;2;3}")),
            ("=TRANSPOSE({1,2;3,4})", A("{1,3;2,4}")),
            ("=TRANSPOSE(A2:B3)", A("{1,2;\"apple\",\"banana\"}")),
            ("=HSTACK({1,2},{3})", A("{1,2,3}")),
            ("=VSTACK({1,2},{3,4})", A("{1,2;3,4}")),
            ("=VSTACK({1,2},{3})", A("{1,2;3,#N/A}")),
            ("=CHOOSECOLS({1,2,3;4,5,6},1,3)", A("{1,3;4,6}")),
            ("=CHOOSECOLS({1,2,3},-1)", N(3.0)),
            ("=CHOOSECOLS({1,2},3)", E("#VALUE!")),
            ("=CHOOSEROWS({1,2;3,4;5,6},2)", A("{3,4}")),
            ("=TAKE({1,2,3;4,5,6},1)", A("{1,2,3}")),
            ("=TAKE({1,2,3;4,5,6},,-1)", A("{3;6}")),
            ("=DROP({1,2,3;4,5,6},1)", A("{4,5,6}")),
            ("=DROP({1,2,3;4,5,6},,2)", A("{3;6}")),
            ("=DROP({1,2},1)", E("#CALC!")),
        ],
    );
}
