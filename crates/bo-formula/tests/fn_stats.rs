//! Statistical functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (AVERAGE, AVERAGEA, AVERAGEIF(S), COUNT,
//! COUNTA, COUNTBLANK, COUNTIF(S), MAX, MAXA, MIN, MINA, MEDIAN, MODE, LARGE,
//! SMALL, RANK, STDEV*, VAR*, PERCENTILE, QUARTILE, CORREL, PEARSON, RSQ, SLOPE,
//! INTERCEPT, FORECAST, COVAR, NORMDIST, NORMINV, NORMSDIST, NORMSINV,
//! STANDARDIZE, GEOMEAN, HARMEAN, AVEDEV, DEVSQ) and OpenFormula (ODF 1.3 Part 4)
//! Statistical functions; the `.S` / `.P` / `.INC` / `.EXC` / `.SNGL` / `.EQ` /
//! `.AVG` variants follow their published definitions ([MS-XLSX] future
//! functions). Normal-distribution values are the exact mathematical values
//! (error function) at the shown precision.
//!
//! Data: A1:A8 = {2,4,4,4,5,5,7,9} (mean 5, population σ = 2),
//! B1:B5 = {1..5}, C1:C5 = {2,4,5,4,5}, D1:D4 = {"x", TRUE, 3, blank}.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    ("A1", "2"),
    ("A2", "4"),
    ("A3", "4"),
    ("A4", "4"),
    ("A5", "5"),
    ("A6", "5"),
    ("A7", "7"),
    ("A8", "9"),
    ("B1", "1"),
    ("B2", "2"),
    ("B3", "3"),
    ("B4", "4"),
    ("B5", "5"),
    ("C1", "2"),
    ("C2", "4"),
    ("C3", "5"),
    ("C4", "4"),
    ("C5", "5"),
    ("D1", "x"),
    ("D2", "TRUE"),
    ("D3", "3"),
    ("E1", "=\"\""),
    ("F1", "1"),
    ("F2", "#N/A"),
];

#[test]
fn averages() {
    check(
        DATA,
        &[
            ("=AVERAGE(A1:A8)", N(5.0)),
            ("=AVERAGE(1,2,\"3\")", N(2.0)),
            ("=AVERAGE(D1:D4)", N(3.0)),
            ("=AVERAGE(Z1:Z3)", E("#DIV/0!")),
            ("=AVERAGE(F1:F2)", E("#N/A")),
            ("=AVERAGEA(D1:D3)", Near(4.0 / 3.0, 1e-12)),
            ("=AVERAGEA(D1:D4)", Near(4.0 / 3.0, 1e-12)),
            ("=AVERAGEIF(A1:A8,\">4\")", N(6.5)),
            ("=AVERAGEIF(A1:A8,\">100\")", E("#DIV/0!")),
            ("=AVERAGEIF(B1:B5,\">2\",C1:C5)", Near(14.0 / 3.0, 1e-12)),
            (
                "=AVERAGEIFS(C1:C5,B1:B5,\">1\",B1:B5,\"<5\")",
                Near(13.0 / 3.0, 1e-12),
            ),
            (
                "=GEOMEAN(4,5,8,7,11,4,3)",
                Near(5.476_986_969_656_962, 1e-12),
            ),
            ("=GEOMEAN(1,-1)", E("#NUM!")),
            (
                "=HARMEAN(4,5,8,7,11,4,3)",
                Near(5.028_375_962_061_728, 1e-12),
            ),
            ("=AVEDEV(4,5,6,7,5,4,3)", Near(1.020_408_163_265_306, 1e-12)),
            ("=DEVSQ(4,5,8,7,11,4,3)", N(48.0)),
        ],
    );
}

