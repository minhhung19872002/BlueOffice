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
            ("=AVERAGE(A1:A8)", N(5.0), "ECMA-376 §18.17.7 AVERAGE"),
            ("=AVERAGE(1,2,\"3\")", N(2.0), "ECMA-376 §18.17.7 AVERAGE"),
            ("=AVERAGE(D1:D4)", N(3.0), "ECMA-376 §18.17.7 AVERAGE"),
            ("=AVERAGE(Z1:Z3)", E("#DIV/0!"), "ECMA-376 §18.17.7 AVERAGE"),
            ("=AVERAGE(F1:F2)", E("#N/A"), "ECMA-376 §18.17.7 AVERAGE"),
            (
                "=AVERAGEA(D1:D3)",
                Near(4.0 / 3.0, 1e-12),
                "ECMA-376 §18.17.7 AVERAGEA",
            ),
            (
                "=AVERAGEA(D1:D4)",
                Near(4.0 / 3.0, 1e-12),
                "ECMA-376 §18.17.7 AVERAGEA",
            ),
            (
                "=AVERAGEIF(A1:A8,\">4\")",
                N(6.5),
                "ECMA-376 §18.17.7 AVERAGEIF",
            ),
            (
                "=AVERAGEIF(A1:A8,\">100\")",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 AVERAGEIF",
            ),
            (
                "=AVERAGEIF(B1:B5,\">2\",C1:C5)",
                Near(14.0 / 3.0, 1e-12),
                "ECMA-376 §18.17.7 AVERAGEIF",
            ),
            (
                "=AVERAGEIFS(C1:C5,B1:B5,\">1\",B1:B5,\"<5\")",
                Near(13.0 / 3.0, 1e-12),
                "ECMA-376 §18.17.7 AVERAGEIFS",
            ),
            (
                "=GEOMEAN(4,5,8,7,11,4,3)",
                Near(5.476_986_969_656_962, 1e-12),
                "ECMA-376 §18.17.7 GEOMEAN",
            ),
            ("=GEOMEAN(1,-1)", E("#NUM!"), "ECMA-376 §18.17.7 GEOMEAN"),
            (
                "=HARMEAN(4,5,8,7,11,4,3)",
                Near(5.028_375_962_061_728, 1e-12),
                "ECMA-376 §18.17.7 HARMEAN",
            ),
            (
                "=AVEDEV(4,5,6,7,5,4,3)",
                Near(1.020_408_163_265_306, 1e-12),
                "ECMA-376 §18.17.7 AVEDEV",
            ),
            ("=DEVSQ(4,5,8,7,11,4,3)", N(48.0), "ECMA-376 §18.17.7 DEVSQ"),
        ],
    );
}

#[test]
fn counting() {
    check(
        DATA,
        &[
            ("=COUNT(A1:A8)", N(8.0), "ECMA-376 §18.17.7 COUNT"),
            ("=COUNT(D1:D4)", N(1.0), "ECMA-376 §18.17.7 COUNT"),
            (
                "=COUNT(1,\"2\",\"x\",TRUE)",
                N(3.0),
                "ECMA-376 §18.17.7 COUNT",
            ),
            ("=COUNTA(D1:D4)", N(3.0), "ECMA-376 §18.17.7 COUNTA"),
            ("=COUNTA(D1:D4,E1)", N(4.0), "ECMA-376 §18.17.7 COUNTA"),
            ("=COUNTBLANK(D1:D4)", N(1.0), "ECMA-376 §18.17.7 COUNTBLANK"),
            ("=COUNTBLANK(E1:E2)", N(2.0), "ECMA-376 §18.17.7 COUNTBLANK"),
            ("=COUNTIF(A1:A8,4)", N(3.0), "ECMA-376 §18.17.7 COUNTIF"),
            ("=COUNTIF(A1:A8,\"4\")", N(3.0), "ECMA-376 §18.17.7 COUNTIF"),
            (
                "=COUNTIF(A1:A8,\">=5\")",
                N(4.0),
                "ECMA-376 §18.17.7 COUNTIF",
            ),
            ("=COUNTIF(D1:D4,\"x\")", N(1.0), "ECMA-376 §18.17.7 COUNTIF"),
            ("=COUNTIF(D1:D4,\"X\")", N(1.0), "ECMA-376 §18.17.7 COUNTIF"),
            ("=COUNTIF(D1:D4,TRUE)", N(1.0), "ECMA-376 §18.17.7 COUNTIF"),
            (
                "=COUNTIF(D1:D4,\"<>\")",
                N(3.0),
                "ECMA-376 §18.17.7 COUNTIF",
            ),
            (
                "=COUNTIFS(B1:B5,\">2\",C1:C5,\"5\")",
                N(2.0),
                "ECMA-376 §18.17.7 COUNTIFS",
            ),
            (
                "=COUNTIFS(B1:B5,\">2\",C1:C4,\"5\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 COUNTIFS",
            ),
        ],
    );
}

