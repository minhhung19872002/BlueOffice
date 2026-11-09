//! Round trip of text orientation / shrink-to-fit (ECMA-376 Part 1, §18.8.1
//! `alignment`) and of locale, calendar and numeral tags inside custom number
//! formats (§18.8.30 `numFmt`) — SHEET-NUMFMT-001.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_model::sheet as m;
use bo_opc::{Limits, Package};
use bo_xlsx::{read_xlsx, write_xlsx};

const CODES: [&str; 4] = [
    "[$-0F042A]d/m ggg",
    "[DBNum1][$-804]0",
    "_(* #,##0_);_(* \\(#,##0\\);_(* \"-\"_);_(@_)",
    "B2dd/mm/yyyy",
];

fn book() -> (m::Workbook, Vec<(u32, m::CellAlignment)>) {
    let mut wb = m::Workbook::new();
    let mut ss = m::Stylesheet::default_styles();
    let mut xfs = Vec::new();
    let aligns = [
        (Some(45), false, None),
        (Some(135), false, Some("center")),
        (Some(90), false, Some("top")),
        (Some(180), false, None),
        (Some(255), false, Some("center")),
        (None, true, None),
    ];
    for (i, (rot, shrink, vertical)) in aligns.into_iter().enumerate() {
        let al = m::CellAlignment {
            text_rotation: rot,
            shrink_to_fit: shrink,
            vertical: vertical.map(str::to_owned),
            ..m::CellAlignment::default()
        };
        let num_fmt_id = ss.add_num_fmt(CODES[i % CODES.len()]);
        let xf = ss.add_xf(m::CellXf {
            num_fmt_id,
            alignment: Some(al.clone()),
            ..m::CellXf::default()
        });
        xfs.push((xf, al));
    }
    wb.styles = ss;
    for (r, (xf, _)) in xfs.iter().enumerate() {
        wb.sheets[0].set_cell(
            r as u32,
            0,
            m::Cell::new(m::CellValue::Number(45332.0)).with_style(*xf),
        );
    }
    (wb, xfs)
}

#[test]
fn alignment_and_format_tags_round_trip() {
    let (wb, xfs) = book();
    let bytes = write_xlsx(&wb).unwrap();
    let back = read_xlsx(&bytes).unwrap();
    for (xf, al) in &xfs {
        let got = back.styles.xf(*xf).unwrap();
        assert_eq!(got.alignment.as_ref(), Some(al), "xf {xf}");
    }
    for code in CODES {
        assert!(
            back.styles.num_fmts.iter().any(|f| f.code == code),
            "format code {code:?} lost"
        );
    }
    // The attributes are written as §18.8.1 specifies.
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let styles = String::from_utf8(pkg.part("/xl/styles.xml").unwrap().data.clone()).unwrap();
    for attr in [
        r#"textRotation="45""#,
        r#"textRotation="135""#,
        r#"textRotation="255""#,
        r#"shrinkToFit="1""#,
    ] {
        assert!(styles.contains(attr), "{attr} missing in {styles}");
    }
    // Writing again is stable.
    assert_eq!(write_xlsx(&back).unwrap().len(), bytes.len());
}
