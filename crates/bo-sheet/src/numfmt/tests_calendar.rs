//! Locale / calendar tags and East Asian numerals (SHEET-NUMFMT-001).
//!
//! Sources of the expected values:
//! * Vietnamese lunar dates — public Vietnamese calendars / holidays: Tết Giáp Thìn
//!   2024-02-10, Tết Ất Tỵ 2025-01-29, Tết Bính Ngọ 2026-02-17, Tết Trung thu
//!   (15/8) 2024-09-17, the leap 2nd month (tháng 2 nhuận) of Quý Mão starting
//!   2023-03-22, and the well-known 1985 divergence (Tết in Vietnam 1985-01-21,
//!   Chinese New Year 1985-02-20). 2024-02-09 is the 30th of month 12 (month 12
//!   started on the 2024-01-11 new moon; 29 days later).
//! * Hijri: arithmetic calendar worked by hand (1 Ramadan 1445 = JDN 2460381 =
//!   2024-03-11; see the comment in the test).
//! * Thai Buddhist: year + 543.
//! * DBNum: CJK numeral conventions, see [`super::dbnum`].
//!
//! Serials: days since 1899-12-30 (§18.17.4.1, 1900 system after 1900-03-01).

use super::*;

fn serial(y: i32, m: u32, d: u32) -> f64 {
    (days_from_civil(y, m, d) - days_from_civil(1899, 12, 30)) as f64
}

fn opts(locale: &Locale) -> FormatOptions {
    FormatOptions {
        locale: *locale,
        date1904: false,
    }
}

