//! Automatic check of the Vietnamese administrative document format (Nghị định
//! 30/2020/NĐ-CP, Phụ lục I) on documents built by hand — not from the built-in
//! templates — and its fixes through the JSON bridge (`checkAdminFormat`,
//! `fixAdminFormat`, undo).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::templates::check::{check, fix, FormatReport};
use bo_engine::Engine;
use bo_model::{
    index, Alignment, Block, Border, Borders, Cell, CellProps, Document, HalfPoints, HeaderFooter,
    LineSpacing, PageSetup, ParaProps, Paragraph, Row, RowProps, Run, RunContent, RunProps,
    Section, Table, TableProps, Twips, Underline,
};
use serde_json::Value;

// ---- a hand-built conforming document -------------------------------------------------

fn rp(size: u32, bold: bool, italic: bool) -> RunProps {
    RunProps {
        size: Some(HalfPoints(size)),
        bold: Some(bold),
        italic: Some(italic),
        ..RunProps::default()
    }
}

fn p(text: &str, run: RunProps, align: Alignment) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(align),
            ..ParaProps::default()
        },
        vec![Run::text(text, run)],
    ))
}

fn empty() -> Block {
    Block::Paragraph(Paragraph::new(ParaProps::default(), Vec::new()))
}

/// An empty paragraph with a top border: a solid line.
fn line() -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Center),
            indent_left: Some(Twips(3000)),
            indent_right: Some(Twips(3000)),
            borders: Some(Borders {
                top: Some(Border::single(6)),
                ..Borders::default()
            }),
            ..ParaProps::default()
        },
        Vec::new(),
    ))
}

fn body(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Justify),
            indent_first_line: Some(Twips(709)),
            space_before: Some(Twips(60)),
            space_after: Some(Twips(60)),
            line_spacing: Some(LineSpacing::Exact(Twips(360))),
            ..ParaProps::default()
        },
        // The size comes from the document defaults (13 pt).
        vec![Run::text(text, RunProps::default())],
    ))
}

fn table(widths: &[i32], rows: Vec<Vec<Vec<Block>>>) -> Block {
    let nil = Border {
        style: "nil".into(),
        size: 0,
        color: None,
        space: 0,
    };
    Block::Table(Table {
        props: TableProps {
            borders: Some(Borders::all(&nil)),
            ..TableProps::default()
        },
        grid: widths.iter().copied().map(Twips).collect(),
        grid_change: None,
        rows: rows
            .into_iter()
            .map(|cells| Row {
                props: RowProps::default(),
                cells: cells
                    .into_iter()
                    .map(|blocks| Cell {
                        props: CellProps::default(),
                        blocks,
                    })
                    .collect(),
            })
            .collect(),
        src: None,
    })
}

const QH: &str = "CỘNG HOÀ XÃ HỘI CHỦ NGHĨA VIỆT NAM";
const TN: &str = "Độc lập – Tự do – Hạnh phúc";
const CQ: &str = "SỞ TƯ PHÁP";
const DATE: &str = "Bình Dương, ngày 12 tháng 3 năm 2025";
const TYPE: &str = "THÔNG BÁO";
const BODY1: &str = "Thực hiện kế hoạch công tác năm 2025, Sở Tư pháp thông báo như sau.";
const BODY2: &str = "Đề nghị các đơn vị cử đại biểu tham dự đầy đủ, đúng thành phần.";
const LIST: &str = "- Như trên;";
const TITLE: &str = "KT. GIÁM ĐỐC";
const NAME: &str = "Trần Văn Bình";