#[test]
fn extremes_and_order() {
    check(
        DATA,
        &[
            ("=MAX(A1:A8)", N(9.0), "ECMA-376 §18.17.7 MAX"),
            ("=MAX(Z1:Z3)", N(0.0), "ECMA-376 §18.17.7 MAX"),
            ("=MAX(D1:D4)", N(3.0), "ECMA-376 §18.17.7 MAX"),
            ("=MAX(1,\"5\")", N(5.0), "ECMA-376 §18.17.7 MAX"),
            ("=MAXA(D1:D3)", N(3.0), "ECMA-376 §18.17.7 MAXA"),
            ("=MIN(A1:A8)", N(2.0), "ECMA-376 §18.17.7 MIN"),
            ("=MIN(1,\"0.5\")", N(0.5), "ECMA-376 §18.17.7 MIN"),
            ("=MINA(D1:D3)", N(0.0), "ECMA-376 §18.17.7 MINA"),
            (
                "=MAXIFS(A1:A8,A1:A8,\"<9\")",
                N(7.0),
                "MS-XLSX _xlfn.MAXIFS",
            ),
            (
                "=MINIFS(A1:A8,A1:A8,\">2\")",
                N(4.0),
                "MS-XLSX _xlfn.MINIFS",
            ),
            (
                "=MAXIFS(A1:A8,A1:A8,\">100\")",
                N(0.0),
                "MS-XLSX _xlfn.MAXIFS",
            ),
            ("=MEDIAN(A1:A8)", N(4.5), "ECMA-376 §18.17.7 MEDIAN"),
            ("=MEDIAN(1,2,3)", N(2.0), "ECMA-376 §18.17.7 MEDIAN"),
            ("=MEDIAN(Z1:Z2)", E("#NUM!"), "ECMA-376 §18.17.7 MEDIAN"),
            (
                "=MODE.SNGL(A1:A8)",
                N(4.0),
                "MS-XLSX _xlfn.MODE.SNGL = ECMA-376 §18.17.7 MODE",
            ),
            ("=MODE(1,2,3)", E("#N/A"), "ECMA-376 §18.17.7 MODE"),
            (
                "=MODE.SNGL({5.6,4,4,3,2,4})",
                N(4.0),
                "MS-XLSX _xlfn.MODE.SNGL = ECMA-376 §18.17.7 MODE",
            ),
            ("=LARGE(A1:A8,1)", N(9.0), "ECMA-376 §18.17.7 LARGE"),
            ("=LARGE(A1:A8,3)", N(5.0), "ECMA-376 §18.17.7 LARGE"),
            ("=LARGE(A1:A8,0)", E("#NUM!"), "ECMA-376 §18.17.7 LARGE"),
            ("=LARGE(A1:A8,9)", E("#NUM!"), "ECMA-376 §18.17.7 LARGE"),
            ("=SMALL(A1:A8,1)", N(2.0), "ECMA-376 §18.17.7 SMALL"),
            ("=SMALL(A1:A8,2)", N(4.0), "ECMA-376 §18.17.7 SMALL"),
            (
                "=RANK.EQ(7,A1:A8)",
                N(2.0),
                "MS-XLSX _xlfn.RANK.EQ = ECMA-376 §18.17.7 RANK",
            ),
            (
                "=RANK.EQ(4,A1:A8)",
                N(5.0),
                "MS-XLSX _xlfn.RANK.EQ = ECMA-376 §18.17.7 RANK",
            ),
            (
                "=RANK.EQ(4,A1:A8,1)",
                N(2.0),
                "MS-XLSX _xlfn.RANK.EQ = ECMA-376 §18.17.7 RANK",
            ),
            ("=RANK(9,A1:A8)", N(1.0), "ECMA-376 §18.17.7 RANK"),
            ("=RANK.AVG(4,A1:A8)", N(6.0), "MS-XLSX _xlfn.RANK.AVG"),
            ("=RANK.AVG(5,A1:A8)", N(3.5), "MS-XLSX _xlfn.RANK.AVG"),
            ("=RANK(3,A1:A8)", E("#N/A"), "ECMA-376 §18.17.7 RANK"),
        ],
    );
}

