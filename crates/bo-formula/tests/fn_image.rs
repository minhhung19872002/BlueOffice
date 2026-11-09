//! `IMAGE(source, [alt_text], [sizing], [height], [width])` ([MS-XLSX] future
//! function `_xlfn.IMAGE`, result: §2.3.6.1.11 web image rich value).
//!
//! The engine never fetches: it returns a [`Value::Image`] request and the
//! argument errors of the published function definition (all `#VALUE!`).
#![allow(clippy::unwrap_used)]

mod common;
use common::*;

use bo_formula::{ImageValue, Value, Workbook};

fn eval(formula: &str) -> Value {
    let mut wb = Workbook::with_sheets(&["S"]);
    wb.set_a1(0, "A1", "https://example.vn/logo.png").unwrap();
    wb.set_a1(0, "A2", "Biểu trưng").unwrap();
    wb.set_a1(0, "B1", formula).unwrap();
    wb.recalculate();
    wb.value_a1(0, "B1")
}

fn image(source: &str, alt: Option<&str>, sizing: u8, h: Option<f64>, w: Option<f64>) -> Value {
    Value::Image(
        ImageValue {
            source: source.into(),
            alt_text: alt.map(str::to_owned),
            sizing,
            height: h,
            width: w,
        }
        .into(),
    )
}

#[test]
fn requests() {
    let url = "https://example.vn/logo.png";
    // MS-XLSX _xlfn.IMAGE: defaults are sizing 0 and no alternative text.
    assert_eq!(eval("=IMAGE(A1)"), image(url, None, 0, None, None));
    assert_eq!(
        eval("=IMAGE(A1,A2)"),
        image(url, Some("Biểu trưng"), 0, None, None)
    );
    assert_eq!(eval("=IMAGE(A1,,1)"), image(url, None, 1, None, None));
    assert_eq!(eval("=IMAGE(A1,\"\",2)"), image(url, None, 2, None, None));
    // Sizing 3 with only one dimension keeps it; the other follows the picture's ratio.
    assert_eq!(
        eval("=IMAGE(A1,,3,40)"),
        image(url, None, 3, Some(40.0), None)
    );
    assert_eq!(
        eval("=IMAGE(A1,,3,,25)"),
        image(url, None, 3, None, Some(25.0))
    );
    assert_eq!(
        eval("=IMAGE(A1,,3,40,25)"),
        image(url, None, 3, Some(40.0), Some(25.0))
    );
    // Sizing is truncated like other integer arguments: 2.9 → 2.
    assert_eq!(eval("=IMAGE(A1,,2.9)"), image(url, None, 2, None, None));
    // A dimension below 1 counts as missing.
    assert_eq!(
        eval("=IMAGE(A1,,3,0.5,25)"),
        image(url, None, 3, None, Some(25.0))
    );
}

#[test]
fn argument_errors() {
    check(
        &[("A1", "https://example.vn/logo.png"), ("A3", "7")],
        &[
            (
                "=IMAGE(\"\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: empty source",
            ),
            (
                "=IMAGE(\"http://example.vn/a.png\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: source must use https",
            ),
            (
                "=IMAGE(\"https://\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: no host",
            ),
            (
                "=IMAGE(A3)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: source is not text",
            ),
            (
                "=IMAGE(A1,5)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: alt_text is not text",
            ),
            (
                "=IMAGE(A1,,4)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: sizing outside 0–3",
            ),
            (
                "=IMAGE(A1,,-1)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: sizing outside 0–3",
            ),
            (
                "=IMAGE(A1,,0,40)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: height with sizing 0",
            ),
            (
                "=IMAGE(A1,,2,,10)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: width with sizing 2",
            ),
            (
                "=IMAGE(A1,,3)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: sizing 3 without a size",
            ),
            (
                "=IMAGE(A1,,3,0,0.5)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.IMAGE: sizing 3 with both sizes below 1",
            ),
            (
                "=IMAGE(1/0)",
                E("#DIV/0!"),
                "hand: argument errors propagate",
            ),
        ],
    );
}

#[test]
fn source_length_limit() {
    let long = format!("https://example.vn/{}", "a".repeat(255 - 19));
    assert_eq!(long.len(), 255);
    assert!(matches!(
        eval(&format!("=IMAGE(\"{long}\")")),
        Value::Image(_)
    ));
    let too_long = format!("{long}b");
    assert_eq!(
        eval(&format!("=IMAGE(\"{too_long}\")")),
        Value::Error(bo_formula::ErrorKind::Value)
    );
}

#[test]
fn in_calculations_a_picture_is_value_error() {
    // [MS-XLSX] §2.3.6.1: the rich value's fallback stands in for it.
    assert_eq!(
        eval("=IMAGE(A1)+1"),
        Value::Error(bo_formula::ErrorKind::Value)
    );
    assert_eq!(eval("=ISERROR(IMAGE(A1))"), Value::Bool(false));
}
