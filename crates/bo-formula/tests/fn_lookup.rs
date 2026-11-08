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
            (
                "=VLOOKUP(3,A2:C5,2,FALSE)",
                T("cherry"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(2.5,A2:C5,2)",
                T("banana"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(2.5,A2:C5,2,TRUE)",
                T("banana"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(0,A2:C5,2)",
                E("#N/A"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(9,A2:C5,2,FALSE)",
                E("#N/A"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(3,A2:C5,4,FALSE)",
                E("#REF!"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(3,A2:C5,0,FALSE)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(\"ch*\",B2:C5,2,FALSE)",
                N(3.0),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=VLOOKUP(\"CHERRY\",B2:C5,2,FALSE)",
                N(3.0),
                "ECMA-376 §18.17.7 VLOOKUP",
            ),
            (
                "=HLOOKUP(\"Price\",A1:C5,3,FALSE)",
                N(0.25),
                "ECMA-376 §18.17.7 HLOOKUP",
            ),
            (
                "=HLOOKUP(\"Name\",A1:C5,5,FALSE)",
                T("date"),
                "ECMA-376 §18.17.7 HLOOKUP",
            ),
            (
                "=HLOOKUP(\"X\",A1:C5,2,FALSE)",
                E("#N/A"),
                "ECMA-376 §18.17.7 HLOOKUP",
            ),
            (
                "=LOOKUP(3,A2:A5,B2:B5)",
                T("cherry"),
                "ECMA-376 §18.17.7 LOOKUP",
            ),
            (
                "=LOOKUP(2.5,A2:A5,C2:C5)",
                N(0.25),
                "ECMA-376 §18.17.7 LOOKUP",
            ),
            (
                "=LOOKUP(0,A2:A5,B2:B5)",
                E("#N/A"),
                "ECMA-376 §18.17.7 LOOKUP",
            ),
            ("=LOOKUP(3,A2:C5)", N(3.0), "ECMA-376 §18.17.7 LOOKUP"),
            (
                "=LOOKUP(3,{1,2,3},{\"a\",\"b\",\"c\"})",
                T("c"),
                "ECMA-376 §18.17.7 LOOKUP",
            ),
        ],
    );
}

#[test]
fn index_match() {
    check(
        DATA,
        &[
            ("=INDEX(A2:C5,2,2)", T("banana"), "ECMA-376 §18.17.7 INDEX"),
            (
                "=INDEX(A2:C5,0,3)",
                A("{1.5;0.25;3;2}"),
                "ECMA-376 §18.17.7 INDEX",
            ),
            (
                "=INDEX(A2:C5,3,0)",
                A("{3,\"cherry\",3}"),
                "ECMA-376 §18.17.7 INDEX",
            ),
            ("=INDEX({1,2;3,4},2,1)", N(3.0), "ECMA-376 §18.17.7 INDEX"),
            ("=INDEX({1,2,3},2)", N(2.0), "ECMA-376 §18.17.7 INDEX"),
            ("=INDEX(A2:C5,5,1)", E("#REF!"), "ECMA-376 §18.17.7 INDEX"),
            (
                "=INDEX((A2:A3,C2:C3),1,1,2)",
                N(1.5),
                "ECMA-376 §18.17.7 INDEX",
            ),
            (
                "=SUM(INDEX(C2:C5,1):INDEX(C2:C5,3))",
                N(4.75),
                "ECMA-376 §18.17.7 INDEX",
            ),
            ("=MATCH(3,A2:A5,0)", N(3.0), "ECMA-376 §18.17.7 MATCH"),
            ("=MATCH(2.5,A2:A5)", N(2.0), "ECMA-376 §18.17.7 MATCH"),
            ("=MATCH(2.5,A2:A5,1)", N(2.0), "ECMA-376 §18.17.7 MATCH"),
            ("=MATCH(\"b*\",B2:B5,0)", N(2.0), "ECMA-376 §18.17.7 MATCH"),
            (
                "=MATCH(\"BANANA\",B2:B5,0)",
                N(2.0),
                "ECMA-376 §18.17.7 MATCH",
            ),
            ("=MATCH(9,A2:A5,0)", E("#N/A"), "ECMA-376 §18.17.7 MATCH"),
            ("=MATCH(3,{5,4,3,2},-1)", N(3.0), "ECMA-376 §18.17.7 MATCH"),
            (
                "=MATCH(4.5,{5,4,3,2},-1)",
                N(1.0),
                "ECMA-376 §18.17.7 MATCH",
            ),
        ],
    );
}

#[test]
fn xlookup_xmatch() {
    check(
        DATA,
        &[
            (
                "=XLOOKUP(\"cherry\",B2:B5,C2:C5)",
                N(3.0),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(\"zzz\",B2:B5,C2:C5,\"none\")",
                T("none"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(\"zzz\",B2:B5,C2:C5)",
                E("#N/A"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(2.5,A2:A5,B2:B5,,-1)",
                T("banana"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(2.5,A2:A5,B2:B5,,1)",
                T("cherry"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(\"c*\",B2:B5,C2:C5,,2)",
                N(3.0),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(3,A2:A5,A2:C5)",
                A("{3,\"cherry\",3}"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(2,{1,2,2,3},{\"a\",\"b\",\"c\",\"d\"},,0,-1)",
                T("c"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(3,A2:A5,B2:B5,,0,2)",
                T("cherry"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            (
                "=XLOOKUP(3,A2:A5,B2:B4)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.XLOOKUP",
            ),
            ("=XMATCH(\"cherry\",B2:B5)", N(3.0), "MS-XLSX _xlfn.XMATCH"),
            ("=XMATCH(2.5,A2:A5,-1)", N(2.0), "MS-XLSX _xlfn.XMATCH"),
            ("=XMATCH(2.5,A2:A5,1)", N(3.0), "MS-XLSX _xlfn.XMATCH"),
            ("=XMATCH(9,A2:A5)", E("#N/A"), "MS-XLSX _xlfn.XMATCH"),
            ("=XMATCH(2,{1,2,2,3},0,-1)", N(3.0), "MS-XLSX _xlfn.XMATCH"),
        ],
    );
}

#[test]
fn choose_offset_indirect() {
    check(
        DATA,
        &[
            (
                "=CHOOSE(2,\"a\",\"b\",\"c\")",
                T("b"),
                "ECMA-376 §18.17.7 CHOOSE",
            ),
            (
                "=CHOOSE(4,\"a\",\"b\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 CHOOSE",
            ),
            ("=CHOOSE(0,\"a\")", E("#VALUE!"), "ECMA-376 §18.17.7 CHOOSE"),
            (
                "=SUM(CHOOSE(2,A2:A5,C2:C5))",
                N(6.75),
                "ECMA-376 §18.17.7 CHOOSE",
            ),
            (
                "=CHOOSE({1,2},\"a\",\"b\")",
                A("{\"a\",\"b\"}"),
                "ECMA-376 §18.17.7 CHOOSE",
            ),
            ("=OFFSET(A1,2,1)", T("banana"), "ECMA-376 §18.17.7 OFFSET"),
            (
                "=SUM(OFFSET(C2,0,0,3,1))",
                N(4.75),
                "ECMA-376 §18.17.7 OFFSET",
            ),
            ("=OFFSET(A1,-1,0)", E("#REF!"), "ECMA-376 §18.17.7 OFFSET"),
            (
                "=ROWS(OFFSET(A1,0,0,3,2))",
                N(3.0),
                "ECMA-376 §18.17.7 OFFSET",
            ),
            (
                "=COLUMNS(OFFSET(A1,0,0,3,2))",
                N(2.0),
                "ECMA-376 §18.17.7 OFFSET",
            ),
            (
                "=INDIRECT(\"B3\")",
                T("banana"),
                "ECMA-376 §18.17.7 INDIRECT",
            ),
            (
                "=INDIRECT(\"R3C2\",FALSE)",
                T("banana"),
                "ECMA-376 §18.17.7 INDIRECT",
            ),
            (
                "=INDIRECT(\"Sheet2!A1\")",
                N(42.0),
                "ECMA-376 §18.17.7 INDIRECT",
            ),
            (
                "=INDIRECT(\"'Sheet2'!A1\")",
                N(42.0),
                "ECMA-376 §18.17.7 INDIRECT",
            ),
            (
                "=SUM(INDIRECT(\"C2:C5\"))",
                N(6.75),
                "ECMA-376 §18.17.7 INDIRECT",
            ),
            (
                "=INDIRECT(\"bogus\")",
                E("#REF!"),
                "ECMA-376 §18.17.7 INDIRECT",
            ),
        ],
    );
}

#[test]
fn row_column_address_areas() {
    check(
        DATA,
        &[
            ("=ROW(C10)", N(10.0), "ECMA-376 §18.17.7 ROW"),
            ("=ROW(A2:A4)", A("{2;3;4}"), "ECMA-376 §18.17.7 ROW"),
            ("=ROWS(A2:C5)", N(4.0), "ECMA-376 §18.17.7 ROWS"),
            ("=ROWS({1,2;3,4})", N(2.0), "ECMA-376 §18.17.7 ROWS"),
            ("=COLUMN(D3)", N(4.0), "ECMA-376 §18.17.7 COLUMN"),
            ("=COLUMN(A1:C1)", A("{1,2,3}"), "ECMA-376 §18.17.7 COLUMN"),
            ("=COLUMNS(A2:C5)", N(3.0), "ECMA-376 §18.17.7 COLUMNS"),
            ("=ADDRESS(2,3)", T("$C$2"), "ECMA-376 §18.17.7 ADDRESS"),
            ("=ADDRESS(2,3,2)", T("C$2"), "ECMA-376 §18.17.7 ADDRESS"),
            (
                "=ADDRESS(2,3,2,FALSE)",
                T("R2C[3]"),
                "ECMA-376 §18.17.7 ADDRESS",
            ),
            (
                "=ADDRESS(2,3,1,FALSE,\"[Book1]Sheet1\")",
                T("[Book1]Sheet1!R2C3"),
                "ECMA-376 §18.17.7 ADDRESS",
            ),
            (
                "=ADDRESS(2,3,1,FALSE,\"EXCEL SHEET\")",
                T("'EXCEL SHEET'!R2C3"),
                "ECMA-376 §18.17.7 ADDRESS",
            ),
            ("=ADDRESS(1,1,4)", T("A1"), "ECMA-376 §18.17.7 ADDRESS"),
            ("=ADDRESS(0,1)", E("#VALUE!"), "ECMA-376 §18.17.7 ADDRESS"),
            ("=AREAS((A1,B2:C3))", N(2.0), "ECMA-376 §18.17.7 AREAS"),
            ("=AREAS(A1:B2)", N(1.0), "ECMA-376 §18.17.7 AREAS"),
        ],
    );
}

#[test]
fn filter_sort_unique() {
    check(
        DATA,
        &[
            (
                "=FILTER(B2:B5,C2:C5>1.6)",
                A("{\"cherry\";\"date\"}"),
                "MS-XLSX _xlfn.FILTER",
            ),
            (
                "=FILTER(B2:B5,C2:C5>10)",
                E("#CALC!"),
                "MS-XLSX _xlfn.FILTER",
            ),
            (
                "=FILTER(B2:B5,C2:C5>10,\"none\")",
                T("none"),
                "MS-XLSX _xlfn.FILTER",
            ),
            (
                "=FILTER(A2:C5,A2:A5>3)",
                A("{4,\"date\",2}"),
                "MS-XLSX _xlfn.FILTER",
            ),
            (
                "=FILTER(B2:B5,C2:C4>1)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.FILTER",
            ),
            ("=SORT({3;1;2})", A("{1;2;3}"), "MS-XLSX _xlfn.SORT"),
            ("=SORT({3;1;2},1,-1)", A("{3;2;1}"), "MS-XLSX _xlfn.SORT"),
            (
                "=SORT(B2:C5,2,-1)",
                A("{\"cherry\",3;\"date\",2;\"apple\",1.5;\"banana\",0.25}"),
                "MS-XLSX _xlfn.SORT",
            ),
            (
                "=SORT({3,1,2},1,1,TRUE)",
                A("{1,2,3}"),
                "MS-XLSX _xlfn.SORT",
            ),
            (
                "=SORT({\"b\";\"A\";\"c\"})",
                A("{\"A\";\"b\";\"c\"}"),
                "MS-XLSX _xlfn.SORT",
            ),
            (
                "=SORTBY(B2:B5,C2:C5)",
                A("{\"banana\";\"apple\";\"date\";\"cherry\"}"),
                "MS-XLSX _xlfn.SORTBY",
            ),
            (
                "=SORTBY(B2:B5,C2:C5,-1)",
                A("{\"cherry\";\"date\";\"apple\";\"banana\"}"),
                "MS-XLSX _xlfn.SORTBY",
            ),
            ("=UNIQUE({1;2;2;3;1})", A("{1;2;3}"), "MS-XLSX _xlfn.UNIQUE"),
            (
                "=UNIQUE({1;2;2;3;1},FALSE,TRUE)",
                A("{3}"),
                "MS-XLSX _xlfn.UNIQUE",
            ),
            (
                "=UNIQUE({\"a\";\"A\";\"b\"})",
                A("{\"a\";\"b\"}"),
                "MS-XLSX _xlfn.UNIQUE",
            ),
            ("=UNIQUE({1,1,2},TRUE)", A("{1,2}"), "MS-XLSX _xlfn.UNIQUE"),
        ],
    );
}

#[test]
fn array_shaping() {
    check(
        DATA,
        &[
            (
                "=TRANSPOSE({1,2,3})",
                A("{1;2;3}"),
                "ECMA-376 §18.17.7 TRANSPOSE",
            ),
            (
                "=TRANSPOSE({1,2;3,4})",
                A("{1,3;2,4}"),
                "ECMA-376 §18.17.7 TRANSPOSE",
            ),
            (
                "=TRANSPOSE(A2:B3)",
                A("{1,2;\"apple\",\"banana\"}"),
                "ECMA-376 §18.17.7 TRANSPOSE",
            ),
            ("=HSTACK({1,2},{3})", A("{1,2,3}"), "MS-XLSX _xlfn.HSTACK"),
            (
                "=VSTACK({1,2},{3,4})",
                A("{1,2;3,4}"),
                "MS-XLSX _xlfn.VSTACK",
            ),
            (
                "=VSTACK({1,2},{3})",
                A("{1,2;3,#N/A}"),
                "MS-XLSX _xlfn.VSTACK",
            ),
            (
                "=CHOOSECOLS({1,2,3;4,5,6},1,3)",
                A("{1,3;4,6}"),
                "MS-XLSX _xlfn.CHOOSECOLS",
            ),
            (
                "=CHOOSECOLS({1,2,3},-1)",
                N(3.0),
                "MS-XLSX _xlfn.CHOOSECOLS",
            ),
            (
                "=CHOOSECOLS({1,2},3)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.CHOOSECOLS",
            ),
            (
                "=CHOOSEROWS({1,2;3,4;5,6},2)",
                A("{3,4}"),
                "MS-XLSX _xlfn.CHOOSEROWS",
            ),
            ("=TAKE({1,2,3;4,5,6},1)", A("{1,2,3}"), "MS-XLSX _xlfn.TAKE"),
            ("=TAKE({1,2,3;4,5,6},,-1)", A("{3;6}"), "MS-XLSX _xlfn.TAKE"),
            ("=DROP({1,2,3;4,5,6},1)", A("{4,5,6}"), "MS-XLSX _xlfn.DROP"),
            ("=DROP({1,2,3;4,5,6},,2)", A("{3;6}"), "MS-XLSX _xlfn.DROP"),
            ("=DROP({1,2},1)", E("#CALC!"), "MS-XLSX _xlfn.DROP"),
        ],
    );
}

/// Additional value, edge and error rows for the Lookup & Reference list (gap #37).
#[test]
fn lookup_edge_cases() {
    check(
        DATA,
        &[
            (
                "=HLOOKUP(\"Name\",A1:C5,3)",
                T("banana"),
                "hand: header row ID<Name<Price is sorted; row 3 of column B",
            ),
            (
                "=HLOOKUP(\"Price\",A1:C5,9,FALSE)",
                E("#REF!"),
                "ECMA-376 §18.17.7 HLOOKUP; hand: row_index > rows",
            ),
            (
                "=HLOOKUP(\"Price\",A1:C5,0,FALSE)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 HLOOKUP; hand: row_index < 1",
            ),
            (
                "=VLOOKUP(\"apple\",B2:C5,2,FALSE)",
                N(1.5),
                "hand: row 1 of B2:C5",
            ),
            (
                "=LOOKUP(\"zzz\",B2:B5)",
                T("date"),
                "ECMA-376 §18.17.7 LOOKUP; hand: larger than all → last",
            ),
            ("=ROW(B5)", N(5.0), "hand: row of B5"),
            (
                "=ROW(B5:C7)",
                A("{5;6;7}"),
                "ECMA-376 §18.17.7 ROW; hand: vertical array of rows",
            ),
            (
                "=ROW(Sheet2!A9)",
                N(9.0),
                "hand: row of A9 on another sheet",
            ),
            ("=COLUMN(Z1)", N(26.0), "hand: Z is the 26th column"),
            (
                "=COLUMN(B2:D9)",
                A("{2,3,4}"),
                "ECMA-376 §18.17.7 COLUMN; hand: horizontal array of columns",
            ),
            ("=COLUMNS({1,2,3})", N(3.0), "hand: 3 columns"),
            ("=COLUMNS(A:C)", N(3.0), "hand: A..C"),
            (
                "=COLUMNS(5)",
                N(1.0),
                "ECMA-376 §18.17.7 COLUMNS; hand: scalar is 1×1",
            ),
            ("=ROWS(A1)", N(1.0), "hand: single cell"),
            (
                "=INDEX(A2:C5,1,4)",
                E("#REF!"),
                "ECMA-376 §18.17.7 INDEX; hand: column out of range",
            ),
            (
                "=MATCH(\"x\",{1,2,3},0)",
                E("#N/A"),
                "ECMA-376 §18.17.7 MATCH; hand: not found",
            ),
            (
                "=XLOOKUP(4,A2:A5,B2:B5,,0,-1)",
                T("date"),
                "MS-XLSX _xlfn.XLOOKUP; hand: last-to-first finds A5",
            ),
            (
                "=XMATCH(\"b?nana\",B2:B5,2)",
                N(2.0),
                "MS-XLSX _xlfn.XMATCH; hand: wildcard ? matches a",
            ),
            (
                "=CHOOSE(1.9,\"a\",\"b\")",
                T("a"),
                "ECMA-376 §18.17.7 CHOOSE; hand: index truncated to 1",
            ),
            ("=OFFSET(C5,0,0)", N(2.0), "hand: zero offset"),
            (
                "=OFFSET(A1,0,0,0,1)",
                E("#REF!"),
                "ECMA-376 §18.17.7 OFFSET; hand: height 0",
            ),
            (
                "=INDIRECT(\"Z1\")",
                N(0.0),
                "ECMA-376 §18.17.7 INDIRECT; hand: blank cell → 0",
            ),
            (
                "=ADDRESS(5,26,3)",
                T("$Z5"),
                "ECMA-376 §18.17.7 ADDRESS; hand: abs_num 3 = absolute column",
            ),
            (
                "=ADDRESS(1,1,5)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 ADDRESS; hand: abs_num outside 1-4",
            ),
            (
                "=FILTER({1;2;3},{TRUE;FALSE;TRUE})",
                A("{1;3}"),
                "MS-XLSX _xlfn.FILTER; hand: rows 1 and 3 kept",
            ),
            (
                "=SORT({2,1;1,2},2)",
                A("{2,1;1,2}"),
                "MS-XLSX _xlfn.SORT; hand: by column 2 ascending",
            ),
            (
                "=SORT({1;2},3)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.SORT; hand: sort_index beyond columns",
            ),
            (
                "=SORTBY({\"a\";\"b\";\"c\"},{3;1;2})",
                A("{\"b\";\"c\";\"a\"}"),
                "MS-XLSX _xlfn.SORTBY; hand: keys 1,2,3 → b,c,a",
            ),
            (
                "=SORTBY({1;2;3},{1;2})",
                E("#VALUE!"),
                "MS-XLSX _xlfn.SORTBY; hand: by_array size mismatch",
            ),
            (
                "=UNIQUE({1;1;1})",
                N(1.0),
                "MS-XLSX _xlfn.UNIQUE; hand: single distinct value",
            ),
            ("=TRANSPOSE(5)", N(5.0), "hand: scalar transposes to itself"),
            ("=TRANSPOSE({1;2})", A("{1,2}"), "hand: column → row"),
            (
                "=TRANSPOSE(#N/A)",
                E("#N/A"),
                "ECMA-376 §18.17.7 TRANSPOSE; hand: error propagates",
            ),
        ],
    );
}