fn check(cases: &[(f64, &str, &str)], locale: &Locale) {
    let mut failures = Vec::new();
    for &(v, code, want) in cases {
        let got = format_number(v, code, &opts(locale)).text;
        if got != want {
            failures.push(format!("{code:?} {v} → {got:?}, want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn serial_helper_matches_known_serials() {
    // 2024-01-01 is serial 45292 (19723 days after 1970-01-01 + 25569).
    assert_eq!(serial(2024, 1, 1), 45292.0);
    assert_eq!(serial(2024, 2, 10), 45332.0);
}

#[test]
fn vietnamese_lunar_calendar() {
    // `[$-0F042A]`: calendar 0x0F (lunar) for vi-VN → UTC+7 lunisolar calendar.
    check(
        &[
            (serial(2024, 2, 10), "[$-0F042A]d/m ggg", "1/1 Giáp Thìn"),
            (serial(2025, 1, 29), "[$-0F042A]d/m ggg", "1/1 Ất Tỵ"),
            (serial(2026, 2, 17), "[$-0F042A]d/m ggg", "1/1 Bính Ngọ"),
            (serial(2024, 2, 9), "[$-0F042A]d/m/yyyy", "30/12/2023"),
            (serial(2024, 9, 17), "[$-0F042A]dd/mm/yyyy", "15/08/2024"),
            (serial(2023, 3, 22), "[$-0F042A]d mmmm", "1 tháng Hai nhuận"),
            (serial(2023, 3, 22), "[$-0F042A]d mmm", "1 Hai nhuận"),
            (serial(2023, 2, 20), "[$-0F042A]d mmmm", "1 tháng Hai"),
            (serial(2024, 2, 10), "[$-0F042A]mmmm", "tháng Giêng"),
            (
                serial(1985, 1, 21),
                "[$-0F042A]d/m/yyyy ggg",
                "1/1/1985 Ất Sửu",
            ),
            // Weekday is the same in every calendar: 2024-02-10 was a Saturday.
            (serial(2024, 2, 10), "[$-0F042A]dddd d/m", "Thứ Bảy 1/1"),
        ],
        &Locale::VI_VN,
    );
}

#[test]
fn chinese_lunar_calendar_uses_utc8() {
    check(
        &[
            (serial(1985, 2, 20), "[$-0F0804]d/m/yyyy", "1/1/1985"),
            (serial(1985, 2, 20), "[$-0F0804]mmmm ggg", "正月 乙丑"),
            (serial(2024, 2, 10), "[$-0F0804]d/m ggg", "1/1 甲辰"),
            (serial(2023, 3, 22), "[$-0F0804]mmmm", "閏二月"),
        ],
        &Locale::EN_US,
    );
    // Tết 1985 in Vietnam is not new year in China.
    let cn = format_number(serial(1985, 1, 21), "[$-0F0804]d/m", &opts(&Locale::EN_US)).text;
    assert_ne!(cn, "1/1");
    // A tag without a language part follows the user's locale (vi → UTC+7).
    check(
        &[(serial(1985, 1, 21), "[$-F0000]d/m", "1/1")],
        &Locale::VI_VN,
    );
}

#[test]
fn hijri_and_thai_calendars() {
    // Hand calculation (Reingold & Dershowitz arithmetic calendar):
    // JDN(1445-09-01) = 1 + ⌈29.5·8⌉ + 1444·354 + ⌊(3 + 11·1445)/30⌋ + 1948440 − 1
    //                 = 1 + 236 + 511176 + 529 + 1948439 = 2460381 = 2024-03-11.
    check(
        &[
            (serial(2024, 3, 11), "B2d/m/yyyy", "1/9/1445"),
            // Sha'ban (month 8, even) has 29 days in the arithmetic calendar.
            (serial(2024, 3, 10), "B2d/m/yyyy", "29/8/1445"),
            (
                serial(2024, 3, 11),
                "[$-060401]d mmmm yyyy",
                "1 Ramadan 1445",
            ),
            (serial(2024, 3, 11), "B1d/m/yyyy", "11/3/2024"),
            (serial(2024, 2, 10), "[$-07041E]d/m/yyyy", "10/2/2567"),
            (serial(2024, 2, 10), "[$-01042A]d/m/yyyy", "10/2/2024"),
        ],
        &Locale::EN_US,
    );
}

#[test]
fn era_letters_outside_dates_are_literals() {
    check(&[(5.0, "0 g", "5 g")], &Locale::EN_US);
}

#[test]
fn locale_tag_with_calendar_keeps_names() {
    // The language part still selects month names (§18.8.31 locale tag).
    check(
        &[(
            serial(2024, 2, 10),
            "[$-010409]mmmm d, yyyy",
            "February 10, 2024",
        )],
        &Locale::VI_VN,
    );
}

#[test]
fn dbnum_numerals() {
    check(
        &[
            (1234.0, "[DBNum1]0", "一千二百三十四"),
            (10.0, "[DBNum1]0", "十"),
            (15.0, "[DBNum1]0", "十五"),
            (105.0, "[DBNum1]0", "一百零五"),
            (1005.0, "[DBNum1]0", "一千零五"),
            (1050.0, "[DBNum1]0", "一千零五十"),
            (10000.0, "[DBNum1]0", "一万"),
            (100_005.0, "[DBNum1]0", "十万零五"),
            (120_000.0, "[DBNum1]0", "十二万"),
            (100_000_000.0, "[DBNum1]0", "一亿"),
            (0.0, "[DBNum1]0", "〇"),
            (-12.0, "[DBNum1]0", "-十二"),
            (12345.0, "[DBNum1]#,##0", "一万二千三百四十五"),
            (12.5, "[DBNum1]0.00", "十二.五〇"),
            (1234.0, "[DBNum2]0", "壹仟贰佰叁拾肆"),
            (10.0, "[DBNum2]0", "壹拾"),
            (1234.0, "[$-404][DBNum2]0", "壹仟貳佰參拾肆"),
            (1005.0, "[$-411][DBNum1]0", "千五"),
            (1234.0, "[$-411][DBNum1]0", "千二百三十四"),
            (1234.0, "[DBNum3]0", "１２３４"),
            (2024.0, "[DBNum4]0", "二〇二四"),
            (
                serial(2024, 2, 10),
                "[DBNum1]yyyy\"年\"m\"月\"d\"日\"",
                "二〇二四年二月十日",
            ),
        ],
        &Locale::EN_US,
    );
}

#[test]
fn text_function_uses_the_same_formatter() {
    let got = text_function_format(
        bo_formula::FormatInput::Number(serial(2024, 2, 10)),
        "[$-0F042A]d/m ggg",
        bo_formula::DateSystem::Date1900,
    );
    assert_eq!(got.as_deref(), Some("1/1 Giáp Thìn"));
    let got = text_function_format(
        bo_formula::FormatInput::Number(1234.0),
        "[DBNum1]0",
        bo_formula::DateSystem::Date1900,
    );
    assert_eq!(got.as_deref(), Some("一千二百三十四"));
}
