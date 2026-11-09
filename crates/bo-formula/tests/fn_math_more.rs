//! ROMAN, ARABIC, BASE, DECIMAL, MMULT, MDETERM, MINVERSE, MUNIT, PERCENTOF, SERIESSUM.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (ROMAN forms, MMULT, MDETERM, MINVERSE, SERIESSUM);
//! [MS-XLSX] future functions (ARABIC, BASE, DECIMAL, MUNIT, PERCENTOF). Expected values are
//! worked by hand (determinants by cofactor expansion, checked with NumPy).

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[];

#[test]
fn numerals_and_bases() {
    check(
        DATA,
        &[
            ("=ROMAN(499)", T("CDXCIX"), "hand: CD + XC + IX"),
            (
                "=ROMAN(499,1)",
                T("LDVLIV"),
                "hand: form 1 allows LD and VL",
            ),
            ("=ROMAN(499,2)", T("XDIX"), "hand: form 2 allows XD"),
            ("=ROMAN(499,3)", T("VDIV"), "hand: form 3 allows VD"),
            ("=ROMAN(499,4)", T("ID"), "hand: form 4 allows ID"),
            (
                "=ROMAN(499,FALSE)",
                T("ID"),
                "ECMA-376 §18.17.7 ROMAN: FALSE is the most concise form",
            ),
            (
                "=ROMAN(499,TRUE)",
                T("CDXCIX"),
                "ECMA-376 §18.17.7 ROMAN: TRUE is classic",
            ),
            ("=ROMAN(1999)", T("MCMXCIX"), "hand: M + CM + XC + IX"),
            ("=ROMAN(0)", T(""), "ECMA-376 §18.17.7 ROMAN"),
            ("=ROMAN(4000)", E("#VALUE!"), "ECMA-376 §18.17.7 ROMAN"),
            (
                "=ARABIC(\"MCMXCIX\")",
                N(1999.0),
                "hand: 1000 + 900 + 90 + 9",
            ),
            (
                "=ARABIC(\"mmxxvi\")",
                N(2026.0),
                "hand: lower case accepted",
            ),
            (
                "=ARABIC(\"-XL\")",
                N(-40.0),
                "MS-XLSX _xlfn.ARABIC: a leading minus",
            ),
            ("=ARABIC(\"\")", N(0.0), "MS-XLSX _xlfn.ARABIC"),
            ("=ARABIC(\"ABC\")", E("#VALUE!"), "MS-XLSX _xlfn.ARABIC"),
            ("=BASE(7,2)", T("111"), "hand: 4+2+1"),
            ("=BASE(100,16)", T("64"), "hand: 6*16+4"),
            ("=BASE(15,2,10)", T("0000001111"), "hand: padded to 10"),
            ("=BASE(-1,2)", E("#NUM!"), "MS-XLSX _xlfn.BASE"),
            ("=BASE(5,37)", E("#NUM!"), "MS-XLSX _xlfn.BASE"),
            ("=DECIMAL(\"FF\",16)", N(255.0), "hand: 15*16+15"),
            (
                "=DECIMAL(\"zap\",36)",
                N(45_745.0),
                "hand: 35*1296 + 10*36 + 25",
            ),
            ("=DECIMAL(\"111\",2)", N(7.0), "hand: 4+2+1"),
            ("=DECIMAL(\"2\",2)", E("#NUM!"), "MS-XLSX _xlfn.DECIMAL"),
        ],
    );
}

#[test]
fn matrices_and_series() {
    check(
        DATA,
        &[
            (
                "=MMULT({1,3;7,2},{2,0;0,2})",
                A("{2,6;14,4}"),
                "hand: multiplying by 2I",
            ),
            ("=MMULT({1,2;3,4},{5;6})", A("{17;39}"), "hand: 5+12, 15+24"),
            (
                "=MMULT({1,2},{1,2})",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 MMULT",
            ),
            ("=MDETERM({3,6;1,1})", N(-3.0), "hand: 3*1 - 6*1"),
            (
                "=MDETERM({1,3,8,5;1,3,6,1;1,1,1,0;7,3,10,2})",
                N(88.0),
                "hand: cofactor expansion (NumPy det)",
            ),
            ("=MDETERM({1,2;2,4})", N(0.0), "hand: singular"),
            (
                "=MDETERM({1,2,3})",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 MDETERM",
            ),
            (
                "=MINVERSE({4,-1;2,0})",
                A("{0,0.5;-1,2}"),
                "hand: adjugate / determinant 2",
            ),
            (
                "=MINVERSE({1,2;2,4})",
                E("#NUM!"),
                "ECMA-376 §18.17.7 MINVERSE",
            ),
            ("=MUNIT(3)", A("{1,0,0;0,1,0;0,0,1}"), "MS-XLSX _xlfn.MUNIT"),
            ("=MUNIT(0)", E("#VALUE!"), "MS-XLSX _xlfn.MUNIT"),
            ("=PERCENTOF(25,100)", N(0.25), "hand: 25/100"),
            ("=PERCENTOF({1,2},{1,2,3,4})", N(0.3), "hand: 3/10"),
            ("=PERCENTOF(1,0)", E("#DIV/0!"), "MS-XLSX _xlfn.PERCENTOF"),
            (
                "=SERIESSUM(0.5,1,1,{1,2,3})",
                N(1.375),
                "hand: 0.5 + 2*0.25 + 3*0.125",
            ),
            ("=SERIESSUM(2,0,2,{1,1})", N(5.0), "hand: 1 + 2^2"),
        ],
    );
}
