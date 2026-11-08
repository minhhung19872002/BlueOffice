//! Logical functions, LET, LAMBDA and the LAMBDA helper functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (IF, AND, OR, NOT, TRUE, FALSE, IFERROR)
//! and OpenFormula (ODF 1.3 Part 4) Logical functions (IFNA, IFS, SWITCH, XOR);
//! LET / LAMBDA / MAP / REDUCE / SCAN / BYROW / BYCOL / MAKEARRAY follow their
//! published function definitions (dynamic-array function reference).

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[("A1", "1"), ("A2", "2"), ("A3", "0"), ("B1", "TRUE")];

#[test]
fn if_ifs_switch() {
    check(
        DATA,
        &[
            ("=IF(TRUE,1,2)", N(1.0), "ECMA-376 §18.17.7 IF"),
            ("=IF(FALSE,1)", B(false), "ECMA-376 §18.17.7 IF"),
            (
                "=IF(A1>0,\"pos\",\"neg\")",
                T("pos"),
                "ECMA-376 §18.17.7 IF",
            ),
            ("=IF(1,\"a\",\"b\")", T("a"), "ECMA-376 §18.17.7 IF"),
            ("=IF(0,\"a\",\"b\")", T("b"), "ECMA-376 §18.17.7 IF"),
            ("=IF(\"x\",1,2)", E("#VALUE!"), "ECMA-376 §18.17.7 IF"),
            ("=IF(#N/A,1,2)", E("#N/A"), "ECMA-376 §18.17.7 IF"),
            // Only the selected branch is evaluated.
            ("=IF(FALSE,1/0,5)", N(5.0), "ECMA-376 §18.17.7 IF"),
            ("=IF({TRUE,FALSE},1,2)", A("{1,2}"), "ECMA-376 §18.17.7 IF"),
            (
                "=IF(A1:A3>0,\"y\",\"n\")",
                A("{\"y\";\"y\";\"n\"}"),
                "ECMA-376 §18.17.7 IF",
            ),
            (
                "=IFS(A1=1,\"one\",TRUE,\"other\")",
                T("one"),
                "MS-XLSX _xlfn.IFS",
            ),
            (
                "=IFS(A1=5,\"five\",TRUE,\"other\")",
                T("other"),
                "MS-XLSX _xlfn.IFS",
            ),
            ("=IFS(FALSE,1)", E("#N/A"), "MS-XLSX _xlfn.IFS"),
            (
                "=SWITCH(2,1,\"a\",2,\"b\",\"c\")",
                T("b"),
                "MS-XLSX _xlfn.SWITCH",
            ),
            (
                "=SWITCH(9,1,\"a\",2,\"b\",\"c\")",
                T("c"),
                "MS-XLSX _xlfn.SWITCH",
            ),
            ("=SWITCH(9,1,\"a\")", E("#N/A"), "MS-XLSX _xlfn.SWITCH"),
            (
                "=SWITCH(\"x\",\"X\",\"up\")",
                T("up"),
                "MS-XLSX _xlfn.SWITCH",
            ),
        ],
    );
}

#[test]
fn and_or_xor_not() {
    check(
        DATA,
        &[
            ("=AND(TRUE,TRUE)", B(true), "ECMA-376 §18.17.7 AND"),
            ("=AND(TRUE,FALSE)", B(false), "ECMA-376 §18.17.7 AND"),
            ("=AND(2+2=4,2+3=5)", B(true), "ECMA-376 §18.17.7 AND"),
            ("=AND(A1:A2)", B(true), "ECMA-376 §18.17.7 AND"),
            ("=AND(A1:A3)", B(false), "ECMA-376 §18.17.7 AND"),
            ("=AND(Z1:Z3)", E("#VALUE!"), "ECMA-376 §18.17.7 AND"),
            ("=AND(\"a\")", E("#VALUE!"), "ECMA-376 §18.17.7 AND"),
            ("=AND(TRUE,#N/A)", E("#N/A"), "ECMA-376 §18.17.7 AND"),
            ("=OR(FALSE,FALSE)", B(false), "ECMA-376 §18.17.7 OR"),
            ("=OR(TRUE,FALSE)", B(true), "ECMA-376 §18.17.7 OR"),
            ("=OR(1+1=1,2+2=5)", B(false), "ECMA-376 §18.17.7 OR"),
            ("=OR({FALSE,TRUE})", B(true), "ECMA-376 §18.17.7 OR"),
            ("=OR(A3,B1)", B(true), "ECMA-376 §18.17.7 OR"),
            ("=XOR(3>0,2<9)", B(false), "OpenFormula XOR"),
            ("=XOR(3>12,4>6)", B(false), "OpenFormula XOR"),
            ("=XOR(TRUE,FALSE)", B(true), "OpenFormula XOR"),
            ("=XOR(TRUE,TRUE,TRUE)", B(true), "OpenFormula XOR"),
            ("=NOT(FALSE)", B(true), "ECMA-376 §18.17.7 NOT"),
            ("=NOT(1+1=2)", B(false), "ECMA-376 §18.17.7 NOT"),
            ("=NOT(0)", B(true), "ECMA-376 §18.17.7 NOT"),
            ("=NOT(\"a\")", E("#VALUE!"), "ECMA-376 §18.17.7 NOT"),
            ("=TRUE()", B(true), "ECMA-376 §18.17.7 TRUE"),
            ("=FALSE()", B(false), "ECMA-376 §18.17.7 FALSE"),
        ],
    );
}