#[test]
fn dispersion() {
    check(
        DATA,
        &[
            (
                "=STDEV.S(A1:A8)",
                Near(2.138_089_935_299_395, 1e-12),
                "MS-XLSX _xlfn.STDEV.S = ECMA-376 §18.17.7 STDEV",
            ),
            (
                "=STDEV(A1:A8)",
                Near(2.138_089_935_299_395, 1e-12),
                "ECMA-376 §18.17.7 STDEV",
            ),
            (
                "=STDEV.P(A1:A8)",
                N(2.0),
                "MS-XLSX _xlfn.STDEV.P = ECMA-376 §18.17.7 STDEVP",
            ),
            ("=STDEVP(A1:A8)", N(2.0), "ECMA-376 §18.17.7 STDEVP"),
            (
                "=VAR.S(A1:A8)",
                Near(32.0 / 7.0, 1e-12),
                "MS-XLSX _xlfn.VAR.S = ECMA-376 §18.17.7 VAR",
            ),
            (
                "=VAR(A1:A8)",
                Near(32.0 / 7.0, 1e-12),
                "ECMA-376 §18.17.7 VAR",
            ),
            (
                "=VAR.P(A1:A8)",
                N(4.0),
                "MS-XLSX _xlfn.VAR.P = ECMA-376 §18.17.7 VARP",
            ),
            ("=VARP(A1:A8)", N(4.0), "ECMA-376 §18.17.7 VARP"),
            (
                "=STDEV.S(1)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.STDEV.S = ECMA-376 §18.17.7 STDEV",
            ),
            (
                "=VAR.P(Z1:Z2)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.VAR.P = ECMA-376 §18.17.7 VARP",
            ),
            (
                "=STDEVA(D1:D3)",
                Near(1.527_525_231_651_947, 1e-12),
                "ECMA-376 §18.17.7 STDEVA",
            ),
            (
                "=VARA(D1:D3)",
                Near(7.0 / 3.0, 1e-12),
                "ECMA-376 §18.17.7 VARA",
            ),
        ],
    );
}

#[test]
fn percentiles() {
    check(
        DATA,
        &[
            (
                "=PERCENTILE.INC(B1:B5,0.3)",
                N(2.2),
                "MS-XLSX _xlfn.PERCENTILE.INC = ECMA-376 §18.17.7 PERCENTILE",
            ),
            (
                "=PERCENTILE(B1:B5,1)",
                N(5.0),
                "ECMA-376 §18.17.7 PERCENTILE",
            ),
            (
                "=PERCENTILE.INC(B1:B5,1.1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.PERCENTILE.INC = ECMA-376 §18.17.7 PERCENTILE",
            ),
            (
                "=PERCENTILE.EXC(B1:B5,0.25)",
                N(1.5),
                "MS-XLSX _xlfn.PERCENTILE.EXC",
            ),
            (
                "=PERCENTILE.EXC(B1:B5,0.1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.PERCENTILE.EXC",
            ),
            (
                "=QUARTILE.INC(A1:A8,1)",
                N(4.0),
                "MS-XLSX _xlfn.QUARTILE.INC = ECMA-376 §18.17.7 QUARTILE",
            ),
            (
                "=QUARTILE.INC(A1:A8,3)",
                N(5.5),
                "MS-XLSX _xlfn.QUARTILE.INC = ECMA-376 §18.17.7 QUARTILE",
            ),
            ("=QUARTILE(B1:B5,0)", N(1.0), "ECMA-376 §18.17.7 QUARTILE"),
            (
                "=QUARTILE.INC(B1:B5,5)",
                E("#NUM!"),
                "MS-XLSX _xlfn.QUARTILE.INC = ECMA-376 §18.17.7 QUARTILE",
            ),
            (
                "=QUARTILE.EXC(B1:B5,1)",
                N(1.5),
                "MS-XLSX _xlfn.QUARTILE.EXC",
            ),
        ],
    );
}

