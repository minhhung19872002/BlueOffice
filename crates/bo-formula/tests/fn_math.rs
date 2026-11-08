//! Math & trigonometry functions.
//!
//! Sources for expected values (public function definitions, implemented clean-room):
//! * ECMA-376 Part 1, §18.17.7 "Predefined Function Definitions" (per-function
//!   descriptions and worked examples), and
//! * OASIS OpenDocument 1.3 Part 4 "OpenFormula" — Mathematical and Rounding functions.
//!
//! Irrational results are the exact mathematical values rounded to the shown
//! digits (checked with an absolute tolerance via `Near`).

// Inputs such as 3.14159 are the published worked examples, not approximations of PI.
#![allow(clippy::approx_constant)]

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    ("A1", "1"),
    ("A2", "2"),
    ("A3", "3"),
    ("A4", "4"),
    ("A5", "5"),
    ("B1", "10"),
    ("B2", "20"),
    ("B3", "30"),
    ("B4", "40"),
    ("B5", "50"),
    ("D1", "'5"),
    ("D2", "TRUE"),
    ("D3", "1"),
    ("E1", "#DIV/0!"),
    ("E2", "7"),
    ("F1", "apple"),
    ("F2", "banana"),
    ("F3", "Apple"),
    ("F4", "cherry"),
];