/// Header in a three-column table (spacer in the middle, number and date on a second
/// row), the motto underlined by its run, the issuing body by a bottom border, body text
/// at the default size, recipients and signer in a table, page number in the header.
fn conforming() -> Document {
    let coquan = Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Center),
            borders: Some(Borders {
                bottom: Some(Border::single(4)),
                ..Borders::default()
            }),
            ..ParaProps::default()
        },
        vec![Run::text(CQ, rp(26, true, false))],
    ));
    let motto = Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Center),
            ..ParaProps::default()
        },
        vec![Run::text(
            TN,
            RunProps {
                underline: Some(Underline::Single),
                ..rp(26, true, false)
            },
        )],
    ));
    let header = table(
        &[3600, 300, 5400],
        vec![
            vec![
                vec![
                    p(
                        "UBND TỈNH BÌNH DƯƠNG",
                        rp(26, false, false),
                        Alignment::Center,
                    ),
                    coquan,
                ],
                vec![empty()],
                vec![p(QH, rp(24, true, false), Alignment::Center), motto],
            ],
            vec![
                vec![p("Số: 125/TB-STP", rp(26, false, false), Alignment::Center)],
                vec![empty()],
                vec![p(DATE, rp(28, false, true), Alignment::Center)],
            ],
        ],
    );
    let signature = table(
        &[4300, 5000],
        vec![vec![
            vec![
                p("Nơi nhận:", rp(24, true, true), Alignment::Left),
                p(LIST, rp(22, false, false), Alignment::Left),
                p("- Lưu: VT, VP.", rp(22, false, false), Alignment::Left),
            ],
            vec![
                p(TITLE, rp(28, true, false), Alignment::Center),
                p("PHÓ GIÁM ĐỐC", rp(28, true, false), Alignment::Center),
                p("(Đã ký)", rp(28, false, true), Alignment::Center),
                empty(),
                p(NAME, rp(28, true, false), Alignment::Center),
            ],
        ]],
    );
    let blocks = vec![
        header,
        empty(),
        p(TYPE, rp(28, true, false), Alignment::Center),
        p(
            "Về việc tổ chức hội nghị tổng kết",
            rp(28, true, false),
            Alignment::Center,
        ),
        line(),
        body(BODY1),
        body(BODY2),
        signature,
    ];
    let number = Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Center),
            ..ParaProps::default()
        },
        vec![Run {
            props: rp(26, false, false),
            content: vec![RunContent::Field {
                instr: "PAGE".into(),
                cached: "2".into(),
                locked: false,
            }],
        }],
    ));
    Document {
        sections: vec![Section {
            page: PageSetup {
                width: Twips(11_906),
                height: Twips(16_838),
                margin_top: Twips(1247),    // 22 mm
                margin_bottom: Twips(1134), // 20 mm
                margin_left: Twips(1814),   // 32 mm
                margin_right: Twips(1021),  // 18 mm
                header: Twips(567),
                footer: Twips(567),
            },
            headers: HeaderFooter {
                default: Some(vec![number]),
                first: Some(vec![empty()]),
                even: None,
            },
            title_page: true,
            ..Section::new(blocks)
        }],
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..RunProps::default()
        },
        ..Document::default()
    }
}

fn para<'d>(d: &'d mut Document, text: &str) -> &'d mut Paragraph {
    let path = index::body_paragraphs(d)
        .into_iter()
        .find(|p| index::para_at(d, p).is_some_and(|x| x.plain_text() == text))
        .unwrap_or_else(|| panic!("no paragraph {text}"));
    index::para_at_mut(d, &path).unwrap()
}

fn runs<'d>(d: &'d mut Document, text: &str) -> impl Iterator<Item = &'d mut RunProps> {
    para(d, text).runs.iter_mut().map(|r| &mut r.props)
}

fn ids(r: &FormatReport) -> Vec<&str> {
    r.ids()
}

type Mutate = fn(&mut Document);