#[test]
fn regression_and_correlation() {
    check(
        DATA,
        &[
            (
                "=CORREL(B1:B5,C1:C5)",
                Near(0.774_596_669_241_483, 1e-12),
                "ECMA-376 §18.17.7 CORREL",
            ),
            (
                "=PEARSON(B1:B5,C1:C5)",
                Near(0.774_596_669_241_483, 1e-12),
                "ECMA-376 §18.17.7 PEARSON",
            ),
            (
                "=RSQ(C1:C5,B1:B5)",
                Near(0.6, 1e-12),
                "ECMA-376 §18.17.7 RSQ",
            ),
            (
                "=SLOPE(C1:C5,B1:B5)",
                Near(0.6, 1e-12),
                "ECMA-376 §18.17.7 SLOPE",
            ),
            (
                "=INTERCEPT(C1:C5,B1:B5)",
                Near(2.2, 1e-12),
                "ECMA-376 §18.17.7 INTERCEPT",
            ),
            (
                "=FORECAST(6,C1:C5,B1:B5)",
                Near(5.8, 1e-12),
                "ECMA-376 §18.17.7 FORECAST",
            ),
            (
                "=FORECAST.LINEAR(6,C1:C5,B1:B5)",
                Near(5.8, 1e-12),
                "MS-XLSX _xlfn.FORECAST.LINEAR = ECMA-376 §18.17.7 FORECAST",
            ),
            (
                "=COVAR(B1:B5,C1:C5)",
                Near(1.2, 1e-12),
                "ECMA-376 §18.17.7 COVAR",
            ),
            (
                "=COVARIANCE.P(B1:B5,C1:C5)",
                Near(1.2, 1e-12),
                "MS-XLSX _xlfn.COVARIANCE.P = ECMA-376 §18.17.7 COVAR",
            ),
            (
                "=COVARIANCE.S(B1:B5,C1:C5)",
                Near(1.5, 1e-12),
                "MS-XLSX _xlfn.COVARIANCE.S",
            ),
            (
                "=CORREL(B1:B5,C1:C4)",
                E("#N/A"),
                "ECMA-376 §18.17.7 CORREL",
            ),
            (
                "=CORREL(B1:B5,{1;1;1;1;1})",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 CORREL",
            ),
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
                "MS-XLSX _xlfn.NORM.DIST = ECMA-376 §18.17.7 NORMDIST",
            ),
            (
                "=NORMDIST(42,40,1.5,TRUE)",
                Near(0.908_788_780_274_132, 1e-9),
                "ECMA-376 §18.17.7 NORMDIST",
            ),
            (
                "=NORM.DIST(42,40,1.5,FALSE)",
                Near(0.109_340_049_783_996, 1e-9),
                "MS-XLSX _xlfn.NORM.DIST = ECMA-376 §18.17.7 NORMDIST",
            ),
            (
                "=NORM.DIST(1,0,0,TRUE)",
                E("#NUM!"),
                "MS-XLSX _xlfn.NORM.DIST = ECMA-376 §18.17.7 NORMDIST",
            ),
            (
                "=NORM.INV(0.908789,40,1.5)",
                Near(42.000_002_009_566, 1e-6),
                "MS-XLSX _xlfn.NORM.INV = ECMA-376 §18.17.7 NORMINV",
            ),
            (
                "=NORMINV(0.908789,40,1.5)",
                Near(42.000_002_009_566, 1e-6),
                "ECMA-376 §18.17.7 NORMINV",
            ),
            (
                "=NORM.INV(0,0,1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.NORM.INV = ECMA-376 §18.17.7 NORMINV",
            ),
            (
                "=NORM.S.DIST(1.333333,TRUE)",
                Near(0.908_788_725_604_095, 1e-9),
                "MS-XLSX _xlfn.NORM.S.DIST = ECMA-376 §18.17.7 NORMSDIST",
            ),
            (
                "=NORM.S.DIST(1.333333,FALSE)",
                Near(0.164_010_147_569_367, 1e-9),
                "MS-XLSX _xlfn.NORM.S.DIST = ECMA-376 §18.17.7 NORMSDIST",
            ),
            (
                "=NORMSDIST(1)",
                Near(0.841_344_746_068_543, 1e-9),
                "ECMA-376 §18.17.7 NORMSDIST",
            ),
            (
                "=NORM.S.INV(0.908789)",
                Near(1.333_334_673_044_107, 1e-6),
                "MS-XLSX _xlfn.NORM.S.INV = ECMA-376 §18.17.7 NORMSINV",
            ),
            (
                "=NORMSINV(0.5)",
                Near(0.0, 1e-12),
                "ECMA-376 §18.17.7 NORMSINV",
            ),
            (
                "=NORM.S.INV(1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.NORM.S.INV = ECMA-376 §18.17.7 NORMSINV",
            ),
            (
                "=STANDARDIZE(42,40,1.5)",
                Near(4.0 / 3.0, 1e-12),
                "ECMA-376 §18.17.7 STANDARDIZE",
            ),
            (
                "=STANDARDIZE(42,40,0)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 STANDARDIZE",
            ),
        ],
    );
}

