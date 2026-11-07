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
            ("=SUM(1,2,3)", N(6.0)),
            ("=SUM(A1:A5)", N(15.0)),
            ("=SUM(A1:A5,10)", N(25.0)),
            ("=SUM(\"3\",2,TRUE)", N(6.0)),
            ("=SUM(D1:D3)", N(1.0)),
            ("=SUM(\"abc\")", E("#VALUE!")),
            ("=SUM(E1:E2)", E("#DIV/0!")),
            ("=SUM(A1:A5,#N/A)", E("#N/A")),
            ("=SUM(A1:B2)", N(33.0)),
            ("=SUM((A1:A2,B1:B2))", N(33.0)),
            ("=SUM(Z1:Z10)", N(0.0)),
            ("=PRODUCT(A1:A5)", N(120.0)),
            ("=PRODUCT(2,3,\"4\")", N(24.0)),
            ("=PRODUCT(A1:A3,2)", N(12.0)),
            ("=SUMSQ(3,4)", N(25.0)),
            ("=SUMSQ(A1:A3)", N(14.0)),
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
            ("=SUMPRODUCT(A1:A3,B1:B3)", N(140.0)),
            ("=SUMPRODUCT({1,2;3,4},{5,6;7,8})", N(70.0)),
            ("=SUMPRODUCT({1,\"a\"},{2,3})", N(2.0)),
            ("=SUMPRODUCT(A1:A3,B1:B2)", E("#VALUE!")),
            ("=SUMPRODUCT((A1:A5>2)*1)", N(3.0)),
            ("=SUMPRODUCT(A1:A5)", N(15.0)),
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
            ("=SUMIF(A1:A5,\">2\")", N(12.0)),
            ("=SUMIF(A1:A5,\">2\",B1:B5)", N(120.0)),
            ("=SUMIF(A1:A5,3)", N(3.0)),
            ("=SUMIF(A1:A5,\"=3\")", N(3.0)),
            ("=SUMIF(A1:A5,\"<>3\")", N(12.0)),
            ("=SUMIF(F1:F4,\"apple\",A1:A4)", N(4.0)),
            ("=SUMIF(F1:F4,\"b*\",A1:A4)", N(2.0)),
            ("=SUMIF(F1:F4,\"?????\",A1:A4)", N(4.0)),
            ("=SUMIF(F1:F4,\"<>apple\",A1:A4)", N(6.0)),
            ("=SUMIFS(B1:B5,A1:A5,\">1\",A1:A5,\"<5\")", N(90.0)),
            ("=SUMIFS(B1:B4,F1:F4,\"apple\",A1:A4,\">1\")", N(30.0)),
            ("=SUMIFS(B1:B5,A1:A4,\">1\")", E("#VALUE!")),
        ],
    );
}