#[test]
fn iferror_ifna() {
    check(
        DATA,
        &[
            (
                "=IFERROR(1/0,\"err\")",
                T("err"),
                "ECMA-376 §18.17.7 IFERROR",
            ),
            ("=IFERROR(5,\"err\")", N(5.0), "ECMA-376 §18.17.7 IFERROR"),
            ("=IFERROR(#N/A,0)", N(0.0), "ECMA-376 §18.17.7 IFERROR"),
            (
                "=IFERROR({1,#N/A},0)",
                A("{1,0}"),
                "ECMA-376 §18.17.7 IFERROR",
            ),
            ("=IFNA(#N/A,\"na\")", T("na"), "OpenFormula IFNA"),
            ("=IFNA(1/0,\"na\")", E("#DIV/0!"), "OpenFormula IFNA"),
            ("=IFNA(3,\"x\")", N(3.0), "OpenFormula IFNA"),
        ],
    );
}

#[test]
fn let_lambda() {
    check(
        DATA,
        &[
            ("=LET(x,2,x*x)", N(4.0), "MS-XLSX _xlfn.LET"),
            ("=LET(x,2,y,x+1,x*y)", N(6.0), "MS-XLSX _xlfn.LET"),
            ("=LET(x,SEQUENCE(3),SUM(x))", N(6.0), "MS-XLSX _xlfn.LET"),
            ("=LET(x,A1:A2,SUM(x)*10)", N(30.0), "MS-XLSX _xlfn.LET"),
            ("=LET(f,LAMBDA(x,x*3),f(4))", N(12.0), "MS-XLSX _xlfn.LET"),
            ("=LET(1,2,3)", E("#VALUE!"), "MS-XLSX _xlfn.LET"),
            ("=LAMBDA(x,x+1)(2)", N(3.0), "MS-XLSX _xlfn.LAMBDA"),
            ("=LAMBDA(x,y,x*y)(3,4)", N(12.0), "MS-XLSX _xlfn.LAMBDA"),
            ("=LAMBDA(x,x)", E("#CALC!"), "MS-XLSX _xlfn.LAMBDA"),
            ("=LAMBDA(x,x+1)(1,2)", E("#VALUE!"), "MS-XLSX _xlfn.LAMBDA"),
            // Closures capture LET bindings.
            (
                "=LET(k,10,f,LAMBDA(x,x+k),f(1))",
                N(11.0),
                "MS-XLSX _xlfn.LET",
            ),
        ],
    );
}