/// (name, change, expected issue, expected value, actual value, fixable).
const CASES: &[(&str, Mutate, &str, &str, &str, bool)] = &[
    (
        "top margin 15 mm",
        |d| d.sections[0].page.margin_top = Twips(850),
        "page.marginTop",
        "20–25 mm",
        "15.0 mm",
        true,
    ),
    (
        "bottom margin 30 mm",
        |d| d.sections[0].page.margin_bottom = Twips(1701),
        "page.marginBottom",
        "20–25 mm",
        "30.0 mm",
        true,
    ),
    (
        "left margin 25 mm",
        |d| d.sections[0].page.margin_left = Twips(1417),
        "page.marginLeft",
        "30–35 mm",
        "25.0 mm",
        true,
    ),
    (
        "right margin 25 mm",
        |d| d.sections[0].page.margin_right = Twips(1417),
        "page.marginRight",
        "15–20 mm",
        "25.0 mm",
        true,
    ),
    (
        "Letter paper",
        |d| {
            d.sections[0].page.width = Twips(12_240);
            d.sections[0].page.height = Twips(15_840);
        },
        "page.paper",
        "210 × 297 mm",
        "216 × 279 mm",
        true,
    ),
    (
        "Arial in the body",
        |d| runs(d, BODY1).for_each(|r| r.font = Some("Arial".into())),
        "font.font",
        "Times New Roman",
        "Arial",
        true,
    ),
    (
        "national title not bold",
        |d| runs(d, QH).for_each(|r| r.bold = Some(false)),
        "quocHieu.bold",
        "yes",
        "no",
        true,
    ),
    (
        "motto 12 pt",
        |d| runs(d, TN).for_each(|r| r.size = Some(HalfPoints(24))),
        "tieuNgu.size",
        "13–14 pt",
        "12 pt",
        true,
    ),
    (
        "motto without its line",
        |d| runs(d, TN).for_each(|r| r.underline = None),
        "tieuNgu.underline",
        "yes",
        "no",
        false,
    ),
    (
        "issuing body without its line",
        |d| para(d, CQ).props.borders = None,
        "coQuan.underline",
        "yes",
        "no",
        false,
    ),
    (
        "date upright",
        |d| runs(d, DATE).for_each(|r| r.italic = Some(false)),
        "diaDanh.italic",
        "yes",
        "no",
        true,
    ),
    (
        "date 12 pt",
        |d| runs(d, DATE).for_each(|r| r.size = Some(HalfPoints(24))),
        "diaDanh.size",
        "13–14 pt",
        "12 pt",
        true,
    ),
    (
        "type name left-aligned",
        |d| para(d, TYPE).props.align = Some(Alignment::Left),
        "tenLoai.align",
        "center",
        "left",
        true,
    ),
    (
        // Last change of the type name (the text no longer matches `TYPE`).
        "type name in lower case",
        |d| {
            for r in &mut para(d, TYPE).runs {
                r.content = vec![RunContent::Text("Thông báo".into())];
            }
        },
        "tenLoai.case",
        "upper",
        "mixed",
        true,
    ),
    (
        "body left-aligned",
        |d| para(d, BODY2).props.align = Some(Alignment::Left),
        "body.align",
        "justify",
        "left",
        true,
    ),
    (
        "body 12 pt",
        |d| runs(d, BODY1).for_each(|r| r.size = Some(HalfPoints(24))),
        "body.size",
        "13–14 pt",
        "12 pt",
        true,
    ),
    (
        "body double-spaced",
        |d| para(d, BODY1).props.line_spacing = Some(LineSpacing::Auto(480)),
        "body.lineSpacing",
        "1–1.5",
        "2.00",
        true,
    ),
    (
        "body without first-line indent",
        |d| para(d, BODY2).props.indent_first_line = None,
        "body.indent",
        "1–1.27 cm",
        "0.00 cm",
        true,
    ),
    (
        "body without paragraph spacing",
        |d| {
            let p = para(d, BODY1);
            p.props.space_before = None;
            p.props.space_after = None;
        },
        "body.spacing",
        "≥ 6 pt",
        "0 pt",
        true,
    ),
    (
        "recipients in 13 pt",
        |d| runs(d, LIST).for_each(|r| r.size = Some(HalfPoints(26))),
        "noiNhanList.size",
        "11 pt",
        "13 pt",
        true,
    ),
    (
        "signer title not bold",
        |d| runs(d, TITLE).for_each(|r| r.bold = Some(false)),
        "chucVu.bold",
        "yes",
        "no",
        true,
    ),
    (
        "signer name italic",
        |d| runs(d, NAME).for_each(|r| r.italic = Some(true)),
        "hoTen.italic",
        "no",
        "yes",
        true,
    ),
    (
        "page number on the first page",
        |d| d.sections[0].title_page = false,
        "soTrang.firstPage",
        "hidden",
        "shown",
        true,
    ),
    (
        "page number on the right",
        |d| {
            if let Some(Block::Paragraph(p)) = d.sections[0]
                .headers
                .default
                .as_mut()
                .and_then(|b| b.first_mut())
            {
                p.props.align = Some(Alignment::Right);
            }
        },
        "soTrang.align",
        "center",
        "right",
        true,
    ),
    (
        "page number in the footer",
        |d| {
            let s = &mut d.sections[0];
            s.footers.default = s.headers.default.take();
        },
        "soTrang.position",
        "header",
        "footer",
        false,
    ),
    (
        "number and symbol removed",
        |d| {
            if let Some(Block::Table(t)) = d.sections[0].blocks.first_mut() {
                t.rows[1].cells[0].blocks = vec![empty()];
            }
        },
        "soKyHieu.missing",
        "present",
        "missing",
        false,
    ),
];

