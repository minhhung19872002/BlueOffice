//! Property tests: model → xlsx → model is the identity, and arbitrary bytes never panic.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    Cell, CellError, CellRange, CellRef, CellValue, ColInfo, Comment, Font, Formula, FormulaKind,
    Hyperlink, RowInfo, SheetColor, TextRun, Workbook, Worksheet,
};
use bo_xlsx::{read_csv, read_xlsx, write_csv, write_xlsx, CsvOptions};
use proptest::prelude::*;

fn text() -> impl Strategy<Value = String> {
    // Printable text plus the control characters that need `_xHHHH_` escaping.
    proptest::string::string_regex("[\\PC\t\r\n\u{1}\u{1f}_x0-9A-F]{0,16}").unwrap()
}

fn font() -> impl Strategy<Value = Option<Font>> {
    proptest::option::of(
        (any::<bool>(), any::<bool>(), proptest::option::of(6u8..40)).prop_map(|(b, i, sz)| Font {
            bold: b,
            italic: i,
            size: sz.map(f64::from),
            color: Some(SheetColor::rgb("C00000")),
            name: Some("Arial".into()),
            ..Font::default()
        }),
    )
}

fn value() -> impl Strategy<Value = CellValue> {
    prop_oneof![
        Just(CellValue::Empty),
        any::<f64>()
            .prop_filter("finite", |v| v.is_finite())
            .prop_map(CellValue::Number),
        (-1e6f64..1e6).prop_map(CellValue::Number),
        text().prop_map(CellValue::Text),
        any::<bool>().prop_map(CellValue::Bool),
        prop_oneof![
            Just(CellError::Div0),
            Just(CellError::NA),
            Just(CellError::Value),
            Just(CellError::Ref),
            Just(CellError::Name),
            Just(CellError::Num),
            Just(CellError::Null),
        ]
        .prop_map(CellValue::Error),
        proptest::collection::vec(
            (text(), font()).prop_map(|(text, font)| TextRun { text, font }),
            1..4
        )
        .prop_map(CellValue::RichText),
    ]
}

fn formula() -> impl Strategy<Value = Option<Formula>> {
    proptest::option::of(prop_oneof![
        "[A-Z]{1,2}[1-9]\\*[0-9]{1,3}".prop_map(|t| Formula::normal(&t)),
        (0u32..4).prop_map(|i| Formula {
            text: String::new(),
            kind: FormulaKind::Shared {
                index: i,
                range: None
            },
            ..Formula::default()
        }),
        Just(Formula {
            text: "SUM(A1:A3)".into(),
            kind: FormulaKind::Array {
                range: "A1:A3".into()
            },
            calc_always: true,
            ..Formula::default()
        }),
    ])
}

fn cell() -> impl Strategy<Value = Cell> {
    (value(), formula(), 0u32..5).prop_map(|(mut value, formula, style)| {
        // A formula's cached text result is stored as plain text (`t="str"`).
        if formula.is_some() {
            if let CellValue::RichText(_) = value {
                value = CellValue::Text("r".into());
            }
        }
        Cell {
            value,
            formula,
            style,
            ..Cell::default()
        }
    })
}

fn sheet(idx: usize) -> impl Strategy<Value = Worksheet> {
    (
        proptest::collection::btree_map((0u32..200, 0u32..30), cell(), 0..40),
        proptest::collection::btree_map(
            0u32..200,
            (proptest::option::of(5u16..100), any::<bool>(), 0u8..3).prop_map(|(h, hidden, ol)| {
                RowInfo {
                    height: h.map(|h| f64::from(h) * 0.75),
                    custom_height: h.is_some(),
                    hidden,
                    outline_level: ol,
                    ..RowInfo::default()
                }
            }),
            0..5,
        ),
        proptest::option::of((1u16..400, any::<bool>())),
        proptest::collection::vec((0u32..50, 0u32..20, 1u32..3, 1u32..3), 0..3),
        proptest::collection::vec((0u32..50, 0u32..20, text()), 0..3),
    )
        .prop_map(move |(cells, rows, col, merges, comments)| {
            let mut ws = Worksheet::new(&format!("Trang {}", idx + 1));
            ws.sheet_id = idx as u32 + 1;
            ws.cells = cells;
            // Rows without any property are not stored by the reader.
            ws.rows = rows
                .into_iter()
                .filter(|(_, r)| *r != RowInfo::default())
                .collect();
            if let Some((w, hidden)) = col {
                ws.cols.push(ColInfo {
                    min: 2,
                    max: 4,
                    width: Some(f64::from(w) / 8.0),
                    custom_width: true,
                    hidden,
                    ..ColInfo::default()
                });
            }
            for (r, c, h, w) in merges {
                ws.merges.push(CellRange::new(
                    CellRef::new(r, c),
                    CellRef::new(r + h, c + w),
                ));
            }
            for (r, c, t) in comments {
                ws.comments.push(Comment {
                    cell: CellRef::new(r, c),
                    author: "Tác giả".into(),
                    text: if t.is_empty() {
                        Vec::new()
                    } else {
                        vec![TextRun {
                            text: t,
                            font: None,
                        }]
                    },
                });
            }
            ws.hyperlinks.push(Hyperlink {
                range: CellRange::single(CellRef::new(0, 0)),
                target: Some("https://example.vn/a b?x=1&y=2".into()),
                ..Hyperlink::default()
            });
            ws
        })
}

fn workbook() -> impl Strategy<Value = Workbook> {
    (proptest::collection::vec(any::<u8>(), 1..4), any::<bool>()).prop_flat_map(
        |(sheets, date1904)| {
            let strategies: Vec<_> = (0..sheets.len()).map(sheet).collect();
            strategies.prop_map(move |sheets| {
                let mut wb = Workbook::new();
                wb.sheets = sheets;
                wb.date1904 = date1904;
                wb
            })
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn model_roundtrip(wb in workbook()) {
        let bytes = write_xlsx(&wb).unwrap();
        let back = read_xlsx(&bytes).unwrap();
        for (a, b) in wb.sheets.iter().zip(&back.sheets) {
            for (k, c) in &a.cells {
                prop_assert_eq!(Some(c), b.cells.get(k), "cell {:?}", k);
            }
            prop_assert_eq!(&a.rows, &b.rows);
            prop_assert_eq!(&a.comments, &b.comments);
            prop_assert_eq!(&a.hyperlinks, &b.hyperlinks);
            prop_assert_eq!(a, b);
        }
        prop_assert_eq!(&back.styles, &wb.styles);
        prop_assert_eq!(&back, &wb);
    }

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let _ = read_xlsx(&bytes);
        let _ = read_csv(&bytes, &CsvOptions::default());
    }

    #[test]
    fn csv_text_roundtrip(rows in proptest::collection::vec(proptest::collection::vec("[a-zA-Zà-ỹ ,;\"\n]{1,8}", 1..4), 1..5)) {
        // Text-only CSV survives write → read for both dialects.
        for opts in [CsvOptions { detect_types: false, ..CsvOptions::default() }, CsvOptions { detect_types: false, ..CsvOptions::vi_vn() }] {
            let mut ws = Worksheet::new("Sheet1");
            for (r, row) in rows.iter().enumerate() {
                for (c, v) in row.iter().enumerate() {
                    ws.set_cell(r as u32, c as u32, Cell::new(CellValue::Text(v.clone())));
                }
            }
            let back = read_csv(&write_csv(&ws, &opts), &opts).unwrap();
            prop_assert_eq!(&back.cells, &ws.cells);
        }
    }
}
