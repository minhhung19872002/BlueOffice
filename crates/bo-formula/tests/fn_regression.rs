//! Regression (LINEST, LOGEST, TREND, GROWTH) and exponential-smoothing forecasts
//! (FORECAST.ETS family).
//!
//! Sources: ECMA-376 Part 1, §18.17.7 for the regression functions' layout and arguments;
//! [MS-XLSX] future functions for FORECAST.ETS*. Expected regression values are ordinary least
//! squares computed independently with NumPy (`numpy.linalg.lstsq`, `polyfit`); forecasts are
//! checked on series that additive Holt–Winters smoothing reproduces exactly (a straight line,
//! a line plus a repeating pattern), whose continuation is known by hand.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    // y = A1:A4, x = B1:B4.
    ("A1", "1"),
    ("A2", "9"),
    ("A3", "5"),
    ("A4", "8"),
    ("B1", "0"),
    ("B2", "4"),
    ("B3", "2"),
    ("B4", "3"),
    // A line 3t + 10 on the timeline 1..8 (C: time, D: value).
    ("C1", "1"),
    ("C2", "2"),
    ("C3", "3"),
    ("C4", "4"),
    ("C5", "5"),
    ("C6", "6"),
    ("C7", "7"),
    ("C8", "8"),
    ("D1", "13"),
    ("D2", "16"),
    ("D3", "19"),
    ("D4", "22"),
    ("D5", "25"),
    ("D6", "28"),
    ("D7", "31"),
    ("D8", "34"),
    // A line t plus the pattern 5, 0, -5, 0 (period 4) on the timeline 1..12 (E, F).
    ("E1", "1"),
    ("E2", "2"),
    ("E3", "3"),
    ("E4", "4"),
    ("E5", "5"),
    ("E6", "6"),
    ("E7", "7"),
    ("E8", "8"),
    ("E9", "9"),
    ("E10", "10"),
    ("E11", "11"),
    ("E12", "12"),
    ("F1", "6"),
    ("F2", "2"),
    ("F3", "-2"),
    ("F4", "4"),
    ("F5", "10"),
    ("F6", "6"),
    ("F7", "2"),
    ("F8", "8"),
    ("F9", "14"),
    ("F10", "10"),
    ("F11", "6"),
    ("F12", "12"),
];

#[test]
fn linest_and_friends() {
    check(
        DATA,
        &[
            ("=INDEX(LINEST(A1:A4,B1:B4),1)", N(2.085_714_285_714_286), "hand: NumPy lstsq slope"),
            ("=INDEX(LINEST(A1:A4,B1:B4),2)", N(1.057_142_857_142_857_8), "hand: NumPy lstsq intercept"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),2,1)", N(0.197_948_663_722_157_6), "hand: se of the slope"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),2,2)", N(0.532_993_088_747_932_8), "hand: se of the intercept"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),3,1)", N(0.982_304_147_465_437_7), "hand: r squared"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),3,2)", N(0.585_540_043_769_120_5), "hand: se of y"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),4,1)", N(111.020_833_333_333_1), "hand: F statistic"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),4,2)", N(2.0), "hand: n - 2 degrees of freedom"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),5,1)", N(38.064_285_714_285_71), "hand: regression sum of squares"),
            ("=INDEX(LINEST(A1:A4,B1:B4,TRUE,TRUE),5,2)", N(0.685_714_285_714_287), "hand: residual sum of squares"),
            ("=LINEST({1,3,5,7})", A("{2,-1}"), "hand: y = 2x - 1 on x = 1..4"),
            ("=INDEX(LINEST(A1:A4,B1:B4,FALSE),1)", N(2.413_793_103_448_276), "hand: sum(xy)/sum(x^2)"),
            ("=INDEX(LINEST(A1:A4,B1:B4,FALSE),2)", N(0.0), "ECMA-376 §18.17.7 LINEST"),
            ("=INDEX(LINEST(A1:A4,B1:B4,FALSE,TRUE),2,2)", E("#N/A"), "ECMA-376 §18.17.7 LINEST"),
            (
                "=INDEX(LINEST({142000;144000;151000;150000;139000;169000;126000;142900;163000;169000;149000},{2310,2,2,20;2333,2,2,12;2356,3,1.5,33;2379,3,2,43;2402,2,3,53;2425,4,2,23;2448,2,1.5,99;2471,2,2,34;2494,3,3,23;2517,4,4,55;2540,2,3,22}),1)",
                N(-234.237_164_471_205_7),
                "hand: NumPy lstsq, coefficients in reverse order",
            ),
            (
                "=INDEX(LINEST({142000;144000;151000;150000;139000;169000;126000;142900;163000;169000;149000},{2310,2,2,20;2333,2,2,12;2356,3,1.5,33;2379,3,2,43;2402,2,3,53;2425,4,2,23;2448,2,1.5,99;2471,2,2,34;2494,3,3,23;2517,4,4,55;2540,2,3,22}),4)",
                N(27.641_387_366_020_037),
                "hand: NumPy lstsq",
            ),
            (
                "=INDEX(LINEST({142000;144000;151000;150000;139000;169000;126000;142900;163000;169000;149000},{2310,2,2,20;2333,2,2,12;2356,3,1.5,33;2379,3,2,43;2402,2,3,53;2425,4,2,23;2448,2,1.5,99;2471,2,2,34;2494,3,3,23;2517,4,4,55;2540,2,3,22}),5)",
                N(52_317.830_507_292_165),
                "hand: NumPy lstsq",
            ),
            ("=LINEST({1,2},{1,2,3})", E("#REF!"), "ECMA-376 §18.17.7 LINEST"),
            ("=INDEX(LOGEST({33100;47300;69000;102000;150000;220000},{11;12;13;14;15;16}),1)", N(1.463_275_628_116_175_6), "hand: exp of the NumPy polyfit slope of ln y"),
            ("=INDEX(LOGEST({33100;47300;69000;102000;150000;220000},{11;12;13;14;15;16}),2)", N(495.304_770_158_728_73), "hand: exp of the intercept"),
            ("=LOGEST({1,-2})", E("#NUM!"), "ECMA-376 §18.17.7 LOGEST"),
            ("=TREND(A1:A4,B1:B4,5)", N(11.485_714_285_714_288), "hand: 1.0571 + 2.0857*5"),
            ("=TREND({1,3,5,7})", A("{1,3,5,7}"), "hand: exact fit at the known x"),
            ("=TREND({1,3,5,7},,{5,6})", A("{9,11}"), "hand: 2x - 1"),
            ("=GROWTH({33100;47300;69000;102000;150000;220000},{11;12;13;14;15;16},17)", N(320_196.718_363_472_67), "hand: exp(b + m 17)"),
            ("=GROWTH({2,4,8,16},,{5;6})", A("{32;64}"), "hand: 2^x"),
        ],
    );
}

