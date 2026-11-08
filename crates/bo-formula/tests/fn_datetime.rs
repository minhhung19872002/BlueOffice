//! Date & time functions and the 1900 / 1904 date systems.
//!
//! Sources: ECMA-376 Part 1, §18.17.4 (date systems: 1900 system with the
//! fictitious 1900-02-29 = serial 60, 1904 system base 1904-01-01 = 0) and
//! §18.17.7 (DATE, DATEVALUE, DAY, DAYS360, EDATE, EOMONTH, HOUR, MINUTE, MONTH,
//! NETWORKDAYS, NOW, SECOND, TIME, TIMEVALUE, TODAY, WEEKDAY, WEEKNUM, WORKDAY,
//! YEAR, YEARFRAC) incl. worked examples; OpenFormula (ODF 1.3 Part 4) for DAYS,
//! ISOWEEKNUM, DATEDIF; NETWORKDAYS.INTL / WORKDAY.INTL weekend codes per their
//! published definitions. Serials were cross-checked with proleptic-Gregorian
//! day counts (serial = days since 1899-12-30 for dates after 1900-03-01).
//!
//! Ambiguity noted: text dates such as "8/22/2011" are read month-first
//! (en-US default locale); ISO `yyyy-mm-dd` is locale-independent.

mod common;
use common::*;

