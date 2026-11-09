//! ADDRESS, EXPAND, TOCOL, TOROW, WRAPROWS, WRAPCOLS, TRIMRANGE, GROUPBY, PIVOTBY, HYPERLINK,
//! GETPIVOTDATA, IMAGE, RTD and eta-reduced function arguments.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (ADDRESS, HYPERLINK, GETPIVOTDATA, RTD); [MS-XLSX]
//! future functions for the rest. Expected values are worked by hand from the data below.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    ("A1", "East"),
    ("A2", "West"),
    ("A3", "East"),
    ("A4", "North"),
    ("A5", "West"),
    ("A6", "East"),
    ("B1", "10"),
    ("B2", "20"),
    ("B3", "30"),
    ("B4", "40"),
    ("B5", "50"),
    ("B6", "60"),
    ("C1", "Q1"),
    ("C2", "Q1"),
    ("C3", "Q2"),
    ("C4", "Q2"),
    ("C5", "Q1"),
    ("C6", "Q2"),
    ("E1", "Item"),
    ("F1", "Qty"),
    ("E2", "a"),
    ("F2", "1"),
    ("E3", "b"),
    ("F3", "2"),
    ("E4", "a"),
    ("F4", "3"),
    ("G1", "1"),
    ("G3", "3"),
    ("H2", "5"),
    ("H3", "6"),
];

#[test]
fn addresses_and_shaping() {
    check(
        DATA,
        &[
            ("=ADDRESS(2,3)", T("$C$2"), "ECMA-376 §18.17.7 ADDRESS"),
            ("=ADDRESS(2,3,2)", T("C$2"), "ECMA-376 §18.17.7 ADDRESS"),
            ("=ADDRESS(2,3,3)", T("$C2"), "ECMA-376 §18.17.7 ADDRESS"),
            ("=ADDRESS(1,27,4)", T("AA1"), "hand: column 27 is AA"),
            (
                "=ADDRESS(2,3,2,FALSE)",
                T("R2C[3]"),
                "ECMA-376 §18.17.7 ADDRESS",
            ),
            (
                "=ADDRESS(2,3,1,FALSE)",
                T("R2C3"),
                "ECMA-376 §18.17.7 ADDRESS",
            ),
            (
                "=ADDRESS(2,3,1,TRUE,\"Sheet 1\")",
                T("'Sheet 1'!$C$2"),
                "hand: a name with a space is quoted",
            ),
            (
                "=ADDRESS(2,3,1,TRUE,\"Data\")",
                T("Data!$C$2"),
                "hand: a plain name",
            ),
            ("=ADDRESS(0,1)", E("#VALUE!"), "ECMA-376 §18.17.7 ADDRESS"),
            (
                "=EXPAND({1,2;3,4},3,3,0)",
                A("{1,2,0;3,4,0;0,0,0}"),
                "MS-XLSX _xlfn.EXPAND",
            ),
            (
                "=INDEX(EXPAND({1},2),2,1)",
                E("#N/A"),
                "MS-XLSX _xlfn.EXPAND: default padding",
            ),
            (
                "=EXPAND({1,2},1,1)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.EXPAND: smaller than the array",
            ),
            ("=TOCOL({1,2;3,4})", A("{1;2;3;4}"), "MS-XLSX _xlfn.TOCOL"),
            (
                "=TOCOL({1,2;3,4},0,TRUE)",
                A("{1;3;2;4}"),
                "MS-XLSX _xlfn.TOCOL: by column",
            ),
            ("=TOROW({1,2;3,4})", A("{1,2,3,4}"), "MS-XLSX _xlfn.TOROW"),
            (
                "=TOCOL(G1:G3,1)",
                A("{1;3}"),
                "MS-XLSX _xlfn.TOCOL: blanks skipped",
            ),
            (
                "=TOCOL({1,#N/A,2},2)",
                A("{1;2}"),
                "MS-XLSX _xlfn.TOCOL: errors skipped",
            ),
            (
                "=WRAPROWS({1,2,3,4,5},2,0)",
                A("{1,2;3,4;5,0}"),
                "MS-XLSX _xlfn.WRAPROWS",
            ),
            (
                "=WRAPCOLS({1,2,3,4,5},2,0)",
                A("{1,3,5;2,4,0}"),
                "MS-XLSX _xlfn.WRAPCOLS",
            ),
            (
                "=WRAPROWS({1,2;3,4},2)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.WRAPROWS: not a vector",
            ),
            ("=WRAPROWS({1,2},0)", E("#NUM!"), "MS-XLSX _xlfn.WRAPROWS"),
            (
                "=ROWS(TRIMRANGE(H1:H5))",
                N(2.0),
                "hand: rows 2-3 hold data",
            ),
            ("=SUM(TRIMRANGE(H1:H5))", N(11.0), "hand: 5+6"),
            (
                "=ROWS(TRIMRANGE(H1:H5,2))",
                N(3.0),
                "hand: only trailing blanks trimmed",
            ),
            (
                "=HYPERLINK(\"https://example.vn\",\"Trang\")",
                T("Trang"),
                "ECMA-376 §18.17.7 HYPERLINK",
            ),
            (
                "=HYPERLINK(\"https://example.vn\")",
                T("https://example.vn"),
                "ECMA-376 §18.17.7 HYPERLINK",
            ),
            (
                "=GETPIVOTDATA(\"Sales\",A1)",
                E("#REF!"),
                "ECMA-376 §18.17.7 GETPIVOTDATA: not in a pivot table",
            ),
            (
                "=RTD(\"srv.prog\",\"\",\"topic\")",
                E("#N/A"),
                "ECMA-376 §18.17.7 RTD: no server",
            ),
            (
                "=IMAGE(\"https://example.vn/a.png\",\"Logo\")",
                T("Logo"),
                "hand: the alternative text",
            ),
            (
                "=BYROW({1,2;3,4},SUM)",
                A("{3;7}"),
                "hand: SUM passed as a function",
            ),
        ],
    );
}