#[test]
fn exponential_smoothing_forecasts() {
    check(
        DATA,
        &[
            (
                "=FORECAST.ETS(9,D1:D8,C1:C8)",
                Near(37.0, 1e-9),
                "hand: the line 3t + 10",
            ),
            (
                "=FORECAST.ETS(12,D1:D8,C1:C8)",
                Near(46.0, 1e-9),
                "hand: the line 3t + 10",
            ),
            (
                "=FORECAST.ETS(4,D1:D8,C1:C8)",
                Near(22.0, 1e-9),
                "hand: a point of the history",
            ),
            (
                "=FORECAST.ETS(0,D1:D8,C1:C8)",
                E("#NUM!"),
                "MS-XLSX _xlfn.FORECAST.ETS",
            ),
            (
                "=FORECAST.ETS(13,F1:F12,E1:E12)",
                Near(18.0, 1e-9),
                "hand: 13 + 5",
            ),
            (
                "=FORECAST.ETS(14,F1:F12,E1:E12)",
                Near(14.0, 1e-9),
                "hand: 14 + 0",
            ),
            (
                "=FORECAST.ETS(15,F1:F12,E1:E12)",
                Near(10.0, 1e-9),
                "hand: 15 - 5",
            ),
            (
                "=FORECAST.ETS(16,F1:F12,E1:E12,4)",
                Near(16.0, 1e-9),
                "hand: 16 + 0, given season",
            ),
            (
                "=FORECAST.ETS.SEASONALITY(F1:F12,E1:E12)",
                N(4.0),
                "hand: the pattern repeats every 4",
            ),
            (
                "=FORECAST.ETS.SEASONALITY(D1:D8,C1:C8)",
                N(1.0),
                "hand: a line has no season",
            ),
            (
                "=FORECAST.ETS.CONFINT(9,D1:D8,C1:C8)",
                Near(0.0, 1e-9),
                "hand: no error on an exact line",
            ),
            (
                "=FORECAST.ETS.STAT(D1:D8,C1:C8,6)",
                Near(0.0, 1e-9),
                "hand: MAE of an exact fit",
            ),
            (
                "=FORECAST.ETS.STAT(D1:D8,C1:C8,8)",
                N(1.0),
                "hand: step of the timeline",
            ),
            (
                "=FORECAST.ETS.STAT(D1:D8,C1:C8,9)",
                E("#NUM!"),
                "MS-XLSX _xlfn.FORECAST.ETS.STAT",
            ),
            (
                "=FORECAST.ETS(6,{13,16,19,25},{1,2,3,5})",
                Near(28.0, 1e-9),
                "hand: t = 4 completed as 22, the line goes on",
            ),
            (
                "=FORECAST.ETS(6,{13,16,19,25},{1,2,3.5,5})",
                E("#NUM!"),
                "hand: 1.5 is not a multiple of the step 1",
            ),
            (
                "=FORECAST.ETS(6,{13,16,19,25},{1,2,3,4},1,1,9)",
                E("#NUM!"),
                "MS-XLSX _xlfn.FORECAST.ETS",
            ),
        ],
    );
}