#[test]
fn conforming_hand_built_document_has_no_finding() {
    let r = check(&conforming());
    assert!(r.issues.is_empty(), "{:#?}", r.issues);
    for c in [
        "quocHieu",
        "tieuNgu",
        "coQuanChuQuan",
        "coQuan",
        "soKyHieu",
        "diaDanh",
        "tenLoai",
        "trichYeu",
        "chucVu",
        "hoTen",
        "noiNhan",
        "noiNhanList",
        "soTrang",
    ] {
        assert!(r.found.iter().any(|f| f == c), "{c} not recognised");
    }
}

#[test]
fn each_wrong_attribute_gives_exactly_its_finding() {
    assert!(CASES.len() >= 10);
    for &(name, mutate, id, expected, actual, fixable) in CASES {
        let mut d = conforming();
        mutate(&mut d);
        let r = check(&d);
        assert_eq!(ids(&r), [id], "{name}: {:#?}", r.issues);
        let i = &r.issues[0];
        assert_eq!(
            (i.expected.as_str(), i.actual.as_str(), i.fixable),
            (expected, actual, fixable),
            "{name}"
        );
        // A paragraph finding points at its paragraph.
        let page_rule = id.starts_with("page.") || id.starts_with("soTrang.");
        assert_eq!(
            i.para.is_some(),
            !page_rule && !id.ends_with(".missing"),
            "{name}"
        );
        // A fixable finding is gone after the fix; the others stay.
        let mut f = d.clone();
        f.sections = fix(&d, None);
        let after = check(&f);
        if fixable {
            assert!(after.issues.is_empty(), "{name}: {:#?}", after.issues);
        } else {
            assert_eq!(ids(&after), [id], "{name}");
        }
    }
}