/// Rounding family (§18.17.7 ROUND, ROUNDUP, ROUNDDOWN, MROUND, INT, TRUNC; worked examples).
#[test]
fn rounding() {
    check(
        &[],
        &[
            ("=ROUND(2.15,1)", N(2.2)),
            ("=ROUND(2.149,1)", N(2.1)),
            ("=ROUND(-1.475,2)", N(-1.48)),
            ("=ROUND(21.5,-1)", N(20.0)),
            ("=ROUND(626.3,-3)", N(1000.0)),
            ("=ROUND(1.98,-1)", N(0.0)),
            ("=ROUND(-50.55,-2)", N(-100.0)),
            ("=ROUND(2.5,0)", N(3.0)),
            ("=ROUND(-2.5,0)", N(-3.0)),
            ("=ROUNDUP(3.2,0)", N(4.0)),
            ("=ROUNDUP(76.9,0)", N(77.0)),
            ("=ROUNDUP(3.14159,3)", N(3.142)),
            ("=ROUNDUP(-3.14159,1)", N(-3.2)),
            ("=ROUNDUP(31415.92654,-2)", N(31500.0)),
            ("=ROUNDDOWN(3.2,0)", N(3.0)),
            ("=ROUNDDOWN(76.9,0)", N(76.0)),
            ("=ROUNDDOWN(3.14159,3)", N(3.141)),
            ("=ROUNDDOWN(-3.14159,1)", N(-3.1)),
            ("=ROUNDDOWN(31415.92654,-2)", N(31400.0)),
            ("=MROUND(10,3)", N(9.0)),
            ("=MROUND(-10,-3)", N(-9.0)),
            ("=MROUND(1.3,0.2)", N(1.4)),
            ("=MROUND(5,-2)", E("#NUM!")),
            ("=INT(8.9)", N(8.0)),
            ("=INT(-8.9)", N(-9.0)),
            ("=TRUNC(8.9)", N(8.0)),
            ("=TRUNC(-8.9)", N(-8.0)),
            ("=TRUNC(0.45)", N(0.0)),
            ("=TRUNC(3.14159,2)", N(3.14)),
            ("=ROUND(\"x\",1)", E("#VALUE!")),
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
            ("=CEILING(2.5,1)", N(3.0)),
            ("=CEILING(-2.5,-2)", N(-4.0)),
            ("=CEILING(-2.5,2)", N(-2.0)),
            ("=CEILING(1.5,0.1)", N(1.5)),
            ("=CEILING(0.234,0.01)", N(0.24)),
            ("=CEILING(2.5,-2)", E("#NUM!")),
            ("=CEILING.MATH(24.3,5)", N(25.0)),
            ("=CEILING.MATH(6.7)", N(7.0)),
            ("=CEILING.MATH(-8.1,2)", N(-8.0)),
            ("=CEILING.MATH(-5.5,2,-1)", N(-6.0)),
            ("=CEILING.PRECISE(4.3)", N(5.0)),
            ("=CEILING.PRECISE(-4.3)", N(-4.0)),
            ("=CEILING.PRECISE(4.3,2)", N(6.0)),
            ("=CEILING.PRECISE(4.3,-2)", N(6.0)),
            ("=CEILING.PRECISE(-4.3,2)", N(-4.0)),
            ("=CEILING.PRECISE(-4.3,-2)", N(-4.0)),
            ("=ISO.CEILING(4.3)", N(5.0)),
            ("=ISO.CEILING(-4.3,2)", N(-4.0)),
            ("=FLOOR(3.7,2)", N(2.0)),
            ("=FLOOR(-2.5,-2)", N(-2.0)),
            ("=FLOOR(2.5,-2)", E("#NUM!")),
            ("=FLOOR(1.58,0.1)", N(1.5)),
            ("=FLOOR(0.234,0.01)", N(0.23)),
            ("=FLOOR(5,0)", E("#DIV/0!")),
            ("=FLOOR.MATH(24.3,5)", N(20.0)),
            ("=FLOOR.MATH(5.5,2)", N(4.0)),
            ("=FLOOR.MATH(-8.1,2)", N(-10.0)),
            ("=FLOOR.MATH(-5.5,2,-1)", N(-4.0)),
            ("=FLOOR.PRECISE(-3.2,-1)", N(-4.0)),
            ("=FLOOR.PRECISE(3.2,1)", N(3.0)),
            ("=FLOOR.PRECISE(-3.2,1)", N(-4.0)),
            ("=FLOOR.PRECISE(3.2,-1)", N(3.0)),
            ("=FLOOR.PRECISE(3.2)", N(3.0)),
        ],
    );
}

/// Arithmetic helpers (§18.17.7 MOD, ABS, POWER, SQRT, SQRTPI, SIGN, EVEN, ODD, QUOTIENT).
#[test]
fn arithmetic() {
    check(
        &[],
        &[
            ("=MOD(3,2)", N(1.0)),
            ("=MOD(-3,2)", N(1.0)),
            ("=MOD(3,-2)", N(-1.0)),
            ("=MOD(-3,-2)", N(-1.0)),
            ("=MOD(3,0)", E("#DIV/0!")),
            ("=ABS(-4)", N(4.0)),
            ("=ABS(2)", N(2.0)),
            ("=POWER(5,2)", N(25.0)),
            ("=POWER(4,5/4)", Near(5.656_854_249, 1e-9)),
            ("=POWER(0,0)", E("#NUM!")),
            ("=POWER(0,-1)", E("#DIV/0!")),
            ("=POWER(-8,1/3)", E("#NUM!")),
            ("=2^10", N(1024.0)),
            ("=SQRT(16)", N(4.0)),
            ("=SQRT(-16)", E("#NUM!")),
            ("=SQRT(ABS(-16))", N(4.0)),
            ("=SQRTPI(1)", Near(1.772_453_851, 1e-9)),
            ("=SQRTPI(2)", Near(2.506_628_275, 1e-9)),
            ("=SQRTPI(-1)", E("#NUM!")),
            ("=SIGN(10)", N(1.0)),
            ("=SIGN(4-4)", N(0.0)),
            ("=SIGN(-0.00001)", N(-1.0)),
            ("=EVEN(1.5)", N(2.0)),
            ("=EVEN(3)", N(4.0)),
            ("=EVEN(2)", N(2.0)),
            ("=EVEN(-1)", N(-2.0)),
            ("=ODD(1.5)", N(3.0)),
            ("=ODD(3)", N(3.0)),
            ("=ODD(2)", N(3.0)),
            ("=ODD(-1)", N(-1.0)),
            ("=ODD(-2)", N(-3.0)),
            ("=QUOTIENT(5,2)", N(2.0)),
            ("=QUOTIENT(4.5,3.1)", N(1.0)),
            ("=QUOTIENT(-10,3)", N(-3.0)),
            ("=QUOTIENT(1,0)", E("#DIV/0!")),
        ],
    );
}

