//! Table-driven tests for the number format engine.
//!
//! Expected strings follow the format-code semantics described in ECMA-376
//! Part 1, §18.8.31 (numFmts) and §18.17.4 (dates); the date serials were
//! computed by hand from the 1900 / 1904 date systems.

use super::*;
use bo_model::sheet::CellError;

fn en() -> FormatOptions {
    FormatOptions {
        locale: Locale::EN_US,
        date1904: false,
    }
}

fn vi() -> FormatOptions {
    FormatOptions {
        locale: Locale::VI_VN,
        date1904: false,
    }
}

fn check(cases: &[(&str, f64, &str)], opts: &FormatOptions) {
    let mut failures = Vec::new();
    for &(code, v, want) in cases {
        let got = format_number(v, code, opts);
        if got.text != want {
            failures.push(format!("{code:?} {v} → {:?}, want {want:?}", got.text));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn general() {
    check(
        &[
            ("General", 0.0, "0"),
            ("General", 1234.5, "1234.5"),
            ("General", -5.0, "-5"),
            ("General", 0.1 + 0.2, "0.3"),
            ("General", 1e20, "1E+20"),
            ("General", 123_456_789_012.0, "1.23457E+11"),
            ("General", 0.000_012_3, "1.23E-05"),
            ("General", 1.0 / 3.0, "0.333333333"),
            ("", 42.0, "42"),
        ],
        &en(),
    );
    check(
        &[("General", 1234.5, "1234,5"), ("General", -0.25, "-0,25")],
        &vi(),
    );
}

#[test]
fn digits_and_separators() {
    check(
        &[
            ("0", 1.5, "2"),
            ("0", 2.5, "3"),
            ("0", -2.5, "-3"),
            ("0.00", 1.23456, "1.23"),
            ("0.00", 0.005, "0.01"),
            ("00000", 123.0, "00123"),
            ("#", 0.0, ""),
            ("#.##", 0.5, ".5"),
            ("0.##", 1.5, "1.5"),
            ("0.##", 1.0, "1."),
            ("#,##0", 1_234_567.0, "1,234,567"),
            ("#,##0", 999.0, "999"),
            ("#,##0.00", -1234.5, "-1,234.50"),
            ("#,##0,", 12_345.0, "12"),
            ("#,##0,,", 1_234_567_890.0, "1,235"),
            ("0.0,", 12_345.0, "12.3"),
            ("000-00-0000", 123_456_789.0, "123-45-6789"),
        ],
        &en(),
    );
    // vi-VN: '.' groups thousands, ',' is the decimal separator.
    check(
        &[
            ("#,##0", 1_234_567.0, "1.234.567"),
            ("#,##0.00", 1234.5, "1.234,50"),
            ("0.000", -0.5, "-0,500"),
        ],
        &vi(),
    );
}

#[test]
fn percent_scientific_fraction() {
    check(
        &[
            ("0%", 0.256, "26%"),
            ("0.0%", 0.5, "50.0%"),
            ("0.00%", -0.012_34, "-1.23%"),
            ("0.00E+00", 12_345.0, "1.23E+04"),
            ("0.00E+00", 0.000_12, "1.20E-04"),
            ("0.0E-0", 12_345.0, "1.2E4"),
            ("##0.0E+0", 12_345.0, "12.3E+3"),
            ("0.00E+00", 0.0, "0.00E+00"),
            ("# ?/?", 1.5, "1 1/2"),
            ("# ?/4", 2.25, "2 1/4"),
            ("?/8", 0.5, "4/8"),
            ("# ??/??", 3.25, "3  1/4 "),
            ("0 ?/?", 2.0, "2    "),
        ],
        &en(),
    );
}

#[test]
fn sections_conditions_and_colors() {
    let code = "0;(0);\"zero\";\"txt \"@";
    check(
        &[(code, 5.0, "5"), (code, -5.0, "(5)"), (code, 0.0, "zero")],
        &en(),
    );
    assert_eq!(
        format_value(&CellValue::Text("a".into()), code, &en()).text,
        "txt a"
    );
    check(
        &[
            ("0;-0;;@", 0.0, ""),
            ("0;0", -5.0, "5"),
            ("[>100]\"big\";[<=100]\"small\"", 150.0, "big"),
            ("[>100]\"big\";[<=100]\"small\"", 50.0, "small"),
            ("[>=1000]#,##0,\"K\";0", 25_000.0, "25K"),
            ("[>=1000]#,##0,\"K\";0", 999.0, "999"),
            ("[<0]\"neg\";\"pos\"", -1.0, "neg"),
        ],
        &en(),
    );
    let red = format_number(3.0, "[Red]0;[Blue]-0", &en());
    assert_eq!((red.text.as_str(), red.color), ("3", Some(FmtColor::Red)));
    let blue = format_number(-3.0, "[Red]0;[Blue]-0", &en());
    assert_eq!(
        (blue.text.as_str(), blue.color),
        ("-3", Some(FmtColor::Blue))
    );
    let idx = format_number(1.0, "[Color10]0", &en());
    assert_eq!(idx.color, Some(FmtColor::Indexed(10)));
    assert_eq!(FmtColor::Red.rgb(), (255, 0, 0));
}

#[test]
fn literals_padding_fill() {
    check(
        &[
            ("0\" kg\"", 5.0, "5 kg"),
            ("\\$0", 5.0, "$5"),
            ("$#,##0.00", 1234.5, "$1,234.50"),
            ("0_)", 5.0, "5 "),
            ("(0)", 5.0, "(5)"),
        ],
        &en(),
    );
    let f = format_number(5.0, "*-0", &en());
    assert_eq!(f.text, "5");
    assert_eq!(f.fill, Some((0, '-')));
}

#[test]
fn text_section() {
    let t = CellValue::Text("abc".into());
    assert_eq!(format_value(&t, "@", &en()).text, "abc");
    assert_eq!(format_value(&t, "\"[\"@\"]\"", &en()).text, "[abc]");
    assert_eq!(format_value(&t, "0.00", &en()).text, "abc");
    assert_eq!(format_number(12.0, "@", &en()).text, "12");
    assert_eq!(
        format_value(&CellValue::Bool(true), "General", &en()).text,
        "TRUE"
    );
    assert_eq!(
        format_value(&CellValue::Error(CellError::Div0), "0.00", &en()).text,
        "#DIV/0!"
    );
    assert_eq!(format_value(&CellValue::Empty, "0.00", &en()).text, "");
}

#[test]
fn dates_and_times() {
    // Serial 45000 = Wednesday 2023-03-15 (1900 system).
    check(
        &[
            ("yyyy-mm-dd", 45_000.0, "2023-03-15"),
            ("d/m/yy", 45_000.0, "15/3/23"),
            ("dd/mm/yyyy", 45_000.0, "15/03/2023"),
            ("dddd", 45_000.0, "Wednesday"),
            ("ddd d mmm", 45_000.0, "Wed 15 Mar"),
            ("mmmm", 45_000.0, "March"),
            ("mmmmm", 45_000.0, "M"),
            ("bbbb", 45_000.0, "2566"),
            ("yyyy-mm-dd", 60.0, "1900-02-29"),
            ("yyyy-mm-dd", 61.0, "1900-03-01"),
            ("yyyy-mm-dd", 0.0, "1900-01-00"),
            ("yyyy-mm-dd", 1.0, "1900-01-01"),
            ("h:mm AM/PM", 0.5, "12:00 PM"),
            ("h:mm AM/PM", 0.25, "6:00 AM"),
            ("h A/P", 0.25, "6 A"),
            ("hh:mm:ss", 0.75, "18:00:00"),
            ("h:m", 0.5 + 5.0 / 1440.0, "12:5"),
            ("mm:ss", 0.5 + 90.0 / 86_400.0, "01:30"),
            ("[h]:mm", 1.5, "36:00"),
            ("[h]:mm:ss", 25.0 / 24.0, "25:00:00"),
            ("[mm]:ss", 1.0 / 1440.0, "01:00"),
            ("[ss]", 1.0 / 1440.0, "60"),
            ("h:mm:ss.00", 0.5 + 1.5 / 86_400.0, "12:00:01.50"),
            ("yyyy-mm-dd hh:mm", 45_000.999_999_9, "2023-03-16 00:00"),
        ],
        &en(),
    );
    check(
        &[
            ("dddd", 45_000.0, "Thứ Tư"),
            ("mmmm", 45_000.0, "Tháng Ba"),
            ("ddd", 45_000.0, "T4"),
            ("h:mm AM/PM", 0.75, "6:00 CH"),
        ],
        &vi(),
    );
    let d1904 = FormatOptions {
        locale: Locale::EN_US,
        date1904: true,
    };
    check(&[("yyyy-mm-dd", 0.0, "1904-01-01")], &d1904);
    assert!(format_number(-1.0, "yyyy-mm-dd", &en()).invalid);
    assert!(format_number(3e6, "yyyy", &en()).invalid);
    assert!(NumberFormat::parse("dd/mm/yyyy").is_date());
    assert!(NumberFormat::parse("[h]:mm").is_date());
    assert!(!NumberFormat::parse("0.00").is_date());
    assert!(!NumberFormat::parse("\"d\"0").is_date());
}

#[test]
fn locale_and_currency_tags() {
    // A locale tag switches names only (§18.8.31); separators stay the caller's.
    check(&[("[$-409]mmmm", 45_000.0, "March")], &vi());
    check(&[("[$-42A]dddd", 45_000.0, "Thứ Tư")], &en());
    check(
        &[
            ("#,##0 [$₫-42A]", 1_234_567.0, "1.234.567 ₫"),
            ("#,##0\\ \"₫\"", 2_500_000.0, "2.500.000 ₫"),
            ("[$-F800]", 45_000.0, "Thứ Tư, 15 Tháng Ba 2023"),
        ],
        &vi(),
    );
    check(
        &[
            ("[$$-409]#,##0.00", 1234.5, "$1,234.50"),
            ("[$€-x-euro2] #,##0", 10.0, "€ 10"),
            ("[$-F400]", 0.75, "6:00:00 PM"),
        ],
        &en(),
    );
}

#[test]
fn builtin_formats() {
    let vi = Locale::VI_VN;
    let en = Locale::EN_US;
    let table: &[(u32, &Locale, &str)] = &[
        (0, &en, "General"),
        (1, &en, "0"),
        (2, &en, "0.00"),
        (3, &en, "#,##0"),
        (4, &en, "#,##0.00"),
        (9, &en, "0%"),
        (10, &en, "0.00%"),
        (11, &en, "0.00E+00"),
        (12, &en, "# ?/?"),
        (13, &en, "# ??/??"),
        (14, &vi, "dd/mm/yyyy"),
        (14, &en, "m/d/yyyy"),
        (20, &en, "h:mm"),
        (21, &en, "h:mm:ss"),
        (46, &en, "[h]:mm:ss"),
        (49, &en, "@"),
    ];
    for &(id, loc, want) in table {
        assert_eq!(builtin_format(id, loc).as_deref(), Some(want), "id {id}");
    }
    assert_eq!(builtin_format(200, &en), None);
    let styles = bo_model::sheet::Stylesheet::default_styles();
    assert_eq!(format_code(&styles, 3, &en), "#,##0");
    assert_eq!(format_code(&styles, 999, &en), "General");
}

#[test]
fn malformed_codes_do_not_panic() {
    for code in [
        "[",
        "]",
        "\"",
        "\\",
        "_",
        "*",
        "[$",
        "[>",
        "0;;;;;;",
        "[Red",
        "E+",
        "0E",
        "?/",
        "/",
        "[h",
        "#,,,,,,,,,,,,",
        "0.0.0",
        "[$-ZZZZ]0",
        "[=x]0",
        "yyyy;;;@@@",
    ] {
        for v in [0.0, -1.0, 1e300, f64::NAN, f64::INFINITY, 0.5] {
            let _ = format_number(v, code, &en());
        }
        let _ = format_value(&CellValue::Text("t".into()), code, &en());
    }
}

/// `TEXT()` through the formula engine with this engine installed (gap #37):
/// `(formula, expected text or None for #VALUE!, source)`. Serial 45366 is
/// 2024-03-15, a Friday (ECMA-376 §18.17.4, 1900 system).
#[test]
fn text_function_uses_full_engine() {
    let cases: &[(&str, Option<&str>, &str)] = &[
        (
            "=TEXT(1234.567,\"#,##0.00\")",
            Some("1,234.57"),
            "§18.8.31 thousands + 2 dp",
        ),
        (
            "=TEXT(1234567,\"#,##0,\")",
            Some("1,235"),
            "§18.8.31 trailing comma scales by 1000",
        ),
        (
            "=TEXT(1234.5,\"$#,##0.00\")",
            Some("$1,234.50"),
            "§18.8.31 literal $",
        ),
        (
            "=TEXT(0.285,\"0.0%\")",
            Some("28.5%"),
            "§18.8.31 percent ×100",
        ),
        (
            "=TEXT(4.34,\"# ?/?\")",
            Some("4 1/3"),
            "§18.8.31 fraction, 0.34 ≈ 1/3",
        ),
        (
            "=TEXT(0.75,\"?/8\")",
            Some("6/8"),
            "§18.8.31 fixed denominator 8",
        ),
        (
            "=TEXT(12200000,\"0.00E+00\")",
            Some("1.22E+07"),
            "§18.8.31 scientific",
        ),
        ("=TEXT(5,\"000\")", Some("005"), "§18.8.31 zero padding"),
        (
            "=TEXT(1,\"0.0##\")",
            Some("1.0"),
            "§18.8.31 optional decimals",
        ),
        (
            "=TEXT(1234.5,\"0_);(0)\")",
            Some("1235 "),
            "§18.8.31 _x pads one space",
        ),
        (
            "=TEXT(-5,\"[Red]0;[Blue](0)\")",
            Some("(5)"),
            "§18.8.31 colours dropped, 2nd section",
        ),
        (
            "=TEXT(150,\"[>100]\"\"big\"\";\"\"small\"\"\")",
            Some("big"),
            "§18.8.31 condition",
        ),
        (
            "=TEXT(50,\"[>100]\"\"big\"\";\"\"small\"\"\")",
            Some("small"),
            "§18.8.31 condition, else",
        ),
        (
            "=TEXT(0,\"0;-0;\"\"zero\"\"\")",
            Some("zero"),
            "§18.8.31 zero section",
        ),
        (
            "=TEXT(1234.5,\"General\")",
            Some("1234.5"),
            "§18.8.31 General",
        ),
        (
            "=TEXT(45366,\"yyyy-mm-dd\")",
            Some("2024-03-15"),
            "§18.8.31 date codes",
        ),
        (
            "=TEXT(45366,\"dddd, mmmm d, yyyy\")",
            Some("Friday, March 15, 2024"),
            "§18.8.31 names",
        ),
        (
            "=TEXT(45366.75,\"dd/mm/yyyy hh:mm:ss\")",
            Some("15/03/2024 18:00:00"),
            "§18.8.31 0.75 day = 18:00",
        ),
        (
            "=TEXT(0.5,\"h:mm AM/PM\")",
            Some("12:00 PM"),
            "§18.8.31 AM/PM",
        ),
        (
            "=TEXT(1.5,\"[h]:mm\")",
            Some("36:00"),
            "§18.8.31 elapsed hours 1.5·24",
        ),
        (
            "=TEXT(45366,\"[$-42A]mmmm\")",
            Some("Tháng Ba"),
            "§18.8.31 vi-VN locale tag, March",
        ),
        (
            "=TEXT(45366,\"[$-42A]dddd\")",
            Some("Thứ Sáu"),
            "§18.8.31 vi-VN locale tag, Friday",
        ),
        (
            "=TEXT(1234567,\"#,##0 [$₫-42A]\")",
            Some("1,234,567 ₫"),
            "§18.8.31 currency tag",
        ),
        (
            "=TEXT(\"abc\",\"\"\"<\"\"@\"\">\"\"\")",
            Some("<abc>"),
            "§18.8.31 text section @",
        ),
        (
            "=TEXT(\"abc\",\"0.00\")",
            Some("abc"),
            "§18.8.31 no text section → text as is",
        ),
        (
            "=TEXT(\"abc\",\"0;0;0;\"\"x\"\"@\")",
            Some("xabc"),
            "§18.8.31 4th section",
        ),
        (
            "=TEXT(\"12\",\"0.0\")",
            Some("12.0"),
            "§18.17.7 TEXT numeric text converted",
        ),
        (
            "=TEXT(-1,\"yyyy\")",
            None,
            "§18.17.4 negative serial is not a date",
        ),
        ("=TEXT(1,\"\")", Some(""), "§18.17.7 TEXT empty code"),
    ];
    let mut fw = bo_formula::Workbook::with_sheets(&["Sheet1"]);
    fw.set_text_formatter(Some(text_function_format));
    for (i, (f, ..)) in cases.iter().enumerate() {
        fw.set_formula(0, u32::try_from(i).unwrap(), 0, f).unwrap();
    }
    fw.recalculate();
    let mut failures = Vec::new();
    for (i, (f, want, src)) in cases.iter().enumerate() {
        let got = fw.value(0, u32::try_from(i).unwrap(), 0);
        let want_v = match want {
            Some(t) => bo_formula::Value::Text((*t).to_owned()),
            None => bo_formula::Value::Error(bo_formula::ErrorKind::Value),
        };
        if got != want_v {
            failures.push(format!(
                "{f}: got {got:?}, want {want_v:?} [ECMA-376 {src}]"
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// The 1904 date system reaches the formatter (ECMA-376 §18.17.4: serial 0 = 1904-01-01).
#[test]
fn text_function_1904() {
    let f = |v: f64, code: &str| {
        text_function_format(
            bo_formula::FormatInput::Number(v),
            code,
            bo_formula::DateSystem::Date1904,
        )
    };
    assert_eq!(f(0.0, "yyyy-mm-dd").as_deref(), Some("1904-01-01"));
    assert_eq!(f(1.0, "d mmm yyyy").as_deref(), Some("2 Jan 1904"));
}

/// BlueSheet's own evaluation paths install the full engine: workbook
/// recalculation (`formula::load`) and model-side evaluation (`ModelCells`).
#[test]
fn bluesheet_paths_use_full_text_engine() {
    let wb = bo_model::sheet::Workbook::new();
    let mut fw = crate::formula::load(&wb, Some(45_366.0));
    // The compact fallback ignores scaling commas; the full engine applies them.
    fw.set_a1(0, "A1", "=TEXT(1234567,\"#,##0,\")").unwrap();
    fw.recalculate();
    assert_eq!(
        fw.value_a1(0, "A1"),
        bo_formula::Value::Text("1,235".into())
    );
    let cells = crate::cells::ModelCells::new(&wb, None);
    assert_eq!(
        cells.eval_at(0, "TEXT(45366,\"[$-42A]mmmm\")", (0, 0), 0, 0),
        bo_formula::Value::Text("Tháng Ba".into())
    );
}