/// Additional value, edge and error rows for the Statistical list (gap #37).
/// Data as in the module docs.
#[test]
fn stats_edge_cases() {
    check(
        DATA,
        &[
            ("=AVERAGEA(B1:B5)", N(3.0), "hand: (1+2+3+4+5)/5"),
            (
                "=AVERAGEA(Z1:Z3)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 AVERAGEA; hand: no values",
            ),
            (
                "=AVERAGEA(F1:F2)",
                E("#N/A"),
                "ECMA-376 §18.17.7 AVERAGEA; hand: error propagates",
            ),
            (
                "=AVERAGEIF(D1:D4,\"x\",B1:B4)",
                N(1.0),
                "hand: only D1 matches → B1",
            ),
            (
                "=AVERAGEIFS(A1:A8,A1:A8,\"4\")",
                N(4.0),
                "hand: the three 4s",
            ),
            (
                "=AVERAGEIFS(C1:C5,B1:B5,\">10\")",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.AVERAGEIFS = ECMA-376 §18.17.7 AVERAGEIFS; hand: no match",
            ),
            (
                "=AVERAGEIFS(C1:C5,B1:B5,\">1\",B1:B4,\"<5\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 AVERAGEIFS; hand: range size mismatch",
            ),
            ("=COUNT(Z1:Z5)", N(0.0), "hand: no numbers"),
            (
                "=COUNT(F1:F2)",
                N(1.0),
                "ECMA-376 §18.17.7 COUNT; hand: errors are not counted",
            ),
            ("=COUNTA(Z1:Z5)", N(0.0), "hand: all blank"),
            (
                "=COUNTA(1,\"\",FALSE)",
                N(3.0),
                "ECMA-376 §18.17.7 COUNTA; hand: empty text counts",
            ),
            ("=COUNTA(F1:F2)", N(2.0), "hand: errors count as non-empty"),
            ("=COUNTBLANK(A1:A8)", N(0.0), "hand: no blanks"),
            ("=COUNTBLANK(Z1:Z5)", N(5.0), "hand: five blanks"),
            ("=COUNTIF(A1:A8,\"<>4\")", N(5.0), "hand: 8 - three 4s"),
            (
                "=COUNTIF(Z1:Z3,\"\")",
                N(3.0),
                "ECMA-376 §18.17.7 COUNTIF; hand: blanks match empty criterion",
            ),
            (
                "=COUNTIFS(A1:A8,\">3\",A1:A8,\"<6\")",
                N(5.0),
                "hand: 4,4,4,5,5",
            ),
            (
                "=COUNTIFS(D1:D4,\"*\")",
                N(1.0),
                "ECMA-376 §18.17.7 COUNTIFS; hand: * matches text only (x)",
            ),
            (
                "=MAX(F1:F2)",
                E("#N/A"),
                "ECMA-376 §18.17.7 MAX; hand: error propagates",
            ),
            (
                "=MIN(Z1:Z3)",
                N(0.0),
                "ECMA-376 §18.17.7 MIN; hand: no numbers → 0",
            ),
            ("=MIN(D1:D4)", N(3.0), "hand: only number is 3"),
            (
                "=MIN(F1:F2)",
                E("#N/A"),
                "ECMA-376 §18.17.7 MIN; hand: error propagates",
            ),
            (
                "=MIN(-1,\"-2\")",
                N(-2.0),
                "hand: numeric text argument coerced",
            ),
            ("=MAXIFS(C1:C5,B1:B5,\"<3\")", N(4.0), "hand: C1=2, C2=4"),
            (
                "=MAXIFS(C1:C5,B1:B4,\"<3\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.MAXIFS; hand: range size mismatch",
            ),
            (
                "=MINIFS(C1:C5,B1:B5,\">2\")",
                N(4.0),
                "hand: C3..C5 = 5,4,5",
            ),
            (
                "=MINIFS(A1:A8,A1:A8,\">100\")",
                N(0.0),
                "MS-XLSX _xlfn.MINIFS; hand: no match → 0",
            ),
            (
                "=MINIFS(C1:C5,B1:B4,\">2\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.MINIFS; hand: range size mismatch",
            ),
            ("=MEDIAN(D1:D4)", N(3.0), "hand: only number is 3"),
            (
                "=MEDIAN(F1:F2)",
                E("#N/A"),
                "ECMA-376 §18.17.7 MEDIAN; hand: error propagates",
            ),
            (
                "=MODE.SNGL(1,2,3)",
                E("#N/A"),
                "MS-XLSX _xlfn.MODE.SNGL = ECMA-376 §18.17.7 MODE; hand: no repeats",
            ),
            (
                "=MODE.SNGL(1,1,2,2)",
                N(1.0),
                "hand: tie → first value reaching the top count",
            ),
            ("=SMALL(A1:A8,8)", N(9.0), "hand: 8th smallest = largest"),
            (
                "=SMALL(A1:A8,0)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 SMALL; hand: k < 1",
            ),
            (
                "=SMALL(Z1:Z2,1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 SMALL; hand: no numbers",
            ),
            (
                "=RANK.EQ(2,A1:A8,1)",
                N(1.0),
                "hand: smallest in ascending order",
            ),
            (
                "=RANK.EQ(10,A1:A8)",
                E("#N/A"),
                "MS-XLSX _xlfn.RANK.EQ = ECMA-376 §18.17.7 RANK; hand: not in list",
            ),
            (
                "=RANK.AVG(4,A1:A8,1)",
                N(3.0),
                "hand: ascending positions 2,3,4 → 3",
            ),
            (
                "=RANK.AVG(3,A1:A8)",
                E("#N/A"),
                "MS-XLSX _xlfn.RANK.AVG; hand: not in list",
            ),
            (
                "=STDEV.S(2,4)",
                Near(std::f64::consts::SQRT_2, 1e-12),
                "hand: mean 3, Σd²=2, /(n-1)=2, √2",
            ),
            (
                "=STDEV.S(Z1:Z3)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.STDEV.S = ECMA-376 §18.17.7 STDEV; hand: n < 2",
            ),
            ("=STDEV.P(5)", N(0.0), "hand: single value"),
            (
                "=STDEV.P(Z1:Z2)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.STDEV.P = ECMA-376 §18.17.7 STDEVP; hand: no values",
            ),
            (
                "=VAR.S(1,2,3,4)",
                Near(5.0 / 3.0, 1e-12),
                "hand: mean 2.5, Σd²=5, /3",
            ),
            (
                "=VAR.S(1)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.VAR.S = ECMA-376 §18.17.7 VAR; hand: n < 2",
            ),
            ("=VAR.P(1,2,3,4)", N(1.25), "hand: Σd²=5, /4"),
            (
                "=VAR.P(F1:F2)",
                E("#N/A"),
                "MS-XLSX _xlfn.VAR.P = ECMA-376 §18.17.7 VARP; hand: error propagates",
            ),
            ("=PERCENTILE.INC(B1:B5,0)", N(1.0), "hand: minimum"),
            (
                "=PERCENTILE.INC(B1:B5,0.5)",
                N(3.0),
                "hand: rank 1+0.5·4 = 3",
            ),
            (
                "=PERCENTILE.INC(B1:B5,-0.1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.PERCENTILE.INC = ECMA-376 §18.17.7 PERCENTILE; hand: k < 0",
            ),
            (
                "=PERCENTILE.INC(Z1:Z2,0.5)",
                E("#NUM!"),
                "MS-XLSX _xlfn.PERCENTILE.INC = ECMA-376 §18.17.7 PERCENTILE; hand: empty array",
            ),
            ("=QUARTILE.INC(B1:B5,2)", N(3.0), "hand: median"),
            ("=QUARTILE.INC(B1:B5,4)", N(5.0), "hand: maximum"),
            (
                "=QUARTILE.INC(B1:B5,-1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.QUARTILE.INC = ECMA-376 §18.17.7 QUARTILE; hand: quart < 0",
            ),
            (
                "=CORREL(B1:B5,B1:B5)",
                Near(1.0, 1e-12),
                "hand: a series with itself",
            ),
            (
                "=CORREL({1;2;3},{3;2;1})",
                Near(-1.0, 1e-12),
                "hand: perfectly anti-correlated",
            ),
            (
                "=FORECAST.LINEAR(0,C1:C5,B1:B5)",
                Near(2.2, 1e-12),
                "hand: intercept 2.2 (slope 0.6)",
            ),
            (
                "=FORECAST.LINEAR(6,C1:C5,B1:B4)",
                E("#N/A"),
                "MS-XLSX _xlfn.FORECAST.LINEAR = ECMA-376 §18.17.7 FORECAST; hand: size mismatch",
            ),
            (
                "=FORECAST.LINEAR(1,{1;2},{3;3})",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.FORECAST.LINEAR = ECMA-376 §18.17.7 FORECAST; hand: var(x) = 0",
            ),
            (
                "=NORM.DIST(0,0,1,TRUE)",
                N(0.5),
                "hand: symmetric CDF at the mean",
            ),
            (
                "=NORM.DIST(0,0,1,FALSE)",
                Near(0.398_942_280_401_433, 1e-12),
                "hand: 1/√(2π)",
            ),
            (
                "=NORM.DIST(1,0,-1,TRUE)",
                E("#NUM!"),
                "MS-XLSX _xlfn.NORM.DIST = ECMA-376 §18.17.7 NORMDIST; hand: σ ≤ 0",
            ),
            (
                "=NORM.INV(0.5,10,2)",
                Near(10.0, 1e-9),
                "hand: median = mean",
            ),
            (
                "=NORM.INV(1,0,1)",
                E("#NUM!"),
                "MS-XLSX _xlfn.NORM.INV = ECMA-376 §18.17.7 NORMINV; hand: p ≥ 1",
            ),
            (
                "=NORM.INV(0.5,0,0)",
                E("#NUM!"),
                "MS-XLSX _xlfn.NORM.INV = ECMA-376 §18.17.7 NORMINV; hand: σ ≤ 0",
            ),
        ],
    );
}
