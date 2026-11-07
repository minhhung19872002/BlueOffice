//! Property tests for the formula parser and formatter:
//! `parse(text) = e` ⇒ `parse(format(e)) = e` and `format` is idempotent, for
//! randomly generated formulas covering the ECMA-376 Part 1, §18.17.2 grammar
//! (constants, references in all forms, names, calls, operators, arrays,
//! structured references, LAMBDA invocation). Also: parsing arbitrary text and
//! formatting/offsetting/adjusting any parsed formula never panics.

// Property bodies take the generated value by value (TestRunner signature).
#![allow(clippy::needless_pass_by_value)]

use bo_formula::{
    adjust_for_change, format, format_with, offset_relative, parse, parse_with, ParseOptions,
    RefStyle, StructuralChange,
};
use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestRunner};

fn col_letters(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn cell() -> impl Strategy<Value = String> {
    (0u32..16_384, 1u32..=1_048_576, any::<bool>(), any::<bool>()).prop_map(|(c, r, ca, ra)| {
        format!(
            "{}{}{}{}",
            if ca { "$" } else { "" },
            col_letters(c),
            if ra { "$" } else { "" },
            r
        )
    })
}

fn sheet() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Sheet2!".to_owned()),
        Just("'My Sheet'!".to_owned()),
        Just("'It''s'!".to_owned()),
        Just("Sheet1:Sheet3!".to_owned()),
        Just("'Q 1:Q 4'!".to_owned()),
        Just("'Bảng tính'!".to_owned()),
    ]
}

fn reference() -> impl Strategy<Value = String> {
    let base = prop_oneof![
        3 => cell(),
        3 => (cell(), cell()).prop_map(|(a, b)| format!("{a}:{b}")),
        1 => (0u32..100, 0u32..100, any::<bool>()).prop_map(|(a, b, abs)| {
            let d = if abs { "$" } else { "" };
            format!("{d}{}:{d}{}", col_letters(a), col_letters(b))
        }),
        1 => (1u32..1000, 1u32..1000).prop_map(|(a, b)| format!("{a}:{b}")),
    ];
    (prop::option::of(sheet()), base).prop_map(|(s, r)| format!("{}{r}", s.unwrap_or_default()))
}

fn number() -> impl Strategy<Value = String> {
    prop_oneof![
        (0u32..100_000).prop_map(|n| n.to_string()),
        (0u32..1000, 0u32..1000).prop_map(|(a, b)| format!("{a}.{b}")),
        (1u32..10, -300i32..300).prop_map(|(m, e)| format!("{m}E{e}")),
        any::<f64>()
            .prop_filter("finite non-negative", |x| x.is_finite() && *x >= 0.0)
            .prop_map(|x| format!("{x:e}")),
    ]
}

fn text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 \"'ệ,;:()]{0,8}".prop_map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
}

fn atom() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => number(),
        2 => text(),
        1 => Just("TRUE".to_owned()),
        1 => Just("FALSE".to_owned()),
        1 => prop::sample::select(vec![
            "#NULL!", "#DIV/0!", "#VALUE!", "#REF!", "#NAME?", "#NUM!", "#N/A", "#SPILL!", "#CALC!",
        ])
        .prop_map(str::to_owned),
        4 => reference(),
        1 => prop::sample::select(vec!["Rate", "my_name", "Tax.Rate", "_x"]).prop_map(str::to_owned),
        1 => prop::sample::select(vec![
            "Table1[Amount]",
            "Table1[[#This Row],[Amount]]",
            "Table1[[#Headers],[A]:[B]]",
            "Table1[#All]",
            "[@Qty]",
            "Table1[]",
        ])
        .prop_map(str::to_owned),
        1 => prop::collection::vec(prop::collection::vec(prop_oneof![number(), text(), Just("TRUE".to_owned())], 2), 1..3)
            .prop_map(|rows| {
                let r: Vec<String> = rows.into_iter().map(|r| r.join(",")).collect();
                format!("{{{}}}", r.join(";"))
            }),
        1 => cell().prop_map(|c| format!("{c}#")),
    ]
}