#[test]
fn date_and_serials() {
    check(
        &[],
        &[
            ("=DATE(1900,1,1)", N(1.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(1900,2,28)", N(59.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(1900,2,29)", N(60.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(1900,3,1)", N(61.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2008,1,1)", N(39_448.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2024,3,15)", N(45_366.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(108,1,2)", N(39_449.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2008,14,2)", N(39_846.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2008,-3,2)", N(39_327.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2008,1,35)", N(39_482.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2008,1,-15)", N(39_432.0), "ECMA-376 §18.17.7 DATE"),
            (
                "=DATE(9999,12,31)",
                N(2_958_465.0),
                "ECMA-376 §18.17.7 DATE",
            ),
            ("=DATE(-1,1,1)", E("#NUM!"), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(10000,1,1)", E("#NUM!"), "ECMA-376 §18.17.7 DATE"),
            ("=DAY(60)", N(29.0), "ECMA-376 §18.17.7 DAY"),
            ("=MONTH(60)", N(2.0), "ECMA-376 §18.17.7 MONTH"),
            ("=DAY(61)", N(1.0), "ECMA-376 §18.17.7 DAY"),
            ("=YEAR(45366)", N(2024.0), "ECMA-376 §18.17.7 YEAR"),
            ("=MONTH(45366)", N(3.0), "ECMA-376 §18.17.7 MONTH"),
            ("=DAY(45366)", N(15.0), "ECMA-376 §18.17.7 DAY"),
            ("=YEAR(\"2024-03-15\")", N(2024.0), "ECMA-376 §18.17.7 YEAR"),
            ("=YEAR(0)", N(1900.0), "ECMA-376 §18.17.7 YEAR"),
            ("=MONTH(0)", N(1.0), "ECMA-376 §18.17.7 MONTH"),
            ("=DAY(0)", N(0.0), "ECMA-376 §18.17.7 DAY"),
            ("=YEAR(-1)", E("#NUM!"), "ECMA-376 §18.17.7 YEAR"),
            ("=YEAR(\"abc\")", E("#VALUE!"), "ECMA-376 §18.17.7 YEAR"),
            (
                "=DATEVALUE(\"8/22/2011\")",
                N(40_777.0),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            (
                "=DATEVALUE(\"22-MAY-2011\")",
                N(40_685.0),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            (
                "=DATEVALUE(\"2011/02/23\")",
                N(40_597.0),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            (
                "=DATEVALUE(\"2024-03-15\")",
                N(45_366.0),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            (
                "=DATEVALUE(\"15 March 2024\")",
                N(45_366.0),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            (
                "=DATEVALUE(\"abc\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            (
                "=\"2024-03-15\"+1",
                N(45_367.0),
                "ECMA-376 §18.17.2 formula syntax",
            ),
        ],
    );
}

#[test]
fn time_parts() {
    check(
        &[],
        &[
            ("=TIME(12,0,0)", N(0.5), "ECMA-376 §18.17.7 TIME"),
            (
                "=TIME(16,48,10)",
                Near(0.700_115_741, 1e-9),
                "ECMA-376 §18.17.7 TIME",
            ),
            ("=TIME(0,0,0)", N(0.0), "ECMA-376 §18.17.7 TIME"),
            ("=TIME(27,0,0)", N(0.125), "ECMA-376 §18.17.7 TIME"),
            (
                "=TIME(0,750,0)",
                Near(0.520_833_333, 1e-9),
                "ECMA-376 §18.17.7 TIME",
            ),
            ("=TIME(0,-1,0)", E("#NUM!"), "ECMA-376 §18.17.7 TIME"),
            (
                "=TIMEVALUE(\"2:24 AM\")",
                Near(0.1, 1e-12),
                "ECMA-376 §18.17.7 TIMEVALUE",
            ),
            (
                "=TIMEVALUE(\"22-Aug-2011 6:35 AM\")",
                Near(0.274_305_556, 1e-9),
                "ECMA-376 §18.17.7 TIMEVALUE",
            ),
            (
                "=TIMEVALUE(\"abc\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 TIMEVALUE",
            ),
            ("=HOUR(0.75)", N(18.0), "ECMA-376 §18.17.7 HOUR"),
            (
                "=HOUR(\"7/18/2011 7:45\")",
                N(7.0),
                "ECMA-376 §18.17.7 HOUR",
            ),
            ("=MINUTE(\"12:45:00\")", N(45.0), "ECMA-376 §18.17.7 MINUTE"),
            (
                "=SECOND(\"4:48:18 PM\")",
                N(18.0),
                "ECMA-376 §18.17.7 SECOND",
            ),
            ("=SECOND(\"4:48 PM\")", N(0.0), "ECMA-376 §18.17.7 SECOND"),
            ("=HOUR(-1)", E("#NUM!"), "ECMA-376 §18.17.7 HOUR"),
            ("=TODAY()", N(45_366.0), "ECMA-376 §18.17.7 TODAY"),
            ("=NOW()", N(45_366.5), "ECMA-376 §18.17.7 NOW"),
        ],
    );
}

#[test]
fn weekday_weeknum() {
    check(
        &[],
        &[
            // 2024-03-15 is a Friday.
            ("=WEEKDAY(45366)", N(6.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=WEEKDAY(45366,2)", N(5.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=WEEKDAY(45366,3)", N(4.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=WEEKDAY(45366,11)", N(5.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=WEEKDAY(45366,16)", N(7.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=WEEKDAY(45366,17)", N(6.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=WEEKDAY(45366,4)", E("#NUM!"), "ECMA-376 §18.17.7 WEEKDAY"),
            (
                "=WEEKNUM(DATE(2012,3,9))",
                N(10.0),
                "ECMA-376 §18.17.7 WEEKNUM",
            ),
            (
                "=WEEKNUM(DATE(2012,3,9),2)",
                N(11.0),
                "ECMA-376 §18.17.7 WEEKNUM",
            ),
            (
                "=WEEKNUM(DATE(2024,1,1))",
                N(1.0),
                "ECMA-376 §18.17.7 WEEKNUM",
            ),
            (
                "=WEEKNUM(DATE(2021,1,1),21)",
                N(53.0),
                "ECMA-376 §18.17.7 WEEKNUM",
            ),
            (
                "=WEEKNUM(45366,99)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 WEEKNUM",
            ),
            (
                "=ISOWEEKNUM(DATE(2012,3,9))",
                N(10.0),
                "OpenFormula ISOWEEKNUM",
            ),
            (
                "=ISOWEEKNUM(DATE(2021,1,1))",
                N(53.0),
                "OpenFormula ISOWEEKNUM",
            ),
            (
                "=ISOWEEKNUM(DATE(2024,12,30))",
                N(1.0),
                "OpenFormula ISOWEEKNUM",
            ),
        ],
    );
}

#[test]
fn month_arithmetic_and_differences() {
    check(
        &[],
        &[
            (
                "=EDATE(DATE(2011,1,15),1)",
                N(40_589.0),
                "ECMA-376 §18.17.7 EDATE",
            ),
            (
                "=EDATE(DATE(2011,1,15),-1)",
                N(40_527.0),
                "ECMA-376 §18.17.7 EDATE",
            ),
            (
                "=EDATE(DATE(2011,1,15),2)",
                N(40_617.0),
                "ECMA-376 §18.17.7 EDATE",
            ),
            (
                "=EDATE(DATE(2011,1,31),1)",
                N(40_602.0),
                "ECMA-376 §18.17.7 EDATE",
            ),
            (
                "=EOMONTH(DATE(2011,1,1),1)",
                N(40_602.0),
                "ECMA-376 §18.17.7 EOMONTH",
            ),
            (
                "=EOMONTH(DATE(2011,1,1),-3)",
                N(40_482.0),
                "ECMA-376 §18.17.7 EOMONTH",
            ),
            (
                "=EOMONTH(DATE(2024,2,10),0)",
                N(45_351.0),
                "ECMA-376 §18.17.7 EOMONTH",
            ),
            (
                "=DATEDIF(DATE(2001,1,1),DATE(2003,1,1),\"Y\")",
                N(2.0),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"D\")",
                N(440.0),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"YD\")",
                N(75.0),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"MD\")",
                N(14.0),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"M\")",
                N(14.0),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"YM\")",
                N(2.0),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2003,1,1),DATE(2001,1,1),\"Y\")",
                E("#NUM!"),
                "OpenFormula DATEDIF",
            ),
            (
                "=DATEDIF(DATE(2001,1,1),DATE(2003,1,1),\"X\")",
                E("#NUM!"),
                "OpenFormula DATEDIF",
            ),
            (
                "=DAYS(DATE(2021,3,15),DATE(2021,2,1))",
                N(42.0),
                "OpenFormula DAYS",
            ),
            (
                "=DAYS(\"15-MAR-2021\",\"1-FEB-2021\")",
                N(42.0),
                "OpenFormula DAYS",
            ),
            (
                "=DAYS360(DATE(2011,1,30),DATE(2011,12,31))",
                N(330.0),
                "ECMA-376 §18.17.7 DAYS360",
            ),
            (
                "=DAYS360(DATE(2011,1,30),DATE(2011,12,31),TRUE)",
                N(330.0),
                "ECMA-376 §18.17.7 DAYS360",
            ),
            (
                "=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30))",
                Near(0.580_555_556, 1e-9),
                "ECMA-376 §18.17.7 YEARFRAC",
            ),
            (
                "=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),1)",
                Near(0.576_502_732, 1e-9),
                "ECMA-376 §18.17.7 YEARFRAC",
            ),
            (
                "=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),2)",
                Near(0.586_111_111, 1e-9),
                "ECMA-376 §18.17.7 YEARFRAC",
            ),
            (
                "=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),3)",
                Near(0.578_082_192, 1e-9),
                "ECMA-376 §18.17.7 YEARFRAC",
            ),
            (
                "=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),4)",
                Near(0.580_555_556, 1e-9),
                "ECMA-376 §18.17.7 YEARFRAC",
            ),
            (
                "=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),5)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 YEARFRAC",
            ),
        ],
    );
}

#[test]
fn workdays() {
    check(
        &[
            ("H1", "=DATE(2008,11,26)"),
            ("H2", "=DATE(2008,12,4)"),
            ("H3", "=DATE(2009,1,21)"),
        ],
        &[
            ("=NETWORKDAYS(DATE(2012,10,1),DATE(2013,3,1))", N(110.0), "ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS(DATE(2012,10,1),DATE(2013,3,1),DATE(2012,11,22))", N(109.0), "ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS(DATE(2012,10,1),DATE(2013,3,1),{\"2012-11-22\",\"2012-12-04\",\"2013-01-21\"})", N(107.0), "ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,1,31))", N(22.0), "MS-XLSX _xlfn.NETWORKDAYS.INTL = ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),7,{\"2006/1/2\",\"2006/1/16\"})", N(22.0), "MS-XLSX _xlfn.NETWORKDAYS.INTL = ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS.INTL(DATE(2006,2,28),DATE(2006,1,31))", N(-21.0), "MS-XLSX _xlfn.NETWORKDAYS.INTL = ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),\"0010001\",{\"2006/1/2\",\"2006/1/16\"})", N(20.0), "MS-XLSX _xlfn.NETWORKDAYS.INTL = ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),99)", E("#NUM!"), "MS-XLSX _xlfn.NETWORKDAYS.INTL = ECMA-376 §18.17.7 NETWORKDAYS"),
            ("=WORKDAY(DATE(2008,10,1),151)", N(39_933.0), "ECMA-376 §18.17.7 WORKDAY"),
            ("=WORKDAY(DATE(2008,10,1),151,H1:H3)", N(39_938.0), "ECMA-376 §18.17.7 WORKDAY"),
            ("=WORKDAY(DATE(2024,3,15),-5)", N(45_359.0), "ECMA-376 §18.17.7 WORKDAY"),
            ("=WORKDAY(DATE(2024,3,15),0)", N(45_366.0), "ECMA-376 §18.17.7 WORKDAY"),
            ("=WORKDAY.INTL(DATE(2012,1,1),30,0)", E("#NUM!"), "MS-XLSX _xlfn.WORKDAY.INTL = ECMA-376 §18.17.7 WORKDAY"),
            ("=WORKDAY.INTL(DATE(2012,1,1),90,11)", N(41_013.0), "MS-XLSX _xlfn.WORKDAY.INTL = ECMA-376 §18.17.7 WORKDAY"),
            ("=WORKDAY.INTL(DATE(2012,1,1),30,17)", N(40_944.0), "MS-XLSX _xlfn.WORKDAY.INTL = ECMA-376 §18.17.7 WORKDAY"),
        ],
    );
}

/// The 1904 date system shifts every serial by 1462 days (ECMA-376 Part 1, §18.17.4.1).
#[test]
fn date_system_1904() {
    let opts = Opts {
        date_system: bo_formula::DateSystem::Date1904,
        now: Some(45_366.5 - 1462.0),
    };
    check_with(
        &[],
        &[
            ("=DATE(1904,1,1)", N(0.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(1904,1,2)", N(1.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(2008,1,1)", N(37_986.0), "ECMA-376 §18.17.7 DATE"),
            ("=DATE(1900,1,1)", E("#NUM!"), "ECMA-376 §18.17.7 DATE"),
            ("=YEAR(0)", N(1904.0), "ECMA-376 §18.17.7 YEAR"),
            ("=DAY(37986)", N(1.0), "ECMA-376 §18.17.7 DAY"),
            (
                "=DATEVALUE(\"2008-01-01\")",
                N(37_986.0),
                "ECMA-376 §18.17.7 DATEVALUE",
            ),
            ("=WEEKDAY(37986)", N(3.0), "ECMA-376 §18.17.7 WEEKDAY"),
            ("=TODAY()", N(43_904.0), "ECMA-376 §18.17.7 TODAY"),
            (
                "=TEXT(37986,\"yyyy-mm-dd\")",
                T("2008-01-01"),
                "ECMA-376 §18.17.7 TEXT",
            ),
        ],
        opts,
    );
}

/// Additional value, edge and error rows for the Date & Time list (gap #37).
/// Serial anchor: 2024-03-15 (Friday) = 45366 (ECMA-376 §18.17.4 1900 system).
#[test]
fn datetime_edge_cases() {
    check(
        &[],
        &[
            ("=TIMEVALUE(\"12:00\")", N(0.5), "hand: 12/24"),
            ("=TIMEVALUE(\"18:00:00\")", N(0.75), "hand: 18/24"),
            ("=MONTH(\"2024-12-31\")", N(12.0), "hand: December"),
            (
                "=MONTH(-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 MONTH; hand: negative serial",
            ),
            (
                "=MONTH(\"x\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 MONTH; hand: not a date",
            ),
            ("=DAY(2958465)", N(31.0), "hand: 9999-12-31"),
            (
                "=DAY(2958466)",
                E("#NUM!"),
                "ECMA-376 §18.17.4; hand: beyond 9999-12-31",
            ),
            (
                "=HOUR(0.999988426)",
                N(23.0),
                "hand: 0.999988426·24 = 23.9997",
            ),
            (
                "=HOUR(\"abc\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 HOUR; hand: not a time",
            ),
            ("=MINUTE(0.5)", N(0.0), "hand: 12:00:00"),
            ("=MINUTE(TIME(10,59,59))", N(59.0), "hand: 10:59:59"),
            (
                "=MINUTE(-0.5)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 MINUTE; hand: negative serial",
            ),
            (
                "=MINUTE(\"x\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 MINUTE; hand: not a time",
            ),
            ("=SECOND(TIME(1,2,3))", N(3.0), "hand: 01:02:03"),
            (
                "=SECOND(-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 SECOND; hand: negative serial",
            ),
            (
                "=TIME(0,0,-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 TIME; hand: negative total",
            ),
            (
                "=TIME(24,0,0)",
                N(0.0),
                "ECMA-376 §18.17.7 TIME; hand: 24h wraps to 0",
            ),
            (
                "=TODAY()+1",
                N(45_367.0),
                "hand: fixed now 45366.5 → today 45366, +1",
            ),
            (
                "=INT(NOW())=TODAY()",
                B(true),
                "hand: TODAY is the date part of NOW",
            ),
            ("=NOW()-TODAY()", N(0.5), "hand: fixed now at 12:00"),
            (
                "=YEAR(DATE(2024,13,1))",
                N(2025.0),
                "hand: month 13 rolls over",
            ),
            (
                "=DATEVALUE(\"2024-02-30\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 DATEVALUE; hand: invalid day",
            ),
            (
                "=WEEKDAY(\"x\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 WEEKDAY; hand: not a date",
            ),
            (
                "=WEEKNUM(DATE(2024,12,31))",
                N(53.0),
                "hand: 2024-01-01 Monday; Sunday-start weeks → 53",
            ),
            (
                "=ISOWEEKNUM(DATE(2024,1,1))",
                N(1.0),
                "hand: 2024-01-01 is Monday of ISO week 1",
            ),
            (
                "=ISOWEEKNUM(-1)",
                E("#NUM!"),
                "OpenFormula ISOWEEKNUM; hand: negative serial",
            ),
            (
                "=ISOWEEKNUM(\"x\")",
                E("#VALUE!"),
                "OpenFormula ISOWEEKNUM; hand: not a date",
            ),
            (
                "=EOMONTH(DATE(2024,1,31),1)",
                N(45_351.0),
                "hand: 2024-02-29 = 45366 - 15",
            ),
            (
                "=EOMONTH(DATE(2023,12,15),2)",
                N(45_351.0),
                "hand: two months after Dec 2023 ends 2024-02-29",
            ),
            (
                "=EOMONTH(-1,0)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 EOMONTH; hand: negative serial",
            ),
            (
                "=EDATE(DATE(2024,2,29),12)",
                N(45_716.0),
                "hand: 2025-02-28 = 45351 + 365",
            ),
            (
                "=EDATE(\"x\",1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 EDATE; hand: not a date",
            ),
            (
                "=DAYS(DATE(2021,2,1),DATE(2021,3,15))",
                N(-42.0),
                "hand: 1 Feb is 42 days before 15 Mar",
            ),
            (
                "=DAYS(\"x\",1)",
                E("#VALUE!"),
                "OpenFormula DAYS; hand: not a date",
            ),
            (
                "=DATEDIF(DATE(2024,1,31),DATE(2024,3,1),\"M\")",
                N(1.0),
                "OpenFormula DATEDIF; hand: one complete month",
            ),
            (
                "=NETWORKDAYS(DATE(2024,3,15),DATE(2024,3,18))",
                N(2.0),
                "hand: Fri 15 and Mon 18",
            ),
            (
                "=NETWORKDAYS(DATE(2024,3,16),DATE(2024,3,17))",
                N(0.0),
                "hand: Sat-Sun only",
            ),
            (
                "=NETWORKDAYS(DATE(2024,3,18),DATE(2024,3,15))",
                N(-2.0),
                "ECMA-376 §18.17.7 NETWORKDAYS; hand: reversed → negative",
            ),
            (
                "=NETWORKDAYS(\"x\",1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 NETWORKDAYS; hand: not a date",
            ),
            (
                "=NETWORKDAYS.INTL(DATE(2024,3,15),DATE(2024,3,17),7)",
                N(1.0),
                "MS-XLSX _xlfn.NETWORKDAYS.INTL; hand: weekend 7 = Fri+Sat, only Sun 17 works",
            ),
            (
                "=WORKDAY(DATE(2024,3,15),1)",
                N(45_369.0),
                "hand: Fri + 1 workday = Mon 18",
            ),
            (
                "=WORKDAY(\"x\",1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 WORKDAY; hand: not a date",
            ),
            (
                "=WORKDAY.INTL(DATE(2024,3,15),1,1)",
                N(45_369.0),
                "MS-XLSX _xlfn.WORKDAY.INTL; hand: Sat/Sun weekend → Mon 18",
            ),
            (
                "=WORKDAY.INTL(DATE(2024,3,15),1,7)",
                N(45_368.0),
                "MS-XLSX _xlfn.WORKDAY.INTL; hand: Fri/Sat weekend → Sun 17",
            ),
            (
                "=WORKDAY.INTL(DATE(2024,3,15),1,\"0000011\")",
                N(45_369.0),
                "MS-XLSX _xlfn.WORKDAY.INTL; hand: mask Mon..Sun, Sat+Sun off",
            ),
            (
                "=WORKDAY.INTL(DATE(2024,3,15),1,\"1111111\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.WORKDAY.INTL; hand: no working day",
            ),
            (
                "=WORKDAY.INTL(DATE(2024,3,15),1,\"abc\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.WORKDAY.INTL; hand: malformed mask",
            ),
            (
                "=YEARFRAC(DATE(2024,1,1),DATE(2024,1,1))",
                N(0.0),
                "hand: same date",
            ),
            (
                "=YEARFRAC(DATE(2024,1,1),DATE(2025,1,1),1)",
                N(1.0),
                "hand: one full actual year",
            ),
        ],
    );
}
