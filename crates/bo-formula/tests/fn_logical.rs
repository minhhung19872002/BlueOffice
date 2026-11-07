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
            ("=IF(TRUE,1,2)", N(1.0)),
            ("=IF(FALSE,1)", B(false)),
            ("=IF(A1>0,\"pos\",\"neg\")", T("pos")),
            ("=IF(1,\"a\",\"b\")", T("a")),
            ("=IF(0,\"a\",\"b\")", T("b")),
            ("=IF(\"x\",1,2)", E("#VALUE!")),
            ("=IF(#N/A,1,2)", E("#N/A")),
            // Only the selected branch is evaluated.
            ("=IF(FALSE,1/0,5)", N(5.0)),
            ("=IF({TRUE,FALSE},1,2)", A("{1,2}")),
            ("=IF(A1:A3>0,\"y\",\"n\")", A("{\"y\";\"y\";\"n\"}")),
            ("=IFS(A1=1,\"one\",TRUE,\"other\")", T("one")),
            ("=IFS(A1=5,\"five\",TRUE,\"other\")", T("other")),
            ("=IFS(FALSE,1)", E("#N/A")),
            ("=SWITCH(2,1,\"a\",2,\"b\",\"c\")", T("b")),
            ("=SWITCH(9,1,\"a\",2,\"b\",\"c\")", T("c")),
            ("=SWITCH(9,1,\"a\")", E("#N/A")),
            ("=SWITCH(\"x\",\"X\",\"up\")", T("up")),
        ],
    );
}

#[test]
fn and_or_xor_not() {
    check(
        DATA,
        &[
            ("=AND(TRUE,TRUE)", B(true)),
            ("=AND(TRUE,FALSE)", B(false)),
            ("=AND(2+2=4,2+3=5)", B(true)),
            ("=AND(A1:A2)", B(true)),
            ("=AND(A1:A3)", B(false)),
            ("=AND(Z1:Z3)", E("#VALUE!")),
            ("=AND(\"a\")", E("#VALUE!")),
            ("=AND(TRUE,#N/A)", E("#N/A")),
            ("=OR(FALSE,FALSE)", B(false)),
            ("=OR(TRUE,FALSE)", B(true)),
            ("=OR(1+1=1,2+2=5)", B(false)),
            ("=OR({FALSE,TRUE})", B(true)),
            ("=OR(A3,B1)", B(true)),
            ("=XOR(3>0,2<9)", B(false)),
            ("=XOR(3>12,4>6)", B(false)),
            ("=XOR(TRUE,FALSE)", B(true)),
            ("=XOR(TRUE,TRUE,TRUE)", B(true)),
            ("=NOT(FALSE)", B(true)),
            ("=NOT(1+1=2)", B(false)),
            ("=NOT(0)", B(true)),
            ("=NOT(\"a\")", E("#VALUE!")),
            ("=TRUE()", B(true)),
            ("=FALSE()", B(false)),
        ],
    );
}

#[test]
fn iferror_ifna() {
    check(
        DATA,
        &[
            ("=IFERROR(1/0,\"err\")", T("err")),
            ("=IFERROR(5,\"err\")", N(5.0)),
            ("=IFERROR(#N/A,0)", N(0.0)),
            ("=IFERROR({1,#N/A},0)", A("{1,0}")),
            ("=IFNA(#N/A,\"na\")", T("na")),
            ("=IFNA(1/0,\"na\")", E("#DIV/0!")),
            ("=IFNA(3,\"x\")", N(3.0)),
        ],
    );
}

#[test]
fn let_lambda() {
    check(
        DATA,
        &[
            ("=LET(x,2,x*x)", N(4.0)),
            ("=LET(x,2,y,x+1,x*y)", N(6.0)),
            ("=LET(x,SEQUENCE(3),SUM(x))", N(6.0)),
            ("=LET(x,A1:A2,SUM(x)*10)", N(30.0)),
            ("=LET(f,LAMBDA(x,x*3),f(4))", N(12.0)),
            ("=LET(1,2,3)", E("#VALUE!")),
            ("=LAMBDA(x,x+1)(2)", N(3.0)),
            ("=LAMBDA(x,y,x*y)(3,4)", N(12.0)),
            ("=LAMBDA(x,x)", E("#CALC!")),
            ("=LAMBDA(x,x+1)(1,2)", E("#VALUE!")),
            // Closures capture LET bindings.
            ("=LET(k,10,f,LAMBDA(x,x+k),f(1))", N(11.0)),
        ],
    );
}

#[test]
fn lambda_helpers() {
    check(
        DATA,
        &[
            ("=MAP(SEQUENCE(3),LAMBDA(x,x*2))", A("{2;4;6}")),
            ("=MAP({1,2},{10,20},LAMBDA(a,b,a+b))", A("{11,22}")),
            ("=REDUCE(0,SEQUENCE(4),LAMBDA(a,b,a+b))", N(10.0)),
            ("=SCAN(0,{1,2,3},LAMBDA(a,b,a+b))", A("{1,3,6}")),
            ("=BYROW({1,2;3,4},LAMBDA(r,SUM(r)))", A("{3;7}")),
            ("=BYCOL({1,2;3,4},LAMBDA(c,SUM(c)))", A("{4,6}")),
            ("=MAKEARRAY(2,2,LAMBDA(r,c,r*10+c))", A("{11,12;21,22}")),
            ("=MAP(SEQUENCE(2),5)", E("#VALUE!")),
        ],
    );
}

/// A workbook-scoped name holding a LAMBDA is callable like a function.
#[test]
fn named_lambda() {
    let mut wb = workbook(&[("A1", "5")], Opts::default());
    wb.define_name("DOUBLE", None, "=LAMBDA(x,x*2)").expect("valid name");
    wb.define_name("TAX", None, "=0.1").expect("valid name");
    wb.set_a1(0, "B1", "=DOUBLE(A1)").expect("valid formula");
    wb.set_a1(0, "B2", "=A1*(1+TAX)").expect("valid formula");
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B1"), bo_formula::Value::Number(10.0));
    assert_eq!(wb.value_a1(0, "B2"), bo_formula::Value::Number(5.5));
}