#[test]
fn counting() {
    check(
        DATA,
        &[
            ("=COUNT(A1:A8)", N(8.0)),
            ("=COUNT(D1:D4)", N(1.0)),
            ("=COUNT(1,\"2\",\"x\",TRUE)", N(3.0)),
            ("=COUNTA(D1:D4)", N(3.0)),
            ("=COUNTA(D1:D4,E1)", N(4.0)),
            ("=COUNTBLANK(D1:D4)", N(1.0)),
            ("=COUNTBLANK(E1:E2)", N(2.0)),
            ("=COUNTIF(A1:A8,4)", N(3.0)),
            ("=COUNTIF(A1:A8,\"4\")", N(3.0)),
            ("=COUNTIF(A1:A8,\">=5\")", N(4.0)),
            ("=COUNTIF(D1:D4,\"x\")", N(1.0)),
            ("=COUNTIF(D1:D4,\"X\")", N(1.0)),
            ("=COUNTIF(D1:D4,TRUE)", N(1.0)),
            ("=COUNTIF(D1:D4,\"<>\")", N(3.0)),
            ("=COUNTIFS(B1:B5,\">2\",C1:C5,\"5\")", N(2.0)),
            ("=COUNTIFS(B1:B5,\">2\",C1:C4,\"5\")", E("#VALUE!")),
        ],
    );
}

#[test]
fn extremes_and_order() {
    check(
        DATA,
        &[
            ("=MAX(A1:A8)", N(9.0)),
            ("=MAX(Z1:Z3)", N(0.0)),
            ("=MAX(D1:D4)", N(3.0)),
            ("=MAX(1,\"5\")", N(5.0)),
            ("=MAXA(D1:D3)", N(3.0)),
            ("=MIN(A1:A8)", N(2.0)),
            ("=MIN(1,\"0.5\")", N(0.5)),
            ("=MINA(D1:D3)", N(0.0)),
            ("=MAXIFS(A1:A8,A1:A8,\"<9\")", N(7.0)),
            ("=MINIFS(A1:A8,A1:A8,\">2\")", N(4.0)),
            ("=MAXIFS(A1:A8,A1:A8,\">100\")", N(0.0)),
            ("=MEDIAN(A1:A8)", N(4.5)),
            ("=MEDIAN(1,2,3)", N(2.0)),
            ("=MEDIAN(Z1:Z2)", E("#NUM!")),
            ("=MODE.SNGL(A1:A8)", N(4.0)),
            ("=MODE(1,2,3)", E("#N/A")),
            ("=MODE.SNGL({5.6,4,4,3,2,4})", N(4.0)),
            ("=LARGE(A1:A8,1)", N(9.0)),
            ("=LARGE(A1:A8,3)", N(5.0)),
            ("=LARGE(A1:A8,0)", E("#NUM!")),
            ("=LARGE(A1:A8,9)", E("#NUM!")),
            ("=SMALL(A1:A8,1)", N(2.0)),
            ("=SMALL(A1:A8,2)", N(4.0)),
            ("=RANK.EQ(7,A1:A8)", N(2.0)),
            ("=RANK.EQ(4,A1:A8)", N(5.0)),
            ("=RANK.EQ(4,A1:A8,1)", N(2.0)),
            ("=RANK(9,A1:A8)", N(1.0)),
            ("=RANK.AVG(4,A1:A8)", N(6.0)),
            ("=RANK.AVG(5,A1:A8)", N(3.5)),
            ("=RANK(3,A1:A8)", E("#N/A")),
        ],
    );
}

#[test]
fn dispersion() {
    check(
        DATA,
        &[
            ("=STDEV.S(A1:A8)", Near(2.138_089_935_299_395, 1e-12)),
            ("=STDEV(A1:A8)", Near(2.138_089_935_299_395, 1e-12)),
            ("=STDEV.P(A1:A8)", N(2.0)),
            ("=STDEVP(A1:A8)", N(2.0)),
            ("=VAR.S(A1:A8)", Near(32.0 / 7.0, 1e-12)),
            ("=VAR(A1:A8)", Near(32.0 / 7.0, 1e-12)),
            ("=VAR.P(A1:A8)", N(4.0)),
            ("=VARP(A1:A8)", N(4.0)),
            ("=STDEV.S(1)", E("#DIV/0!")),
            ("=VAR.P(Z1:Z2)", E("#DIV/0!")),
            ("=STDEVA(D1:D3)", Near(1.527_525_231_651_947, 1e-12)),
            ("=VARA(D1:D3)", Near(7.0 / 3.0, 1e-12)),
        ],
    );
}