#[test]
fn grouping() {
    check(
        DATA,
        &[
            (
                "=GROUPBY(A1:A6,B1:B6,SUM)",
                A("{\"East\",100;\"North\",40;\"West\",70;\"Total\",210}"),
                "hand: sums per region, sorted, grand total",
            ),
            (
                "=GROUPBY(A1:A6,B1:B6,SUM,0,0)",
                A("{\"East\",100;\"North\",40;\"West\",70}"),
                "hand: no total",
            ),
            (
                "=GROUPBY(A1:A6,B1:B6,SUM,0,1,-2)",
                A("{\"East\",100;\"West\",70;\"North\",40;\"Total\",210}"),
                "hand: sorted by the value, descending",
            ),
            (
                "=GROUPBY(A1:A6,B1:B6,COUNT,0,-1)",
                A("{\"Total\",6;\"East\",3;\"North\",1;\"West\",2}"),
                "hand: counts, total on top",
            ),
            (
                "=GROUPBY(A1:A6,B1:B6,LAMBDA(x,MAX(x)),0,0)",
                A("{\"East\",60;\"North\",40;\"West\",50}"),
                "hand: largest per region",
            ),
            (
                "=GROUPBY(A1:A6,B1:B6,SUM,0,1,1,B1:B6>15)",
                A("{\"East\",90;\"North\",40;\"West\",70;\"Total\",200}"),
                "hand: rows above 15 only",
            ),
            (
                "=GROUPBY(E1:E4,F1:F4,SUM)",
                A("{\"Item\",\"Qty\";\"a\",4;\"b\",2;\"Total\",6}"),
                "hand: headers detected and shown",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),1,2)",
                T("Q1"),
                "hand: column keys across",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),1,4)",
                T("Total"),
                "hand: total column",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),2,1)",
                T("East"),
                "hand: row keys down",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),2,3)",
                N(90.0),
                "hand: East Q2 30+60",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),4,2)",
                N(70.0),
                "hand: West Q1 20+50",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),5,3)",
                N(130.0),
                "hand: Q2 total",
            ),
            (
                "=INDEX(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM),5,4)",
                N(210.0),
                "hand: grand total",
            ),
            (
                "=ROWS(PIVOTBY(A1:A6,C1:C6,B1:B6,SUM))",
                N(5.0),
                "hand: header, 3 regions, total",
            ),
        ],
    );
}
