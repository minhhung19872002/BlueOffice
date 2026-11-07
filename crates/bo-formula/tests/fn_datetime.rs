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
            ("=DATE(1900,1,1)", N(1.0)),
            ("=DATE(1900,2,28)", N(59.0)),
            ("=DATE(1900,2,29)", N(60.0)),
            ("=DATE(1900,3,1)", N(61.0)),
            ("=DATE(2008,1,1)", N(39_448.0)),
            ("=DATE(2024,3,15)", N(45_366.0)),
            ("=DATE(108,1,2)", N(39_449.0)),
            ("=DATE(2008,14,2)", N(39_846.0)),
            ("=DATE(2008,-3,2)", N(39_327.0)),
            ("=DATE(2008,1,35)", N(39_482.0)),
            ("=DATE(2008,1,-15)", N(39_432.0)),
            ("=DATE(9999,12,31)", N(2_958_465.0)),
            ("=DATE(-1,1,1)", E("#NUM!")),
            ("=DATE(10000,1,1)", E("#NUM!")),
            ("=DAY(60)", N(29.0)),
            ("=MONTH(60)", N(2.0)),
            ("=DAY(61)", N(1.0)),
            ("=YEAR(45366)", N(2024.0)),
            ("=MONTH(45366)", N(3.0)),
            ("=DAY(45366)", N(15.0)),
            ("=YEAR(\"2024-03-15\")", N(2024.0)),
            ("=YEAR(0)", N(1900.0)),
            ("=MONTH(0)", N(1.0)),
            ("=DAY(0)", N(0.0)),
            ("=YEAR(-1)", E("#NUM!")),
            ("=YEAR(\"abc\")", E("#VALUE!")),
            ("=DATEVALUE(\"8/22/2011\")", N(40_777.0)),
            ("=DATEVALUE(\"22-MAY-2011\")", N(40_685.0)),
            ("=DATEVALUE(\"2011/02/23\")", N(40_597.0)),
            ("=DATEVALUE(\"2024-03-15\")", N(45_366.0)),
            ("=DATEVALUE(\"15 March 2024\")", N(45_366.0)),
            ("=DATEVALUE(\"abc\")", E("#VALUE!")),
            ("=\"2024-03-15\"+1", N(45_367.0)),
        ],
    );
}

#[test]
fn time_parts() {
    check(
        &[],
        &[
            ("=TIME(12,0,0)", N(0.5)),
            ("=TIME(16,48,10)", Near(0.700_115_741, 1e-9)),
            ("=TIME(0,0,0)", N(0.0)),
            ("=TIME(27,0,0)", N(0.125)),
            ("=TIME(0,750,0)", Near(0.520_833_333, 1e-9)),
            ("=TIME(0,-1,0)", E("#NUM!")),
            ("=TIMEVALUE(\"2:24 AM\")", Near(0.1, 1e-12)),
            ("=TIMEVALUE(\"22-Aug-2011 6:35 AM\")", Near(0.274_305_556, 1e-9)),
            ("=TIMEVALUE(\"abc\")", E("#VALUE!")),
            ("=HOUR(0.75)", N(18.0)),
            ("=HOUR(\"7/18/2011 7:45\")", N(7.0)),
            ("=MINUTE(\"12:45:00\")", N(45.0)),
            ("=SECOND(\"4:48:18 PM\")", N(18.0)),
            ("=SECOND(\"4:48 PM\")", N(0.0)),
            ("=HOUR(-1)", E("#NUM!")),
            ("=TODAY()", N(45_366.0)),
            ("=NOW()", N(45_366.5)),
        ],
    );
}

#[test]
fn weekday_weeknum() {
    check(
        &[],
        &[
            // 2024-03-15 is a Friday.
            ("=WEEKDAY(45366)", N(6.0)),
            ("=WEEKDAY(45366,2)", N(5.0)),
            ("=WEEKDAY(45366,3)", N(4.0)),
            ("=WEEKDAY(45366,11)", N(5.0)),
            ("=WEEKDAY(45366,16)", N(7.0)),
            ("=WEEKDAY(45366,17)", N(6.0)),
            ("=WEEKDAY(45366,4)", E("#NUM!")),
            ("=WEEKNUM(DATE(2012,3,9))", N(10.0)),
            ("=WEEKNUM(DATE(2012,3,9),2)", N(11.0)),
            ("=WEEKNUM(DATE(2024,1,1))", N(1.0)),
            ("=WEEKNUM(DATE(2021,1,1),21)", N(53.0)),
            ("=WEEKNUM(45366,99)", E("#NUM!")),
            ("=ISOWEEKNUM(DATE(2012,3,9))", N(10.0)),
            ("=ISOWEEKNUM(DATE(2021,1,1))", N(53.0)),
            ("=ISOWEEKNUM(DATE(2024,12,30))", N(1.0)),
        ],
    );
}