#[test]
fn stacked_layout_without_tables_is_recognised_and_misplaced() {
    // Header and signature as plain paragraphs: every component is found by its text;
    // the header ones are reported out of their columns.
    let mut d = conforming();
    let header_blocks = vec![
        p(
            "UBND TỈNH BÌNH DƯƠNG",
            rp(26, false, false),
            Alignment::Center,
        ),
        p(CQ, rp(26, true, false), Alignment::Center),
        line(),
        p("Số: 125/TB-STP", rp(26, false, false), Alignment::Center),
        p(QH, rp(24, true, false), Alignment::Center),
        p(TN, rp(26, true, false), Alignment::Center),
        line(),
        p(DATE, rp(28, false, true), Alignment::Center),
    ];
    let closing = vec![
        p(TITLE, rp(28, true, false), Alignment::Center),
        p("PHÓ GIÁM ĐỐC", rp(28, true, false), Alignment::Center),
        p("(Đã ký)", rp(28, false, true), Alignment::Center),
        empty(),
        p(NAME, rp(28, true, false), Alignment::Center),
        p("Nơi nhận:", rp(24, true, true), Alignment::Left),
        p(LIST, rp(22, false, false), Alignment::Left),
        p("- Lưu: VT, VP.", rp(22, false, false), Alignment::Left),
    ];
    let blocks = &mut d.sections[0].blocks;
    blocks.pop();
    blocks.extend(closing);
    blocks.splice(0..1, header_blocks);
    let r = check(&d);
    assert_eq!(
        ids(&r),
        [
            "coQuanChuQuan.position",
            "coQuan.position",
            "soKyHieu.position",
            "quocHieu.position",
            "tieuNgu.position",
            "diaDanh.position",
        ],
        "{:#?}",
        r.issues
    );
    assert!(r.issues.iter().all(|i| i.actual == "body" && !i.fixable));
    for c in ["chucVu", "hoTen", "noiNhan", "noiNhanList", "tenLoai"] {
        assert!(r.found.iter().any(|f| f == c), "{c} not recognised");
    }
}

#[test]
fn built_in_templates_have_no_finding_and_nothing_to_fix() {
    for id in bo_engine::templates::TEMPLATES {
        let d = bo_engine::templates::build(id).unwrap();
        let r = check(&d);
        assert!(r.issues.is_empty(), "{id}: {:#?}", r.issues);
        assert_eq!(fix(&d, None), d.sections, "{id}");
    }
}

// ---- through the bridge --------------------------------------------------------------

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn issue_ids(r: &Value) -> Vec<String> {
    r["adminFormat"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn fix_one_fix_all_and_undo() {
    // Every fixable deviation at once.
    let mut d = conforming();
    for &(_, mutate, _, _, _, fixable) in CASES {
        if fixable {
            mutate(&mut d);
        }
    }
    let original = d.sections.clone();
    let mut e = Engine::with_bundled_fonts();
    e.load(d, "Thông báo.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"checkAdminFormat"}"#);
    let before = issue_ids(&r);
    assert!(before.len() >= 15, "{before:?}");
    assert!(r["adminFormat"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["fixable"] == true));

    // Fix one issue (all its occurrences).
    let r = cmd(
        &mut e,
        r#"{"cmd":"fixAdminFormat","ids":["page.marginTop"]}"#,
    );
    let one = issue_ids(&r);
    assert!(!one.contains(&"page.marginTop".to_owned()));
    assert_eq!(one.len(), before.len() - 1, "{one:?}");
    assert!(r["format"]["canUndo"].as_bool().unwrap());

    // Fix the rest.
    let r = cmd(&mut e, r#"{"cmd":"fixAdminFormat"}"#);
    assert_eq!(issue_ids(&r), Vec::<String>::new());
    let fixed = e.document().unwrap();
    assert_eq!(fixed.sections[0].page.margin_top.0, 1134);
    assert!(fixed.sections[0].title_page);

    // Undo restores the deviations, one edit per fix.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(
        issue_ids(&cmd(&mut e, r#"{"cmd":"checkAdminFormat"}"#)),
        one
    );
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(
        issue_ids(&cmd(&mut e, r#"{"cmd":"checkAdminFormat"}"#)),
        before
    );
    assert_eq!(e.document().unwrap().sections, original);
    cmd(&mut e, r#"{"cmd":"redo"}"#);
    cmd(&mut e, r#"{"cmd":"redo"}"#);
    assert_eq!(
        issue_ids(&cmd(&mut e, r#"{"cmd":"checkAdminFormat"}"#)),
        Vec::<String>::new()
    );

    // The fixed document passes after saving and reopening.
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "Thông báo.docx").unwrap();
    let r = check(f.document().unwrap());
    assert!(r.issues.is_empty(), "{:#?}", r.issues);
}