#[test]
fn percentiles() {
    check(
        DATA,
        &[
            ("=PERCENTILE.INC(B1:B5,0.3)", N(2.2)),
            ("=PERCENTILE(B1:B5,1)", N(5.0)),
            ("=PERCENTILE.INC(B1:B5,1.1)", E("#NUM!")),
            ("=PERCENTILE.EXC(B1:B5,0.25)", N(1.5)),
            ("=PERCENTILE.EXC(B1:B5,0.1)", E("#NUM!")),
            ("=QUARTILE.INC(A1:A8,1)", N(4.0)),
            ("=QUARTILE.INC(A1:A8,3)", N(5.5)),
            ("=QUARTILE(B1:B5,0)", N(1.0)),
            ("=QUARTILE.INC(B1:B5,5)", E("#NUM!")),
            ("=QUARTILE.EXC(B1:B5,1)", N(1.5)),
        ],
    );
}

#[test]
fn regression_and_correlation() {
    check(
        DATA,
        &[
            ("=CORREL(B1:B5,C1:C5)", Near(0.774_596_669_241_483, 1e-12)),
            ("=PEARSON(B1:B5,C1:C5)", Near(0.774_596_669_241_483, 1e-12)),
            ("=RSQ(C1:C5,B1:B5)", Near(0.6, 1e-12)),
            ("=SLOPE(C1:C5,B1:B5)", Near(0.6, 1e-12)),
            ("=INTERCEPT(C1:C5,B1:B5)", Near(2.2, 1e-12)),
            ("=FORECAST(6,C1:C5,B1:B5)", Near(5.8, 1e-12)),
            ("=FORECAST.LINEAR(6,C1:C5,B1:B5)", Near(5.8, 1e-12)),
            ("=COVAR(B1:B5,C1:C5)", Near(1.2, 1e-12)),
            ("=COVARIANCE.P(B1:B5,C1:C5)", Near(1.2, 1e-12)),
            ("=COVARIANCE.S(B1:B5,C1:C5)", Near(1.5, 1e-12)),
            ("=CORREL(B1:B5,C1:C4)", E("#N/A")),
            ("=CORREL(B1:B5,{1;1;1;1;1})", E("#DIV/0!")),
        ],
    );
}

#[test]
fn normal_distribution() {
    check(
        DATA,
        &[
            (
                "=NORM.DIST(42,40,1.5,TRUE)",
                Near(0.908_788_780_274_132, 1e-9),
            ),
            (
                "=NORMDIST(42,40,1.5,TRUE)",
                Near(0.908_788_780_274_132, 1e-9),
            ),
            (
                "=NORM.DIST(42,40,1.5,FALSE)",
                Near(0.109_340_049_783_996, 1e-9),
            ),
            ("=NORM.DIST(1,0,0,TRUE)", E("#NUM!")),
            ("=NORM.INV(0.908789,40,1.5)", Near(42.000_002_009_566, 1e-6)),
            ("=NORMINV(0.908789,40,1.5)", Near(42.000_002_009_566, 1e-6)),
            ("=NORM.INV(0,0,1)", E("#NUM!")),
            (
                "=NORM.S.DIST(1.333333,TRUE)",
                Near(0.908_788_725_604_095, 1e-9),
            ),
            (
                "=NORM.S.DIST(1.333333,FALSE)",
                Near(0.164_010_147_569_367, 1e-9),
            ),
            ("=NORMSDIST(1)", Near(0.841_344_746_068_543, 1e-9)),
            ("=NORM.S.INV(0.908789)", Near(1.333_334_673_044_107, 1e-6)),
            ("=NORMSINV(0.5)", Near(0.0, 1e-12)),
            ("=NORM.S.INV(1)", E("#NUM!")),
            ("=STANDARDIZE(42,40,1.5)", Near(4.0 / 3.0, 1e-12)),
            ("=STANDARDIZE(42,40,0)", E("#NUM!")),
        ],
    );
}