/// SUM, PRODUCT, SUMSQ: direct arguments are coerced, text/booleans inside
/// references are ignored, errors propagate (§18.17.7 SUM; OpenFormula SUM).
#[test]
fn sum_product() {
    check(
        DATA,
        &[
            ("=SUM(1,2,3)", N(6.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(A1:A5)", N(15.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(A1:A5,10)", N(25.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(\"3\",2,TRUE)", N(6.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(D1:D3)", N(1.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(\"abc\")", E("#VALUE!"), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(E1:E2)", E("#DIV/0!"), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(A1:A5,#N/A)", E("#N/A"), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(A1:B2)", N(33.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM((A1:A2,B1:B2))", N(33.0), "ECMA-376 §18.17.7 SUM"),
            ("=SUM(Z1:Z10)", N(0.0), "ECMA-376 §18.17.7 SUM"),
            ("=PRODUCT(A1:A5)", N(120.0), "ECMA-376 §18.17.7 PRODUCT"),
            ("=PRODUCT(2,3,\"4\")", N(24.0), "ECMA-376 §18.17.7 PRODUCT"),
            ("=PRODUCT(A1:A3,2)", N(12.0), "ECMA-376 §18.17.7 PRODUCT"),
            ("=SUMSQ(3,4)", N(25.0), "ECMA-376 §18.17.7 SUMSQ"),
            ("=SUMSQ(A1:A3)", N(14.0), "ECMA-376 §18.17.7 SUMSQ"),
        ],
    );
}

/// SUMPRODUCT: arrays must have equal dimensions; non-numeric entries are 0
/// (§18.17.7 SUMPRODUCT).
#[test]
fn sumproduct() {
    check(
        DATA,
        &[
            (
                "=SUMPRODUCT(A1:A3,B1:B3)",
                N(140.0),
                "ECMA-376 §18.17.7 SUMPRODUCT",
            ),
            (
                "=SUMPRODUCT({1,2;3,4},{5,6;7,8})",
                N(70.0),
                "ECMA-376 §18.17.7 SUMPRODUCT",
            ),
            (
                "=SUMPRODUCT({1,\"a\"},{2,3})",
                N(2.0),
                "ECMA-376 §18.17.7 SUMPRODUCT",
            ),
            (
                "=SUMPRODUCT(A1:A3,B1:B2)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SUMPRODUCT",
            ),
            (
                "=SUMPRODUCT((A1:A5>2)*1)",
                N(3.0),
                "ECMA-376 §18.17.7 SUMPRODUCT",
            ),
            (
                "=SUMPRODUCT(A1:A5)",
                N(15.0),
                "ECMA-376 §18.17.7 SUMPRODUCT",
            ),
        ],
    );
}

/// SUMIF / SUMIFS criteria: comparison operators, case-insensitive text,
/// wildcards `*` `?` (§18.17.7 SUMIF, SUMIFS; OpenFormula SUMIF).
#[test]
fn sumif_sumifs() {
    check(
        DATA,
        &[
            ("=SUMIF(A1:A5,\">2\")", N(12.0), "ECMA-376 §18.17.7 SUMIF"),
            (
                "=SUMIF(A1:A5,\">2\",B1:B5)",
                N(120.0),
                "ECMA-376 §18.17.7 SUMIF",
            ),
            ("=SUMIF(A1:A5,3)", N(3.0), "ECMA-376 §18.17.7 SUMIF"),
            ("=SUMIF(A1:A5,\"=3\")", N(3.0), "ECMA-376 §18.17.7 SUMIF"),
            ("=SUMIF(A1:A5,\"<>3\")", N(12.0), "ECMA-376 §18.17.7 SUMIF"),
            (
                "=SUMIF(F1:F4,\"apple\",A1:A4)",
                N(4.0),
                "ECMA-376 §18.17.7 SUMIF",
            ),
            (
                "=SUMIF(F1:F4,\"b*\",A1:A4)",
                N(2.0),
                "ECMA-376 §18.17.7 SUMIF",
            ),
            (
                "=SUMIF(F1:F4,\"?????\",A1:A4)",
                N(4.0),
                "ECMA-376 §18.17.7 SUMIF",
            ),
            (
                "=SUMIF(F1:F4,\"<>apple\",A1:A4)",
                N(6.0),
                "ECMA-376 §18.17.7 SUMIF",
            ),
            (
                "=SUMIFS(B1:B5,A1:A5,\">1\",A1:A5,\"<5\")",
                N(90.0),
                "ECMA-376 §18.17.7 SUMIFS",
            ),
            (
                "=SUMIFS(B1:B4,F1:F4,\"apple\",A1:A4,\">1\")",
                N(30.0),
                "ECMA-376 §18.17.7 SUMIFS",
            ),
            (
                "=SUMIFS(B1:B5,A1:A4,\">1\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SUMIFS",
            ),
        ],
    );
}

/// Rounding family (§18.17.7 ROUND, ROUNDUP, ROUNDDOWN, MROUND, INT, TRUNC; worked examples).
#[test]
fn rounding() {
    check(
        &[],
        &[
            ("=ROUND(2.15,1)", N(2.2), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(2.149,1)", N(2.1), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(-1.475,2)", N(-1.48), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(21.5,-1)", N(20.0), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(626.3,-3)", N(1000.0), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(1.98,-1)", N(0.0), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(-50.55,-2)", N(-100.0), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(2.5,0)", N(3.0), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUND(-2.5,0)", N(-3.0), "ECMA-376 §18.17.7 ROUND"),
            ("=ROUNDUP(3.2,0)", N(4.0), "ECMA-376 §18.17.7 ROUNDUP"),
            ("=ROUNDUP(76.9,0)", N(77.0), "ECMA-376 §18.17.7 ROUNDUP"),
            ("=ROUNDUP(3.14159,3)", N(3.142), "ECMA-376 §18.17.7 ROUNDUP"),
            ("=ROUNDUP(-3.14159,1)", N(-3.2), "ECMA-376 §18.17.7 ROUNDUP"),
            (
                "=ROUNDUP(31415.92654,-2)",
                N(31500.0),
                "ECMA-376 §18.17.7 ROUNDUP",
            ),
            ("=ROUNDDOWN(3.2,0)", N(3.0), "ECMA-376 §18.17.7 ROUNDDOWN"),
            ("=ROUNDDOWN(76.9,0)", N(76.0), "ECMA-376 §18.17.7 ROUNDDOWN"),
            (
                "=ROUNDDOWN(3.14159,3)",
                N(3.141),
                "ECMA-376 §18.17.7 ROUNDDOWN",
            ),
            (
                "=ROUNDDOWN(-3.14159,1)",
                N(-3.1),
                "ECMA-376 §18.17.7 ROUNDDOWN",
            ),
            (
                "=ROUNDDOWN(31415.92654,-2)",
                N(31400.0),
                "ECMA-376 §18.17.7 ROUNDDOWN",
            ),
            ("=MROUND(10,3)", N(9.0), "ECMA-376 §18.17.7 MROUND"),
            ("=MROUND(-10,-3)", N(-9.0), "ECMA-376 §18.17.7 MROUND"),
            ("=MROUND(1.3,0.2)", N(1.4), "ECMA-376 §18.17.7 MROUND"),
            ("=MROUND(5,-2)", E("#NUM!"), "ECMA-376 §18.17.7 MROUND"),
            ("=INT(8.9)", N(8.0), "ECMA-376 §18.17.7 INT"),
            ("=INT(-8.9)", N(-9.0), "ECMA-376 §18.17.7 INT"),
            ("=TRUNC(8.9)", N(8.0), "ECMA-376 §18.17.7 TRUNC"),
            ("=TRUNC(-8.9)", N(-8.0), "ECMA-376 §18.17.7 TRUNC"),
            ("=TRUNC(0.45)", N(0.0), "ECMA-376 §18.17.7 TRUNC"),
            ("=TRUNC(3.14159,2)", N(3.14), "ECMA-376 §18.17.7 TRUNC"),
            ("=ROUND(\"x\",1)", E("#VALUE!"), "ECMA-376 §18.17.7 ROUND"),
        ],
    );
}

/// CEILING / FLOOR variants (§18.17.7 CEILING, FLOOR; CEILING.MATH, FLOOR.MATH,
/// CEILING.PRECISE, FLOOR.PRECISE, ISO.CEILING from [MS-XLSX] future functions;
/// worked examples of the published definitions).
#[test]
fn ceiling_floor() {
    check(
        &[],
        &[
            ("=CEILING(2.5,1)", N(3.0), "ECMA-376 §18.17.7 CEILING"),
            ("=CEILING(-2.5,-2)", N(-4.0), "ECMA-376 §18.17.7 CEILING"),
            ("=CEILING(-2.5,2)", N(-2.0), "ECMA-376 §18.17.7 CEILING"),
            ("=CEILING(1.5,0.1)", N(1.5), "ECMA-376 §18.17.7 CEILING"),
            ("=CEILING(0.234,0.01)", N(0.24), "ECMA-376 §18.17.7 CEILING"),
            ("=CEILING(2.5,-2)", E("#NUM!"), "ECMA-376 §18.17.7 CEILING"),
            (
                "=CEILING.MATH(24.3,5)",
                N(25.0),
                "MS-XLSX _xlfn.CEILING.MATH = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.MATH(6.7)",
                N(7.0),
                "MS-XLSX _xlfn.CEILING.MATH = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.MATH(-8.1,2)",
                N(-8.0),
                "MS-XLSX _xlfn.CEILING.MATH = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.MATH(-5.5,2,-1)",
                N(-6.0),
                "MS-XLSX _xlfn.CEILING.MATH = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.PRECISE(4.3)",
                N(5.0),
                "MS-XLSX _xlfn.CEILING.PRECISE = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.PRECISE(-4.3)",
                N(-4.0),
                "MS-XLSX _xlfn.CEILING.PRECISE = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.PRECISE(4.3,2)",
                N(6.0),
                "MS-XLSX _xlfn.CEILING.PRECISE = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.PRECISE(4.3,-2)",
                N(6.0),
                "MS-XLSX _xlfn.CEILING.PRECISE = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.PRECISE(-4.3,2)",
                N(-4.0),
                "MS-XLSX _xlfn.CEILING.PRECISE = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=CEILING.PRECISE(-4.3,-2)",
                N(-4.0),
                "MS-XLSX _xlfn.CEILING.PRECISE = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=ISO.CEILING(4.3)",
                N(5.0),
                "MS-XLSX _xlfn.ISO.CEILING = ECMA-376 §18.17.7 CEILING",
            ),
            (
                "=ISO.CEILING(-4.3,2)",
                N(-4.0),
                "MS-XLSX _xlfn.ISO.CEILING = ECMA-376 §18.17.7 CEILING",
            ),
            ("=FLOOR(3.7,2)", N(2.0), "ECMA-376 §18.17.7 FLOOR"),
            ("=FLOOR(-2.5,-2)", N(-2.0), "ECMA-376 §18.17.7 FLOOR"),
            ("=FLOOR(2.5,-2)", E("#NUM!"), "ECMA-376 §18.17.7 FLOOR"),
            ("=FLOOR(1.58,0.1)", N(1.5), "ECMA-376 §18.17.7 FLOOR"),
            ("=FLOOR(0.234,0.01)", N(0.23), "ECMA-376 §18.17.7 FLOOR"),
            ("=FLOOR(5,0)", E("#DIV/0!"), "ECMA-376 §18.17.7 FLOOR"),
            (
                "=FLOOR.MATH(24.3,5)",
                N(20.0),
                "MS-XLSX _xlfn.FLOOR.MATH = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.MATH(5.5,2)",
                N(4.0),
                "MS-XLSX _xlfn.FLOOR.MATH = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.MATH(-8.1,2)",
                N(-10.0),
                "MS-XLSX _xlfn.FLOOR.MATH = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.MATH(-5.5,2,-1)",
                N(-4.0),
                "MS-XLSX _xlfn.FLOOR.MATH = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.PRECISE(-3.2,-1)",
                N(-4.0),
                "MS-XLSX _xlfn.FLOOR.PRECISE = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.PRECISE(3.2,1)",
                N(3.0),
                "MS-XLSX _xlfn.FLOOR.PRECISE = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.PRECISE(-3.2,1)",
                N(-4.0),
                "MS-XLSX _xlfn.FLOOR.PRECISE = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.PRECISE(3.2,-1)",
                N(3.0),
                "MS-XLSX _xlfn.FLOOR.PRECISE = ECMA-376 §18.17.7 FLOOR",
            ),
            (
                "=FLOOR.PRECISE(3.2)",
                N(3.0),
                "MS-XLSX _xlfn.FLOOR.PRECISE = ECMA-376 §18.17.7 FLOOR",
            ),
        ],
    );
}

/// Arithmetic helpers (§18.17.7 MOD, ABS, POWER, SQRT, SQRTPI, SIGN, EVEN, ODD, QUOTIENT).
#[test]
fn arithmetic() {
    check(
        &[],
        &[
            ("=MOD(3,2)", N(1.0), "ECMA-376 §18.17.7 MOD"),
            ("=MOD(-3,2)", N(1.0), "ECMA-376 §18.17.7 MOD"),
            ("=MOD(3,-2)", N(-1.0), "ECMA-376 §18.17.7 MOD"),
            ("=MOD(-3,-2)", N(-1.0), "ECMA-376 §18.17.7 MOD"),
            ("=MOD(3,0)", E("#DIV/0!"), "ECMA-376 §18.17.7 MOD"),
            ("=ABS(-4)", N(4.0), "ECMA-376 §18.17.7 ABS"),
            ("=ABS(2)", N(2.0), "ECMA-376 §18.17.7 ABS"),
            ("=POWER(5,2)", N(25.0), "ECMA-376 §18.17.7 POWER"),
            (
                "=POWER(4,5/4)",
                Near(5.656_854_249, 1e-9),
                "ECMA-376 §18.17.7 POWER",
            ),
            ("=POWER(0,0)", E("#NUM!"), "ECMA-376 §18.17.7 POWER"),
            ("=POWER(0,-1)", E("#DIV/0!"), "ECMA-376 §18.17.7 POWER"),
            ("=POWER(-8,1/3)", E("#NUM!"), "ECMA-376 §18.17.7 POWER"),
            ("=2^10", N(1024.0), "ECMA-376 §18.17.2 formula syntax"),
            ("=SQRT(16)", N(4.0), "ECMA-376 §18.17.7 SQRT"),
            ("=SQRT(-16)", E("#NUM!"), "ECMA-376 §18.17.7 SQRT"),
            ("=SQRT(ABS(-16))", N(4.0), "ECMA-376 §18.17.7 SQRT"),
            (
                "=SQRTPI(1)",
                Near(1.772_453_851, 1e-9),
                "ECMA-376 §18.17.7 SQRTPI",
            ),
            (
                "=SQRTPI(2)",
                Near(2.506_628_275, 1e-9),
                "ECMA-376 §18.17.7 SQRTPI",
            ),
            ("=SQRTPI(-1)", E("#NUM!"), "ECMA-376 §18.17.7 SQRTPI"),
            ("=SIGN(10)", N(1.0), "ECMA-376 §18.17.7 SIGN"),
            ("=SIGN(4-4)", N(0.0), "ECMA-376 §18.17.7 SIGN"),
            ("=SIGN(-0.00001)", N(-1.0), "ECMA-376 §18.17.7 SIGN"),
            ("=EVEN(1.5)", N(2.0), "ECMA-376 §18.17.7 EVEN"),
            ("=EVEN(3)", N(4.0), "ECMA-376 §18.17.7 EVEN"),
            ("=EVEN(2)", N(2.0), "ECMA-376 §18.17.7 EVEN"),
            ("=EVEN(-1)", N(-2.0), "ECMA-376 §18.17.7 EVEN"),
            ("=ODD(1.5)", N(3.0), "ECMA-376 §18.17.7 ODD"),
            ("=ODD(3)", N(3.0), "ECMA-376 §18.17.7 ODD"),
            ("=ODD(2)", N(3.0), "ECMA-376 §18.17.7 ODD"),
            ("=ODD(-1)", N(-1.0), "ECMA-376 §18.17.7 ODD"),
            ("=ODD(-2)", N(-3.0), "ECMA-376 §18.17.7 ODD"),
            ("=QUOTIENT(5,2)", N(2.0), "ECMA-376 §18.17.7 QUOTIENT"),
            ("=QUOTIENT(4.5,3.1)", N(1.0), "ECMA-376 §18.17.7 QUOTIENT"),
            ("=QUOTIENT(-10,3)", N(-3.0), "ECMA-376 §18.17.7 QUOTIENT"),
            ("=QUOTIENT(1,0)", E("#DIV/0!"), "ECMA-376 §18.17.7 QUOTIENT"),
        ],
    );
}

/// Exponential and logarithms (§18.17.7 EXP, LN, LOG, LOG10).
#[test]
fn exp_log() {
    check(
        &[],
        &[
            (
                "=EXP(1)",
                Near(2.718_281_828, 1e-9),
                "ECMA-376 §18.17.7 EXP",
            ),
            (
                "=EXP(2)",
                Near(7.389_056_099, 1e-9),
                "ECMA-376 §18.17.7 EXP",
            ),
            ("=LN(86)", Near(4.454_347_296, 1e-9), "ECMA-376 §18.17.7 LN"),
            ("=LN(EXP(3))", N(3.0), "ECMA-376 §18.17.7 LN"),
            ("=LN(0)", E("#NUM!"), "ECMA-376 §18.17.7 LN"),
            ("=LOG(10)", N(1.0), "ECMA-376 §18.17.7 LOG"),
            ("=LOG(8,2)", N(3.0), "ECMA-376 §18.17.7 LOG"),
            (
                "=LOG(86,2.7182818)",
                Near(4.454_347_4, 1e-6),
                "ECMA-376 §18.17.7 LOG",
            ),
            ("=LOG(10,1)", E("#DIV/0!"), "ECMA-376 §18.17.7 LOG"),
            ("=LOG(-1)", E("#NUM!"), "ECMA-376 §18.17.7 LOG"),
            (
                "=LOG10(86)",
                Near(1.934_498_451, 1e-9),
                "ECMA-376 §18.17.7 LOG10",
            ),
            ("=LOG10(10)", N(1.0), "ECMA-376 §18.17.7 LOG10"),
            ("=LOG10(1E5)", N(5.0), "ECMA-376 §18.17.7 LOG10"),
            ("=LOG10(-1)", E("#NUM!"), "ECMA-376 §18.17.7 LOG10"),
        ],
    );
}

/// Trigonometric and hyperbolic functions, mathematical definitions
/// (§18.17.7 SIN … ACOTH; OpenFormula trigonometric functions).
#[test]
fn trigonometry() {
    check(
        &[],
        &[
            (
                "=PI()",
                Near(std::f64::consts::PI, 1e-15),
                "ECMA-376 §18.17.7 PI",
            ),
            ("=SIN(PI()/2)", N(1.0), "ECMA-376 §18.17.7 SIN"),
            (
                "=SIN(30*PI()/180)",
                Near(0.5, 1e-12),
                "ECMA-376 §18.17.7 SIN",
            ),
            ("=SIN(PI())", Near(0.0, 1e-15), "ECMA-376 §18.17.7 SIN"),
            (
                "=COS(1.047)",
                Near(0.500_171, 1e-6),
                "ECMA-376 §18.17.7 COS",
            ),
            (
                "=COS(60*PI()/180)",
                Near(0.5, 1e-12),
                "ECMA-376 §18.17.7 COS",
            ),
            (
                "=TAN(0.785)",
                Near(0.999_204, 1e-6),
                "ECMA-376 §18.17.7 TAN",
            ),
            (
                "=TAN(45*PI()/180)",
                Near(1.0, 1e-12),
                "ECMA-376 §18.17.7 TAN",
            ),
            (
                "=ASIN(-0.5)",
                Near(-0.523_598_776, 1e-9),
                "ECMA-376 §18.17.7 ASIN",
            ),
            ("=ASIN(2)", E("#NUM!"), "ECMA-376 §18.17.7 ASIN"),
            (
                "=ACOS(-0.5)",
                Near(2.094_395_102, 1e-9),
                "ECMA-376 §18.17.7 ACOS",
            ),
            ("=ACOS(1.5)", E("#NUM!"), "ECMA-376 §18.17.7 ACOS"),
            (
                "=ATAN(1)",
                Near(0.785_398_163, 1e-9),
                "ECMA-376 §18.17.7 ATAN",
            ),
            (
                "=ATAN2(1,1)",
                Near(0.785_398_163, 1e-9),
                "ECMA-376 §18.17.7 ATAN2",
            ),
            (
                "=ATAN2(-1,-1)",
                Near(-2.356_194_49, 1e-8),
                "ECMA-376 §18.17.7 ATAN2",
            ),
            ("=ATAN2(1,0)", N(0.0), "ECMA-376 §18.17.7 ATAN2"),
            ("=ATAN2(0,0)", E("#DIV/0!"), "ECMA-376 §18.17.7 ATAN2"),
            (
                "=SINH(1)",
                Near(1.175_201_194, 1e-9),
                "ECMA-376 §18.17.7 SINH",
            ),
            (
                "=COSH(4)",
                Near(27.308_232_84, 1e-8),
                "ECMA-376 §18.17.7 COSH",
            ),
            (
                "=TANH(-2)",
                Near(-0.964_027_58, 1e-8),
                "ECMA-376 §18.17.7 TANH",
            ),
            (
                "=TANH(0.5)",
                Near(0.462_117_157, 1e-9),
                "ECMA-376 §18.17.7 TANH",
            ),
            (
                "=ASINH(-2.5)",
                Near(-1.647_231_146, 1e-9),
                "ECMA-376 §18.17.7 ASINH",
            ),
            (
                "=ASINH(10)",
                Near(2.998_222_95, 1e-8),
                "ECMA-376 §18.17.7 ASINH",
            ),
            ("=ACOSH(1)", N(0.0), "ECMA-376 §18.17.7 ACOSH"),
            (
                "=ACOSH(10)",
                Near(2.993_222_846, 1e-9),
                "ECMA-376 §18.17.7 ACOSH",
            ),
            ("=ACOSH(0.5)", E("#NUM!"), "ECMA-376 §18.17.7 ACOSH"),
            (
                "=ATANH(-0.1)",
                Near(-0.100_335_348, 1e-9),
                "ECMA-376 §18.17.7 ATANH",
            ),
            ("=ATANH(1)", E("#NUM!"), "ECMA-376 §18.17.7 ATANH"),
            ("=COT(30)", Near(-0.156_119_952, 1e-9), "OpenFormula COT"),
            ("=COT(0)", E("#DIV/0!"), "OpenFormula COT"),
            ("=COTH(2)", Near(1.037_314_721, 1e-9), "OpenFormula COTH"),
            ("=CSC(15)", Near(1.537_780_562, 1e-9), "OpenFormula CSC"),
            ("=CSC(0)", E("#DIV/0!"), "OpenFormula CSC"),
            ("=CSCH(1.5)", Near(0.469_642_441, 1e-9), "OpenFormula CSCH"),
            ("=SEC(45)", Near(1.903_594_407, 1e-9), "OpenFormula SEC"),
            ("=SECH(1)", Near(0.648_054_274, 1e-9), "OpenFormula SECH"),
            ("=ACOT(2)", Near(0.463_647_609, 1e-9), "OpenFormula ACOT"),
            ("=ACOTH(6)", Near(0.168_236_118, 1e-9), "OpenFormula ACOTH"),
            ("=ACOTH(0.5)", E("#NUM!"), "OpenFormula ACOTH"),
            ("=DEGREES(PI())", N(180.0), "ECMA-376 §18.17.7 DEGREES"),
            (
                "=RADIANS(270)",
                Near(4.712_388_98, 1e-8),
                "ECMA-376 §18.17.7 RADIANS",
            ),
        ],
    );
}

/// Combinatorics and number theory (§18.17.7 FACT, FACTDOUBLE, GCD, LCM, COMBIN,
/// PERMUT, MULTINOMIAL; COMBINA, PERMUTATIONA from OpenFormula).
#[test]
fn combinatorics() {
    check(
        &[],
        &[
            ("=FACT(5)", N(120.0), "ECMA-376 §18.17.7 FACT"),
            ("=FACT(1.9)", N(1.0), "ECMA-376 §18.17.7 FACT"),
            ("=FACT(0)", N(1.0), "ECMA-376 §18.17.7 FACT"),
            ("=FACT(-1)", E("#NUM!"), "ECMA-376 §18.17.7 FACT"),
            ("=FACTDOUBLE(6)", N(48.0), "ECMA-376 §18.17.7 FACTDOUBLE"),
            ("=FACTDOUBLE(7)", N(105.0), "ECMA-376 §18.17.7 FACTDOUBLE"),
            ("=GCD(5,2)", N(1.0), "ECMA-376 §18.17.7 GCD"),
            ("=GCD(24,36)", N(12.0), "ECMA-376 §18.17.7 GCD"),
            ("=GCD(5,0)", N(5.0), "ECMA-376 §18.17.7 GCD"),
            ("=GCD(-1,2)", E("#NUM!"), "ECMA-376 §18.17.7 GCD"),
            ("=LCM(5,2)", N(10.0), "ECMA-376 §18.17.7 LCM"),
            ("=LCM(24,36)", N(72.0), "ECMA-376 §18.17.7 LCM"),
            ("=LCM(-1,2)", E("#NUM!"), "ECMA-376 §18.17.7 LCM"),
            ("=COMBIN(8,2)", N(28.0), "ECMA-376 §18.17.7 COMBIN"),
            ("=COMBIN(2,3)", E("#NUM!"), "ECMA-376 §18.17.7 COMBIN"),
            ("=COMBINA(4,3)", N(20.0), "OpenFormula COMBINA"),
            ("=COMBINA(10,3)", N(220.0), "OpenFormula COMBINA"),
            ("=PERMUT(100,3)", N(970_200.0), "ECMA-376 §18.17.7 PERMUT"),
            ("=PERMUT(3,2)", N(6.0), "ECMA-376 §18.17.7 PERMUT"),
            ("=PERMUT(2,3)", E("#NUM!"), "ECMA-376 §18.17.7 PERMUT"),
            ("=PERMUTATIONA(3,2)", N(9.0), "OpenFormula PERMUTATIONA"),
            ("=PERMUTATIONA(2,2)", N(4.0), "OpenFormula PERMUTATIONA"),
            (
                "=MULTINOMIAL(2,3,4)",
                N(1260.0),
                "ECMA-376 §18.17.7 MULTINOMIAL",
            ),
            (
                "=SUMX2MY2({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})",
                N(-55.0),
                "ECMA-376 §18.17.7 SUMX2MY2",
            ),
            (
                "=SUMX2PY2({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})",
                N(521.0),
                "ECMA-376 §18.17.7 SUMX2PY2",
            ),
            (
                "=SUMXMY2({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})",
                N(79.0),
                "ECMA-376 §18.17.7 SUMXMY2",
            ),
            (
                "=SUMXMY2({1,2},{1,2,3})",
                E("#N/A"),
                "ECMA-376 §18.17.7 SUMXMY2",
            ),
        ],
    );
}

/// Random numbers and SEQUENCE: ranges of RAND/RANDBETWEEN are checked, not
/// values (§18.17.7 RAND; SEQUENCE/RANDARRAY dynamic-array definitions).
#[test]
fn random_sequence() {
    check(
        &[],
        &[
            (
                "=AND(RAND()>=0,RAND()<1)",
                B(true),
                "ECMA-376 §18.17.7 RAND",
            ),
            ("=RANDBETWEEN(1,1)", N(1.0), "ECMA-376 §18.17.7 RANDBETWEEN"),
            (
                "=AND(RANDBETWEEN(1,6)>=1,RANDBETWEEN(1,6)<=6)",
                B(true),
                "ECMA-376 §18.17.7 RANDBETWEEN",
            ),
            (
                "=RANDBETWEEN(5,1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 RANDBETWEEN",
            ),
            ("=ROWS(RANDARRAY(3,2))", N(3.0), "MS-XLSX _xlfn.RANDARRAY"),
            (
                "=COLUMNS(RANDARRAY(3,2))",
                N(2.0),
                "MS-XLSX _xlfn.RANDARRAY",
            ),
            ("=SEQUENCE(3)", A("{1;2;3}"), "MS-XLSX _xlfn.SEQUENCE"),
            (
                "=SEQUENCE(2,3)",
                A("{1,2,3;4,5,6}"),
                "MS-XLSX _xlfn.SEQUENCE",
            ),
            (
                "=SEQUENCE(2,2,10,-1)",
                A("{10,9;8,7}"),
                "MS-XLSX _xlfn.SEQUENCE",
            ),
            (
                "=SEQUENCE(1,3,0,0.5)",
                A("{0,0.5,1}"),
                "MS-XLSX _xlfn.SEQUENCE",
            ),
            ("=SUM(SEQUENCE(100))", N(5050.0), "MS-XLSX _xlfn.SEQUENCE"),
        ],
    );
}

/// SUBTOTAL / AGGREGATE function numbers 1-11 / 1-19 and the error-ignoring
/// options of AGGREGATE (§18.17.7 SUBTOTAL; AGGREGATE published definition).
#[test]
fn subtotal_aggregate() {
    let data = &[
        ("A1", "1"),
        ("A2", "2"),
        ("A3", "3"),
        ("A4", "4"),
        ("A5", "5"),
        ("G1", "3"),
        ("G2", "#DIV/0!"),
        ("G3", "5"),
        ("G4", "1"),
    ];
    check(
        data,
        &[
            ("=SUBTOTAL(9,A1:A5)", N(15.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(1,A1:A5)", N(3.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(2,A1:A5)", N(5.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(3,A1:A5)", N(5.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(4,A1:A5)", N(5.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(5,A1:A5)", N(1.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(6,A1:A3)", N(6.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            (
                "=SUBTOTAL(7,A1:A5)",
                Near(1.581_138_83, 1e-8),
                "ECMA-376 §18.17.7 SUBTOTAL",
            ),
            (
                "=SUBTOTAL(8,A1:A5)",
                Near(1.414_213_562, 1e-9),
                "ECMA-376 §18.17.7 SUBTOTAL",
            ),
            ("=SUBTOTAL(10,A1:A5)", N(2.5), "ECMA-376 §18.17.7 SUBTOTAL"),
            ("=SUBTOTAL(11,A1:A5)", N(2.0), "ECMA-376 §18.17.7 SUBTOTAL"),
            (
                "=SUBTOTAL(109,A1:A5)",
                N(15.0),
                "ECMA-376 §18.17.7 SUBTOTAL",
            ),
            (
                "=SUBTOTAL(12,A1:A5)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SUBTOTAL",
            ),
            (
                "=SUBTOTAL(9,G1:G4)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 SUBTOTAL",
            ),
            ("=AGGREGATE(9,6,G1:G4)", N(9.0), "MS-XLSX _xlfn.AGGREGATE"),
            ("=AGGREGATE(4,6,G1:G4)", N(5.0), "MS-XLSX _xlfn.AGGREGATE"),
            (
                "=AGGREGATE(14,6,A1:A5,2)",
                N(4.0),
                "MS-XLSX _xlfn.AGGREGATE",
            ),
            (
                "=AGGREGATE(15,6,A1:A5,2)",
                N(2.0),
                "MS-XLSX _xlfn.AGGREGATE",
            ),
            ("=AGGREGATE(12,6,A1:A5)", N(3.0), "MS-XLSX _xlfn.AGGREGATE"),
            (
                "=AGGREGATE(16,6,A1:A5,0.5)",
                N(3.0),
                "MS-XLSX _xlfn.AGGREGATE",
            ),
            (
                "=AGGREGATE(9,0,G1:G4)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.AGGREGATE",
            ),
        ],
    );
}

/// SUBTOTAL 1xx ignores hidden rows; nested SUBTOTALs are not double counted
/// (§18.17.7 SUBTOTAL remarks).
#[test]
fn subtotal_hidden_rows_and_nesting() {
    let mut wb = workbook(
        &[
            ("A1", "1"),
            ("A2", "2"),
            ("A3", "3"),
            ("A4", "=SUBTOTAL(9,A1:A3)"),
            ("B1", "=SUBTOTAL(109,A1:A3)"),
            ("B2", "=SUBTOTAL(9,A1:A4)"),
            ("B3", "=SUM(A1:A4)"),
        ],
        Opts::default(),
    );
    wb.set_row_hidden(0, 1, true);
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "B1"), bo_formula::Value::Number(4.0));
    assert_eq!(wb.value_a1(0, "B2"), bo_formula::Value::Number(6.0));
    assert_eq!(wb.value_a1(0, "B3"), bo_formula::Value::Number(12.0));
}

/// Additional value, edge and error rows for the "Hàm đợt 1" Math & Trig list
/// (gap #37): every function has at least one error or boundary case.
#[test]
fn math_edge_cases() {
    check(
        DATA,
        &[
            ("=SUMIFS(B1:B5,A1:A5,\">=3\")", N(120.0), "hand: 30+40+50"),
            (
                "=SUMIFS(B1:B4,F1:F4,\"*an*\")",
                N(20.0),
                "hand: only banana (B2) matches *an*",
            ),
            (
                "=SUMIFS(B1:B5,A1:A5,\">9\")",
                N(0.0),
                "ECMA-376 §18.17.7 SUMIFS; hand: no match → 0",
            ),
            (
                "=PRODUCT(Z1:Z3)",
                N(0.0),
                "ECMA-376 §18.17.7 PRODUCT; hand: no numbers → 0",
            ),
            (
                "=PRODUCT(D1:D3)",
                N(1.0),
                "ECMA-376 §18.17.7 PRODUCT; hand: text/TRUE in refs ignored, 1",
            ),
            (
                "=PRODUCT(E1:E2)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 PRODUCT; hand: error propagates",
            ),
            ("=INT(0)", N(0.0), "hand: floor(0) = 0"),
            ("=INT(-0.5)", N(-1.0), "hand: floor(-0.5) = -1"),
            (
                "=INT(\"x\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 INT; hand: non-numeric text",
            ),
            (
                "=ABS(\"-3\")",
                N(3.0),
                "hand: numeric text coerced, |-3| = 3",
            ),
            ("=ABS(0)", N(0.0), "hand: |0| = 0"),
            (
                "=ABS(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 ABS; hand: non-numeric text",
            ),
            ("=SQRT(0)", N(0.0), "hand: sqrt(0) = 0"),
            ("=SQRT(2.25)", N(1.5), "hand: 1.5^2 = 2.25"),
            ("=EXP(0)", N(1.0), "hand: e^0 = 1"),
            ("=EXP(-1)", Near(0.367_879_441_171, 1e-12), "hand: 1/e"),
            (
                "=EXP(1000)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 EXP; hand: overflows double range",
            ),
            ("=LN(1)", N(0.0), "hand: ln 1 = 0"),
            (
                "=LN(-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 LN; hand: domain x > 0",
            ),
            ("=LOG(100,10)", N(2.0), "hand: 10^2 = 100"),
            ("=LOG(1)", N(0.0), "hand: log10 1 = 0"),
            (
                "=LOG10(0)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 LOG10; hand: domain x > 0",
            ),
            (
                "=MOD(-7,3)",
                N(2.0),
                "hand: -7 - 3*floor(-7/3) = -7 + 9 = 2",
            ),
            ("=MOD(7.5,2)", N(1.5), "hand: 7.5 - 2*3 = 1.5"),
            ("=POWER(2,-2)", N(0.25), "hand: 1/2^2"),
            (
                "=POWER(\"a\",2)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 POWER; hand: non-numeric text",
            ),
            (
                "=TRUNC(-4.567,2)",
                N(-4.56),
                "hand: drop digits after 2nd decimal",
            ),
            ("=TRUNC(1234,-2)", N(1200.0), "hand: truncate to hundreds"),
            ("=MROUND(0,5)", N(0.0), "hand: 0 is a multiple of 5"),
            (
                "=MROUND(7,0)",
                N(0.0),
                "ECMA-376 §18.17.7 MROUND; hand: multiple 0 → 0",
            ),
            ("=ROUNDUP(0,2)", N(0.0), "hand: 0 stays 0"),
            ("=ROUNDDOWN(-0.001,2)", N(0.0), "hand: toward zero → -0 = 0"),
            ("=CEILING(0,1)", N(0.0), "hand: 0 is a multiple of 1"),
            (
                "=FLOOR(-3.7,-1)",
                N(-3.0),
                "hand: toward zero by |significance| 1",
            ),
            (
                "=CEILING.MATH(0)",
                N(0.0),
                "MS-XLSX _xlfn.CEILING.MATH; hand: 0",
            ),
            (
                "=FLOOR.MATH(-0.5)",
                N(-1.0),
                "MS-XLSX _xlfn.FLOOR.MATH; hand: round down (away from 0 for negatives)",
            ),
            (
                "=FLOOR.MATH(\"a\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.FLOOR.MATH; hand: non-numeric text",
            ),
            (
                "=RAND()<1",
                B(true),
                "ECMA-376 §18.17.7 RAND; hand: range [0,1)",
            ),
            (
                "=RANDBETWEEN(-3,-3)",
                N(-3.0),
                "ECMA-376 §18.17.7 RANDBETWEEN; hand: bottom = top",
            ),
            (
                "=RANDBETWEEN(\"a\",2)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 RANDBETWEEN; hand: non-numeric text",
            ),
            (
                "=SEQUENCE(0)",
                E("#CALC!"),
                "MS-XLSX _xlfn.SEQUENCE; hand: empty result",
            ),
            (
                "=SEQUENCE(1,1,5)",
                N(5.0),
                "MS-XLSX _xlfn.SEQUENCE; hand: single cell = start",
            ),
            (
                "=SUBTOTAL(9,Z1:Z3)",
                N(0.0),
                "ECMA-376 §18.17.7 SUBTOTAL; hand: empty sum",
            ),
            (
                "=SUBTOTAL(0,A1:A5)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SUBTOTAL; hand: function_num out of 1-11/101-111",
            ),
            (
                "=AGGREGATE(1,6,A1:A5,E1:E2)",
                Near(22.0 / 6.0, 1e-12),
                "MS-XLSX _xlfn.AGGREGATE; hand: errors ignored, (1+2+3+4+5+7)/6",
            ),
            (
                "=AGGREGATE(20,6,A1:A5)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.AGGREGATE; hand: function_num out of 1-19",
            ),
            ("=PI()*2", Near(6.283_185_307_179_586, 1e-15), "hand: 2π"),
            ("=SIN(0)", N(0.0), "hand: sin 0 = 0"),
            ("=SIN(-PI()/2)", N(-1.0), "hand: sin(-π/2) = -1"),
            (
                "=SIN(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SIN; hand: non-numeric text",
            ),
            ("=COS(0)", N(1.0), "hand: cos 0 = 1"),
            ("=COS(PI())", N(-1.0), "hand: cos π = -1"),
            (
                "=COS(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 COS; hand: non-numeric text",
            ),
            ("=TAN(0)", N(0.0), "hand: tan 0 = 0"),
            ("=TAN(-PI()/4)", Near(-1.0, 1e-12), "hand: tan(-π/4) = -1"),
            (
                "=ASIN(1)",
                Near(std::f64::consts::FRAC_PI_2, 1e-15),
                "hand: asin 1 = π/2",
            ),
            ("=ASIN(0)", N(0.0), "hand: asin 0 = 0"),
            ("=ACOS(1)", N(0.0), "hand: acos 1 = 0"),
            (
                "=ACOS(0)",
                Near(std::f64::consts::FRAC_PI_2, 1e-15),
                "hand: acos 0 = π/2",
            ),
            ("=ATAN(0)", N(0.0), "hand: atan 0 = 0"),
            ("=ATAN(-1)", Near(-0.785_398_163_397, 1e-12), "hand: -π/4"),
            (
                "=ATAN(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 ATAN; hand: non-numeric text",
            ),
            (
                "=ATAN2(-1,0)",
                Near(std::f64::consts::PI, 1e-15),
                "hand: point (-1,0) → π",
            ),
            ("=SINH(0)", N(0.0), "hand: sinh 0 = 0"),
            (
                "=SINH(-1)",
                Near(-1.175_201_194, 1e-9),
                "hand: (e^-1 - e)/2",
            ),
            (
                "=SINH(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SINH; hand: non-numeric text",
            ),
            ("=COSH(0)", N(1.0), "hand: cosh 0 = 1"),
            ("=COSH(-4)", Near(27.308_232_84, 1e-8), "hand: cosh is even"),
            (
                "=COSH(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 COSH; hand: non-numeric text",
            ),
            ("=TANH(0)", N(0.0), "hand: tanh 0 = 0"),
            ("=TANH(50)", Near(1.0, 1e-15), "hand: tanh saturates at 1"),
            ("=ASINH(0)", N(0.0), "hand: asinh 0 = 0"),
            (
                "=ASINH(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 ASINH; hand: non-numeric text",
            ),
            (
                "=ACOSH(1.5)",
                Near(0.962_423_650_119, 1e-12),
                "hand: ln(1.5 + sqrt(1.25))",
            ),
            ("=ATANH(0)", N(0.0), "hand: atanh 0 = 0"),
            (
                "=ATANH(0.5)",
                Near(0.549_306_144_334, 1e-12),
                "hand: ln(3)/2",
            ),
            (
                "=ATANH(-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 ATANH; hand: domain |x| < 1",
            ),
            ("=DEGREES(PI()/2)", N(90.0), "hand: (π/2)·180/π"),
            ("=DEGREES(0)", N(0.0), "hand: 0"),
            (
                "=DEGREES(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 DEGREES; hand: non-numeric text",
            ),
            (
                "=RADIANS(180)",
                Near(std::f64::consts::PI, 1e-15),
                "hand: 180·π/180",
            ),
            (
                "=RADIANS(-90)",
                Near(-std::f64::consts::FRAC_PI_2, 1e-15),
                "hand: -90·π/180",
            ),
            (
                "=RADIANS(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 RADIANS; hand: non-numeric text",
            ),
            ("=COT(PI()/4)", Near(1.0, 1e-12), "hand: 1/tan(π/4)"),
            (
                "=COT(\"a\")",
                E("#VALUE!"),
                "OpenFormula COT; hand: non-numeric text",
            ),
            ("=CSC(PI()/2)", Near(1.0, 1e-12), "hand: 1/sin(π/2)"),
            ("=CSC(-PI()/2)", Near(-1.0, 1e-12), "hand: 1/sin(-π/2)"),
            ("=SEC(0)", N(1.0), "hand: 1/cos 0"),
            ("=SEC(PI())", Near(-1.0, 1e-12), "hand: 1/cos π"),
            (
                "=SEC(\"a\")",
                E("#VALUE!"),
                "OpenFormula SEC; hand: non-numeric text",
            ),
        ],
    );
}