#[test]
fn month_arithmetic_and_differences() {
    check(
        &[],
        &[
            ("=EDATE(DATE(2011,1,15),1)", N(40_589.0)),
            ("=EDATE(DATE(2011,1,15),-1)", N(40_527.0)),
            ("=EDATE(DATE(2011,1,15),2)", N(40_617.0)),
            ("=EDATE(DATE(2011,1,31),1)", N(40_602.0)),
            ("=EOMONTH(DATE(2011,1,1),1)", N(40_602.0)),
            ("=EOMONTH(DATE(2011,1,1),-3)", N(40_482.0)),
            ("=EOMONTH(DATE(2024,2,10),0)", N(45_351.0)),
            ("=DATEDIF(DATE(2001,1,1),DATE(2003,1,1),\"Y\")", N(2.0)),
            ("=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"D\")", N(440.0)),
            ("=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"YD\")", N(75.0)),
            ("=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"MD\")", N(14.0)),
            ("=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"M\")", N(14.0)),
            ("=DATEDIF(DATE(2001,6,1),DATE(2002,8,15),\"YM\")", N(2.0)),
            ("=DATEDIF(DATE(2003,1,1),DATE(2001,1,1),\"Y\")", E("#NUM!")),
            ("=DATEDIF(DATE(2001,1,1),DATE(2003,1,1),\"X\")", E("#NUM!")),
            ("=DAYS(DATE(2021,3,15),DATE(2021,2,1))", N(42.0)),
            ("=DAYS(\"15-MAR-2021\",\"1-FEB-2021\")", N(42.0)),
            ("=DAYS360(DATE(2011,1,30),DATE(2011,12,31))", N(330.0)),
            ("=DAYS360(DATE(2011,1,30),DATE(2011,12,31),TRUE)", N(330.0)),
            ("=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30))", Near(0.580_555_556, 1e-9)),
            ("=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),1)", Near(0.576_502_732, 1e-9)),
            ("=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),2)", Near(0.586_111_111, 1e-9)),
            ("=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),3)", Near(0.578_082_192, 1e-9)),
            ("=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),4)", Near(0.580_555_556, 1e-9)),
            ("=YEARFRAC(DATE(2012,1,1),DATE(2012,7,30),5)", E("#NUM!")),
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
            ("=NETWORKDAYS(DATE(2012,10,1),DATE(2013,3,1))", N(110.0)),
            ("=NETWORKDAYS(DATE(2012,10,1),DATE(2013,3,1),DATE(2012,11,22))", N(109.0)),
            (
                "=NETWORKDAYS(DATE(2012,10,1),DATE(2013,3,1),{\"2012-11-22\",\"2012-12-04\",\"2013-01-21\"})",
                N(107.0),
            ),
            ("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,1,31))", N(22.0)),
            (
                "=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),7,{\"2006/1/2\",\"2006/1/16\"})",
                N(22.0),
            ),
            ("=NETWORKDAYS.INTL(DATE(2006,2,28),DATE(2006,1,31))", N(-21.0)),
            (
                "=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),\"0010001\",{\"2006/1/2\",\"2006/1/16\"})",
                N(20.0),
            ),
            ("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),99)", E("#NUM!")),
            ("=WORKDAY(DATE(2008,10,1),151)", N(39_933.0)),
            ("=WORKDAY(DATE(2008,10,1),151,H1:H3)", N(39_938.0)),
            ("=WORKDAY(DATE(2024,3,15),-5)", N(45_359.0)),
            ("=WORKDAY(DATE(2024,3,15),0)", N(45_366.0)),
            ("=WORKDAY.INTL(DATE(2012,1,1),30,0)", E("#NUM!")),
            ("=WORKDAY.INTL(DATE(2012,1,1),90,11)", N(41_013.0)),
            ("=WORKDAY.INTL(DATE(2012,1,1),30,17)", N(40_944.0)),
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
            ("=DATE(1904,1,1)", N(0.0)),
            ("=DATE(1904,1,2)", N(1.0)),
            ("=DATE(2008,1,1)", N(37_986.0)),
            ("=DATE(1900,1,1)", E("#NUM!")),
            ("=YEAR(0)", N(1904.0)),
            ("=DAY(37986)", N(1.0)),
            ("=DATEVALUE(\"2008-01-01\")", N(37_986.0)),
            ("=WEEKDAY(37986)", N(3.0)),
            ("=TODAY()", N(43_904.0)),
            ("=TEXT(37986,\"yyyy-mm-dd\")", T("2008-01-01")),
        ],
        opts,
    );
}