/// Exponential and logarithms (§18.17.7 EXP, LN, LOG, LOG10).
#[test]
fn exp_log() {
    check(
        &[],
        &[
            ("=EXP(1)", Near(2.718_281_828, 1e-9)),
            ("=EXP(2)", Near(7.389_056_099, 1e-9)),
            ("=LN(86)", Near(4.454_347_296, 1e-9)),
            ("=LN(EXP(3))", N(3.0)),
            ("=LN(0)", E("#NUM!")),
            ("=LOG(10)", N(1.0)),
            ("=LOG(8,2)", N(3.0)),
            ("=LOG(86,2.7182818)", Near(4.454_347_4, 1e-6)),
            ("=LOG(10,1)", E("#DIV/0!")),
            ("=LOG(-1)", E("#NUM!")),
            ("=LOG10(86)", Near(1.934_498_451, 1e-9)),
            ("=LOG10(10)", N(1.0)),
            ("=LOG10(1E5)", N(5.0)),
            ("=LOG10(-1)", E("#NUM!")),
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
            ("=PI()", Near(std::f64::consts::PI, 1e-15)),
            ("=SIN(PI()/2)", N(1.0)),
            ("=SIN(30*PI()/180)", Near(0.5, 1e-12)),
            ("=SIN(PI())", Near(0.0, 1e-15)),
            ("=COS(1.047)", Near(0.500_171, 1e-6)),
            ("=COS(60*PI()/180)", Near(0.5, 1e-12)),
            ("=TAN(0.785)", Near(0.999_204, 1e-6)),
            ("=TAN(45*PI()/180)", Near(1.0, 1e-12)),
            ("=ASIN(-0.5)", Near(-0.523_598_776, 1e-9)),
            ("=ASIN(2)", E("#NUM!")),
            ("=ACOS(-0.5)", Near(2.094_395_102, 1e-9)),
            ("=ACOS(1.5)", E("#NUM!")),
            ("=ATAN(1)", Near(0.785_398_163, 1e-9)),
            ("=ATAN2(1,1)", Near(0.785_398_163, 1e-9)),
            ("=ATAN2(-1,-1)", Near(-2.356_194_49, 1e-8)),
            ("=ATAN2(1,0)", N(0.0)),
            ("=ATAN2(0,0)", E("#DIV/0!")),
            ("=SINH(1)", Near(1.175_201_194, 1e-9)),
            ("=COSH(4)", Near(27.308_232_84, 1e-8)),
            ("=TANH(-2)", Near(-0.964_027_58, 1e-8)),
            ("=TANH(0.5)", Near(0.462_117_157, 1e-9)),
            ("=ASINH(-2.5)", Near(-1.647_231_146, 1e-9)),
            ("=ASINH(10)", Near(2.998_222_95, 1e-8)),
            ("=ACOSH(1)", N(0.0)),
            ("=ACOSH(10)", Near(2.993_222_846, 1e-9)),
            ("=ACOSH(0.5)", E("#NUM!")),
            ("=ATANH(-0.1)", Near(-0.100_335_348, 1e-9)),
            ("=ATANH(1)", E("#NUM!")),
            ("=COT(30)", Near(-0.156_119_952, 1e-9)),
            ("=COT(0)", E("#DIV/0!")),
            ("=COTH(2)", Near(1.037_314_721, 1e-9)),
            ("=CSC(15)", Near(1.537_780_562, 1e-9)),
            ("=CSC(0)", E("#DIV/0!")),
            ("=CSCH(1.5)", Near(0.469_642_441, 1e-9)),
            ("=SEC(45)", Near(1.903_594_407, 1e-9)),
            ("=SECH(1)", Near(0.648_054_274, 1e-9)),
            ("=ACOT(2)", Near(0.463_647_609, 1e-9)),
            ("=ACOTH(6)", Near(0.168_236_118, 1e-9)),
            ("=ACOTH(0.5)", E("#NUM!")),
            ("=DEGREES(PI())", N(180.0)),
            ("=RADIANS(270)", Near(4.712_388_98, 1e-8)),
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
            ("=FACT(5)", N(120.0)),
            ("=FACT(1.9)", N(1.0)),
            ("=FACT(0)", N(1.0)),
            ("=FACT(-1)", E("#NUM!")),
            ("=FACTDOUBLE(6)", N(48.0)),
            ("=FACTDOUBLE(7)", N(105.0)),
            ("=GCD(5,2)", N(1.0)),
            ("=GCD(24,36)", N(12.0)),
            ("=GCD(5,0)", N(5.0)),
            ("=GCD(-1,2)", E("#NUM!")),
            ("=LCM(5,2)", N(10.0)),
            ("=LCM(24,36)", N(72.0)),
            ("=LCM(-1,2)", E("#NUM!")),
            ("=COMBIN(8,2)", N(28.0)),
            ("=COMBIN(2,3)", E("#NUM!")),
            ("=COMBINA(4,3)", N(20.0)),
            ("=COMBINA(10,3)", N(220.0)),
            ("=PERMUT(100,3)", N(970_200.0)),
            ("=PERMUT(3,2)", N(6.0)),
            ("=PERMUT(2,3)", E("#NUM!")),
            ("=PERMUTATIONA(3,2)", N(9.0)),
            ("=PERMUTATIONA(2,2)", N(4.0)),
            ("=MULTINOMIAL(2,3,4)", N(1260.0)),
            ("=SUMX2MY2({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})", N(-55.0)),
            ("=SUMX2PY2({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})", N(521.0)),
            ("=SUMXMY2({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})", N(79.0)),
            ("=SUMXMY2({1,2},{1,2,3})", E("#N/A")),
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
            ("=AND(RAND()>=0,RAND()<1)", B(true)),
            ("=RANDBETWEEN(1,1)", N(1.0)),
            ("=AND(RANDBETWEEN(1,6)>=1,RANDBETWEEN(1,6)<=6)", B(true)),
            ("=RANDBETWEEN(5,1)", E("#NUM!")),
            ("=ROWS(RANDARRAY(3,2))", N(3.0)),
            ("=COLUMNS(RANDARRAY(3,2))", N(2.0)),
            ("=SEQUENCE(3)", A("{1;2;3}")),
            ("=SEQUENCE(2,3)", A("{1,2,3;4,5,6}")),
            ("=SEQUENCE(2,2,10,-1)", A("{10,9;8,7}")),
            ("=SEQUENCE(1,3,0,0.5)", A("{0,0.5,1}")),
            ("=SUM(SEQUENCE(100))", N(5050.0)),
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
            ("=SUBTOTAL(9,A1:A5)", N(15.0)),
            ("=SUBTOTAL(1,A1:A5)", N(3.0)),
            ("=SUBTOTAL(2,A1:A5)", N(5.0)),
            ("=SUBTOTAL(3,A1:A5)", N(5.0)),
            ("=SUBTOTAL(4,A1:A5)", N(5.0)),
            ("=SUBTOTAL(5,A1:A5)", N(1.0)),
            ("=SUBTOTAL(6,A1:A3)", N(6.0)),
            ("=SUBTOTAL(7,A1:A5)", Near(1.581_138_83, 1e-8)),
            ("=SUBTOTAL(8,A1:A5)", Near(1.414_213_562, 1e-9)),
            ("=SUBTOTAL(10,A1:A5)", N(2.5)),
            ("=SUBTOTAL(11,A1:A5)", N(2.0)),
            ("=SUBTOTAL(109,A1:A5)", N(15.0)),
            ("=SUBTOTAL(12,A1:A5)", E("#VALUE!")),
            ("=SUBTOTAL(9,G1:G4)", E("#DIV/0!")),
            ("=AGGREGATE(9,6,G1:G4)", N(9.0)),
            ("=AGGREGATE(4,6,G1:G4)", N(5.0)),
            ("=AGGREGATE(14,6,A1:A5,2)", N(4.0)),
            ("=AGGREGATE(15,6,A1:A5,2)", N(2.0)),
            ("=AGGREGATE(12,6,A1:A5)", N(3.0)),
            ("=AGGREGATE(16,6,A1:A5,0.5)", N(3.0)),
            ("=AGGREGATE(9,0,G1:G4)", E("#DIV/0!")),
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