fn formula() -> impl Strategy<Value = String> {
    atom().prop_recursive(5, 48, 4, |inner| {
        prop_oneof![
            (
                inner.clone(),
                prop::sample::select(vec![
                    "+", "-", "*", "/", "^", "&", "=", "<>", "<", "<=", ">", ">="
                ]),
                inner.clone()
            )
                .prop_map(|(a, op, b)| format!("{a}{op}{b}")),
            inner.clone().prop_map(|a| format!("-{a}")),
            inner.clone().prop_map(|a| format!("+{a}")),
            inner.clone().prop_map(|a| format!("{a}%")),
            inner.clone().prop_map(|a| format!("({a})")),
            (reference(), reference()).prop_map(|(a, b)| format!("({a},{b})")),
            (reference(), reference()).prop_map(|(a, b)| format!("{a} {b}")),
            reference().prop_map(|a| format!("@{a}")),
            (
                prop::sample::select(vec![
                    "SUM",
                    "IF",
                    "VLOOKUP",
                    "MAX",
                    "_xlfn.XLOOKUP",
                    "LET",
                    "CONCAT"
                ]),
                prop::collection::vec(
                    prop_oneof![4 => inner.clone(), 1 => Just(String::new())],
                    0..4
                )
            )
                .prop_map(|(f, args)| format!("{f}({})", args.join(","))),
            (inner.clone(), inner).prop_map(|(a, b)| format!("LAMBDA(x,x+{a})({b})")),
        ]
    })
}

/// Deterministic pseudo-random parameters derived from the generated text.
fn derived(src: &str, m1: u32, m2: u32) -> (u32, u32) {
    let h = src.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    (h % m1, (h >> 16) % m2)
}

/// Run a property over generated formulas on a thread with a large stack:
/// building values of the deeply recursive strategy needs more stack than the
/// default test thread has in debug builds.
fn run_property(cases: u32, test: fn(String) -> Result<(), TestCaseError>) {
    let handle = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let mut runner = TestRunner::new(ProptestConfig {
                cases,
                ..ProptestConfig::default()
            });
            runner.run(&formula(), test).map_err(|e| e.to_string())
        })
        .expect("spawn property thread");
    match handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => panic!("{e}"),
        Err(p) => std::panic::resume_unwind(p),
    }
}

fn round_trip(src: String) -> Result<(), TestCaseError> {
    let Ok(e) = parse(&src) else {
        // Generated text can be invalid (e.g. a range of whole rows and a cell); skip.
        return Ok(());
    };
    let text = format(&e);
    let again = parse(&text);
    prop_assert!(again.is_ok(), "{src} -> {text}: {:?}", again.err());
    let again = again.unwrap_or_else(|_| unreachable!());
    prop_assert_eq!(&again, &e, "{} -> {}", src, text);
    prop_assert_eq!(format(&again), text);
    Ok(())
}

fn r1c1_round_trip(src: String) -> Result<(), TestCaseError> {
    let (row, col) = derived(&src, 2000, 200);
    let Ok(e) = parse(&src) else {
        return Ok(());
    };
    let opts = ParseOptions {
        style: RefStyle::R1C1,
        origin_row: row,
        origin_col: col,
    };
    let text = format_with(&e, &opts);
    let again = parse_with(&text, &opts);
    prop_assert!(again.is_ok(), "{src} -> {text}: {:?}", again.err());
    prop_assert_eq!(again.unwrap_or_else(|_| unreachable!()), e);
    Ok(())
}

fn transforms(src: String) -> Result<(), TestCaseError> {
    let (a, b) = derived(&src, 50, 10);
    let (dr, dc) = (i64::from(a % 11) - 5, i64::from(b % 11) - 5);
    let (at, count) = (a, b % 4 + 1);
    if let Ok(e) = parse(&src) {
        let moved = offset_relative(&e, dr, dc);
        prop_assert!(parse(&format(&moved)).is_ok());
        for change in [
            StructuralChange::InsertRows { at, count },
            StructuralChange::DeleteRows { at, count },
            StructuralChange::InsertCols { at, count },
            StructuralChange::DeleteCols { at, count },
        ] {
            let adjusted = adjust_for_change(&e, change, "Sheet1", "Sheet1");
            prop_assert!(parse(&format(&adjusted)).is_ok());
        }
    }
    Ok(())
}

#[test]
fn parse_format_parse_round_trip() {
    run_property(2000, round_trip);
}

#[test]
fn r1c1_format_parse_round_trip() {
    run_property(1000, r1c1_round_trip);
}

#[test]
fn transforms_never_panic() {
    run_property(1000, transforms);
}

proptest! {
    #[test]
    fn arbitrary_text_never_panics(src in r"\PC{0,40}") {
        if let Ok(e) = parse(&src) {
            let _ = parse(&format(&e));
        }
    }
}