#[test]
fn lambda_helpers() {
    check(
        DATA,
        &[
            (
                "=MAP(SEQUENCE(3),LAMBDA(x,x*2))",
                A("{2;4;6}"),
                "MS-XLSX _xlfn.MAP",
            ),
            (
                "=MAP({1,2},{10,20},LAMBDA(a,b,a+b))",
                A("{11,22}"),
                "MS-XLSX _xlfn.MAP",
            ),
            (
                "=REDUCE(0,SEQUENCE(4),LAMBDA(a,b,a+b))",
                N(10.0),
                "MS-XLSX _xlfn.REDUCE",
            ),
            (
                "=SCAN(0,{1,2,3},LAMBDA(a,b,a+b))",
                A("{1,3,6}"),
                "MS-XLSX _xlfn.SCAN",
            ),
            (
                "=BYROW({1,2;3,4},LAMBDA(r,SUM(r)))",
                A("{3;7}"),
                "MS-XLSX _xlfn.BYROW",
            ),
            (
                "=BYCOL({1,2;3,4},LAMBDA(c,SUM(c)))",
                A("{4,6}"),
                "MS-XLSX _xlfn.BYCOL",
            ),
            (
                "=MAKEARRAY(2,2,LAMBDA(r,c,r*10+c))",
                A("{11,12;21,22}"),
                "MS-XLSX _xlfn.MAKEARRAY",
            ),
            ("=MAP(SEQUENCE(2),5)", E("#VALUE!"), "MS-XLSX _xlfn.MAP"),
        ],
    );
}

/// A workbook-scoped name holding a LAMBDA is callable like a function.
#[test]
fn named_lambda() {
    let mut wb = workbook(&[("A1", "5")], Opts::default());
    wb.define_name("DOUBLE", None, "=LAMBDA(x,x*2)")
        .expect("valid name");
    wb.define_name("TAX", None, "=0.1").expect("valid name");
    wb.set_a1(0, "B1", "=DOUBLE(A1)").expect("valid formula");
    wb.set_a1(0, "B2", "=A1*(1+TAX)").expect("valid formula");
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B1"), bo_formula::Value::Number(10.0));
    assert_eq!(wb.value_a1(0, "B2"), bo_formula::Value::Number(5.5));
}

/// Additional value, edge and error rows for the Logical list (gap #37).
#[test]
fn logical_edge_cases() {
    check(
        DATA,
        &[
            (
                "=IFS(A1=2,\"a\",A2=2,\"b\")",
                T("b"),
                "MS-XLSX _xlfn.IFS; hand: first TRUE test is A2=2",
            ),
            (
                "=IFS(\"x\",1)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IFS; hand: test is not logical",
            ),
            (
                "=IFS(1/0,1)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.IFS; hand: error in test propagates",
            ),
            (
                "=IFNA({1,#N/A},0)",
                A("{1,0}"),
                "OpenFormula IFNA; hand: element-wise replacement",
            ),
            (
                "=IFNA(\"\",1)",
                T(""),
                "OpenFormula IFNA; hand: empty text is not #N/A",
            ),
            ("=IFNA(NA(),A1+1)", N(2.0), "OpenFormula IFNA; hand: A1+1"),
            (
                "=IFERROR(\"\",1)",
                T(""),
                "ECMA-376 §18.17.7 IFERROR; hand: no error",
            ),
            (
                "=SWITCH(A3,0,\"zero\",\"other\")",
                T("zero"),
                "MS-XLSX _xlfn.SWITCH; hand: A3 = 0",
            ),
            (
                "=SWITCH(1/0,1,\"a\")",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.SWITCH; hand: error in expression",
            ),
            (
                "=XOR(A1:A3)",
                B(false),
                "OpenFormula XOR; hand: two TRUE values (1,2) → even",
            ),
            (
                "=XOR(\"a\")",
                E("#VALUE!"),
                "OpenFormula XOR; hand: no logical values",
            ),
            ("=NOT(A3)", B(true), "hand: A3 = 0 is FALSE"),
            (
                "=OR(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 OR; hand: no logical values",
            ),
            (
                "=OR(Z1:Z3,FALSE)",
                B(false),
                "ECMA-376 §18.17.7 OR; hand: blanks ignored",
            ),
            (
                "=TRUE()+1",
                N(2.0),
                "ECMA-376 §18.17.7 TRUE; hand: TRUE = 1 in arithmetic",
            ),
            (
                "=FALSE()*5",
                N(0.0),
                "ECMA-376 §18.17.7 FALSE; hand: FALSE = 0 in arithmetic",
            ),
            (
                "=IF(Z1,\"y\",\"n\")",
                T("n"),
                "ECMA-376 §18.17.7 IF; hand: blank is FALSE",
            ),
            (
                "=LET(x,1/0,5)",
                N(5.0),
                "MS-XLSX _xlfn.LET; hand: unused error binding not returned",
            ),
            (
                "=LAMBDA(x,y,x+y)(1)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.LAMBDA; hand: missing argument",
            ),
        ],
    );
}
