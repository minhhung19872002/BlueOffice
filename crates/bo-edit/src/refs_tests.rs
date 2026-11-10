//! Tables of figures, indexes, tables of authorities and note conversion. Expected
//! entries are worked out by hand from the page each paragraph is given and the rules
//! of the field switches (ECMA-376 Part 1, §17.16.5.29 INDEX, §17.16.5.67 TOA,
//! §17.16.5.68 TOC); Vietnamese order: `a ă â b c d đ e ê g h …`, tones ngang, huyền,
//! hỏi, ngã, sắc, nặng.
#![allow(clippy::unwrap_used)]

use super::*;
use crate::fields::{bookmark_xml, caption_paragraph, update, FieldEnv};
use bo_model::Run;

fn field(instr: &str) -> RunContent {
    RunContent::Field {
        instr: instr.into(),
        cached: String::new(),
        locked: false,
    }
}

/// A paragraph of `text` followed by fields.
fn marked(text: &str, instrs: &[&str]) -> Block {
    let mut runs = vec![Run::text(text, RunProps::default())];
    for i in instrs {
        runs.push(Run {
            props: RunProps::default(),
            content: vec![field(i)],
        });
    }
    Block::Paragraph(Paragraph::new(ParaProps::default(), runs))
}

fn doc_of(blocks: Vec<Block>) -> Document {
    Document {
        sections: vec![Section::new(blocks)],
        ..Document::default()
    }
}

/// Run `fields::update` with `pages[i]` = page of body paragraph `i`; the texts of the
/// generated paragraphs of the first block field, with their styles.
fn generated(doc: &Document, pages: &[u32], vi: bool) -> Vec<(String, String)> {
    let env = FieldEnv {
        vi,
        ..FieldEnv::default()
    };
    let up = update(doc, pages, &env);
    let blocks = &up.sections[0];
    let t = &fields::toc_ranges(blocks)[0];
    blocks[t.first..=t.last]
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|p| (p.props.style.clone().unwrap_or_default(), p.plain_text()))
        .collect()
}

fn texts(v: &[(String, String)]) -> Vec<&str> {
    v.iter().map(|(_, t)| t.as_str()).collect()
}

fn table(instr: &str) -> Block {
    Block::Paragraph(fields::new_block_field_paragraph(instr, None, 9000))
}

fn bm(start: bool, id: i64, name: &str) -> Run {
    let (s, e) = bookmark_xml(id, name);
    Run {
        props: RunProps::default(),
        content: vec![RunContent::Markup(if start { s } else { e })],
    }
}

/// Index entries on known pages: `pages[i]` is the page of body paragraph `i`.
fn index_doc(instr: &str, leader: Option<TabLeader>) -> (Document, Vec<u32>) {
    let mut range = marked("Chương", &["XE \"Hợp đồng\" \\r chuong"]);
    if let Block::Paragraph(p) = &mut range {
        p.runs.insert(0, bm(true, 1, "chuong"));
    }
    let mut end = marked("hết", &[]);
    if let Block::Paragraph(p) = &mut end {
        p.runs.push(bm(false, 1, "chuong"));
    }
    let blocks = vec![
        Block::Paragraph(fields::new_block_field_paragraph(instr, leader, 9000)),
        marked("Đà Nẵng", &["XE \"Đà Nẵng\""]),
        marked("Dừa", &["XE \"Dừa\""]),
        marked("Thủ đô", &["XE \"Hà Nội:Ba Đình\"", "XE \"Hà Nội\" \\b"]),
        marked(
            "Ăn",
            &["XE \"an toàn\"", "XE \"Ăn uống\"", "XE \"âm nhạc\" \\i"],
        ),
        marked("Phố", &["XE \"Hà Nội:Hoàn Kiếm\"", "XE \"Dừa\""]),
        range,
        marked("giữa", &[]),
        end,
        marked("Ký", &["XE \"Giao kết\" \\t \"Xem Hợp đồng\""]),
        marked("Mạng", &["XE \"3G\"", "XE \"Phụ lục\" \\f \"B\""]),
    ];
    (doc_of(blocks), vec![1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
}

#[test]
fn index_in_vietnamese_order_with_letter_headings() {
    let (d, pages) = index_doc(r#"INDEX \h "A" \z "1066""#, None);
    let out = generated(&d, &pages, false);
    assert_eq!(
        texts(&out),
        vec![
            "#",
            "3G, 10",
            "A",
            "an toàn, 4",
            "Ă",
            "Ăn uống, 4",
            "Â",
            "âm nhạc, 4",
            "D",
            "Dừa, 2, 5",
            "Đ",
            "Đà Nẵng, 1",
            "G",
            "Giao kết. Xem Hợp đồng",
            "H",
            "Hà Nội, 3",
            "Ba Đình, 3",
            "Hoàn Kiếm, 5",
            "Hợp đồng, 6\u{2013}8",
        ]
    );
    let style = |t: &str| out.iter().find(|o| o.1 == t).unwrap().0.as_str();
    assert_eq!(style("H"), "IndexHeading");
    assert_eq!(style("Hà Nội, 3"), "Index1");
    assert_eq!(style("Ba Đình, 3"), "Index2");
    // `\b` / `\i` format that entry's page number only.
    let env = FieldEnv::default();
    let up = update(&d, &pages, &env);
    let para = |t: &str| {
        up.sections[0]
            .iter()
            .filter_map(Block::as_paragraph)
            .find(|p| p.plain_text() == t)
            .unwrap()
            .clone()
    };
    let p = para("Hà Nội, 3");
    assert!(p
        .runs
        .iter()
        .any(|r| r.plain_text() == "3" && r.props.bold == Some(true)));
    let p = para("âm nhạc, 4");
    assert!(p
        .runs
        .iter()
        .any(|r| r.plain_text() == "4" && r.props.italic == Some(true)));
    // The index styles are added to the document.
    let ids: Vec<&str> = up
        .styles
        .as_ref()
        .unwrap()
        .styles
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert!(ids.contains(&"Index1") && ids.contains(&"Index2") && ids.contains(&"IndexHeading"));
}

#[test]
fn index_run_in_language_letters_types_and_bookmark() {
    let (d, pages) = index_doc(r#"INDEX \r \z "1066""#, None);
    let out = generated(&d, &pages, false);
    assert!(texts(&out).contains(&"Hà Nội, 3: Ba Đình, 3; Hoàn Kiếm, 5"));
    assert_eq!(out.len(), 9);
    // English order: letters without diacritics first, groups by base letter.
    let (d, pages) = index_doc(r#"INDEX \h "-A-" \z "1033""#, None);
    let out = generated(&d, &pages, true);
    assert_eq!(
        texts(&out)[..9],
        [
            "-#-",
            "3G, 10",
            "-A-",
            "âm nhạc, 4",
            "an toàn, 4",
            "Ăn uống, 4",
            "-D-",
            "Đà Nẵng, 1",
            "Dừa, 2, 5",
        ]
    );
    // Without `\z` the update's language decides (here Vietnamese).
    let (d, pages) = index_doc(r#"INDEX \p "d-g""#, None);
    assert_eq!(
        texts(&generated(&d, &pages, true)),
        vec!["Dừa, 2, 5", "Đà Nẵng, 1", "Giao kết. Xem Hợp đồng"]
    );
    let (d, pages) = index_doc(r#"INDEX \f "B""#, None);
    assert_eq!(texts(&generated(&d, &pages, true)), vec!["Phụ lục, 10"]);
    let (d, pages) = index_doc(r"INDEX \b chuong", None);
    assert_eq!(
        texts(&generated(&d, &pages, true)),
        vec!["Hợp đồng, 6\u{2013}8"]
    );
}

#[test]
fn index_right_aligned_page_numbers_keep_their_leader() {
    let (d, pages) = index_doc("INDEX \\e \"\t\" \\l \"; \"", Some(TabLeader::Hyphen));
    let env = FieldEnv {
        vi: true,
        ..FieldEnv::default()
    };
    let up = update(&d, &pages, &env);
    let p = up.sections[0]
        .iter()
        .filter_map(Block::as_paragraph)
        .find(|p| p.plain_text() == "Dừa\t2; 5")
        .unwrap();
    assert_eq!(
        p.props.tabs,
        Some(vec![TabStop {
            pos: Twips(fields::text_width(&d, 0)),
            align: TabAlign::Right,
            leader: TabLeader::Hyphen,
        }])
    );
    // Updating again changes nothing (the leader stays with the field).
    let d2 = Document {
        sections: vec![Section::new(up.sections[0].clone())],
        styles: up.styles.clone().unwrap(),
        ..Document::default()
    };
    // The same pages for the same paragraphs: the index grew to `grown` paragraphs.
    let total = body_paragraphs(&d2).len();
    let grown = total - 10;
    let pages2: Vec<u32> = (0..total)
        .map(|i| if i < grown { 1 } else { pages[i - grown + 1] })
        .collect();
    let up2 = update(&d2, &pages2, &env);
    assert_eq!(up2.sections[0], d2.sections[0].blocks);
}

/// Citations: the Civil Code on five pages, a judgment on two, a precedent and the
/// constitution on one each.
fn toa_doc(instr: &str) -> (Document, Vec<u32>) {
    let blocks = vec![
        table(instr),
        marked("a", &[r#"TA \l "Bộ luật Dân sự 2015" \s "BLDS" \c 2"#]),
        marked(
            "b",
            &[
                r#"TA \s "BLDS""#,
                r#"TA \l "Bản án 01/2020/DS-PT" \s "BA01" \c 1"#,
            ],
        ),
        marked(
            "c",
            &[r#"TA \s "BLDS""#, r#"TA \l "Án lệ 47/2021/AL" \c 1"#],
        ),
        marked("d", &[r#"TA \s "BLDS""#, r#"TA \s "BA01""#]),
        marked("e", &[r#"TA \s "BLDS""#]),
        marked("f", &[r#"TA \l "Hiến pháp 2013" \s "HP" \c 7 \b"#]),
    ];
    (doc_of(blocks), vec![1, 1, 2, 3, 4, 5, 6])
}

#[test]
fn table_of_authorities_by_category_with_passim() {
    let (d, pages) = toa_doc(r"TOA \h \p");
    let out = generated(&d, &pages, true);
    assert_eq!(
        texts(&out),
        vec![
            "Bản án và quyết định của tòa án",
            "Án lệ 47/2021/AL, 3",
            "Bản án 01/2020/DS-PT, 2, 4",
            "Luật và bộ luật",
            "Bộ luật Dân sự 2015, passim",
            "Hiến pháp",
            "Hiến pháp 2013, 6",
        ]
    );
    assert_eq!(out[0].0, "TOAHeading");
    assert_eq!(out[1].0, "TableofAuthorities");
    // Without `\p` every page is listed.
    let (d, pages) = toa_doc(r#"TOA \c "2" \h"#);
    assert_eq!(
        texts(&generated(&d, &pages, false)),
        vec!["Statutes and codes", "Bộ luật Dân sự 2015, 1, 2, 3, 4, 5"]
    );
    let (d, pages) = toa_doc(r#"TOA \c "7""#);
    assert_eq!(
        texts(&generated(&d, &pages, false)),
        vec!["Hiến pháp 2013, 6"]
    );
    assert_eq!(toa_category_name(9, true), "Nhóm 9");
}

fn caption(label: &str, n: i64, text: &str) -> Block {
    Block::Paragraph(caption_paragraph(label, n, text))
}

fn figures_doc(instr: &str) -> (Document, Vec<u32>) {
    let heading = Paragraph::new(
        ParaProps {
            outline_level: Some(0),
            ..ParaProps::default()
        },
        vec![Run::text("Chương 1", RunProps::default())],
    );
    let blocks = vec![
        marked("Mở đầu", &[]),
        table(instr),
        caption("Hình", 1, "Sơ đồ tổ chức"),
        caption("Bảng", 1, "Giá"),
        caption("Hình", 2, "Quy trình"),
        Block::Paragraph(heading),
    ];
    (doc_of(blocks), vec![1, 1, 2, 2, 4, 3])
}

#[test]
fn table_of_figures_with_and_without_labels() {
    let (d, pages) = figures_doc(r#"TOC \h \z \c "Hình""#);
    let out = generated(&d, &pages, true);
    assert_eq!(
        texts(&out),
        vec!["Hình 1: Sơ đồ tổ chức\t2", "Hình 2: Quy trình\t4"]
    );
    assert_eq!(out[0].0, "TableofFigures");
    // Entries link to the caption's `_Toc` bookmark (`\h`).
    let up = update(&d, &pages, &FieldEnv::default());
    let entry = up.sections[0][2].as_paragraph().unwrap();
    let link = entry
        .runs
        .iter()
        .find_map(|r| r.props.link.clone())
        .unwrap();
    assert!(link.starts_with("#_Toc"), "{link}");
    let target = up.sections[0]
        .iter()
        .filter_map(Block::as_paragraph)
        .find(|p| p.plain_text() == "Hình 2: Quy trình")
        .unwrap();
    assert!(fields::bookmark_marks(&para::flatten(target))
        .iter()
        .any(|(_, n, _)| n.as_deref() == Some(&link[1..])));

    let (d, pages) = figures_doc(r#"TOC \a "Hình""#);
    assert_eq!(
        texts(&generated(&d, &pages, true)),
        vec!["Sơ đồ tổ chức\t2", "Quy trình\t4"]
    );
    let (d, pages) = figures_doc(r#"TOC \c "Bảng" \n"#);
    assert_eq!(texts(&generated(&d, &pages, true)), vec!["Bảng 1: Giá"]);
    let (d, pages) = figures_doc(r#"TOC \c "Hình" \p " - ""#);
    assert_eq!(
        texts(&generated(&d, &pages, true))[0],
        "Hình 1: Sơ đồ tổ chức - 2"
    );
    // A table of contents still lists the heading only.
    let (d, pages) = figures_doc(r#"TOC \o "1-3""#);
    assert_eq!(texts(&generated(&d, &pages, true)), vec!["Chương 1\t3"]);
}

fn note(text: &str, style: &str, mark_style: &str) -> Vec<Block> {
    vec![Block::Paragraph(Paragraph::new(
        ParaProps {
            style: Some(style.into()),
            ..ParaProps::default()
        },
        vec![
            Run {
                props: RunProps {
                    style: Some(mark_style.into()),
                    ..RunProps::default()
                },
                content: vec![RunContent::NoteMark],
            },
            Run::text(format!(" {text}"), RunProps::default()),
        ],
    ))]
}

fn note_ref(text: &str, c: RunContent, style: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![
            Run::text(text, RunProps::default()),
            Run {
                props: RunProps {
                    style: Some(style.into()),
                    ..RunProps::default()
                },
                content: vec![c],
            },
        ],
    ))
}

fn notes_doc() -> Document {
    let mut d = doc_of(vec![
        note_ref("Một", RunContent::FootnoteRef(2), "FootnoteReference"),
        note_ref("Hai", RunContent::EndnoteRef(2), "EndnoteReference"),
        note_ref("Ba", RunContent::FootnoteRef(3), "FootnoteReference"),
    ]);
    d.footnotes
        .insert(2, note("Chú thích A", "FootnoteText", "FootnoteReference"));
    d.footnotes
        .insert(3, note("Chú thích B", "FootnoteText", "FootnoteReference"));
    d.endnotes
        .insert(2, note("Ghi chú C", "EndnoteText", "EndnoteReference"));
    for (id, kind) in [
        ("FootnoteText", bo_model::StyleKind::Paragraph),
        ("EndnoteText", bo_model::StyleKind::Paragraph),
        ("FootnoteReference", bo_model::StyleKind::Character),
        ("EndnoteReference", bo_model::StyleKind::Character),
    ] {
        d.styles.styles.push(bo_model::Style::new(id, id, kind));
    }
    d
}

fn note_list(s: &Session) -> Vec<(bool, i32, String)> {
    s.notes()
        .into_iter()
        .map(|n| (n.endnote, n.id, n.text))
        .collect()
}

#[test]
fn footnotes_become_endnotes_and_back() {
    let mut s = Session::new(notes_doc());
    let before = s.doc().clone();
    assert_eq!(
        s.convert_notes(NoteConversion::ToEndnotes, false).unwrap(),
        2
    );
    // New endnote ids after the existing ones; numbering follows the reference order.
    assert_eq!(
        note_list(&s),
        vec![
            (true, 3, "Chú thích A".to_owned()),
            (true, 2, "Ghi chú C".to_owned()),
            (true, 4, "Chú thích B".to_owned()),
        ]
    );
    assert!(s.doc().footnotes.is_empty());
    let moved = s.doc().endnotes[&3][0].as_paragraph().unwrap();
    assert_eq!(moved.props.style.as_deref(), Some("EndnoteText"));
    assert_eq!(
        moved.runs[0].props.style.as_deref(),
        Some("EndnoteReference")
    );
    let body = s.doc().sections[0].blocks[0].as_paragraph().unwrap();
    assert_eq!(
        body.runs[1].props.style.as_deref(),
        Some("EndnoteReference")
    );
    // One undo step.
    assert!(s.undo());
    assert_eq!(s.doc(), &before);

    assert_eq!(s.convert_notes(NoteConversion::Swap, false).unwrap(), 3);
    assert_eq!(
        note_list(&s),
        vec![
            (true, 3, "Chú thích A".to_owned()),
            (false, 4, "Ghi chú C".to_owned()),
            (true, 4, "Chú thích B".to_owned()),
        ]
    );
    assert!(s.undo());

    // Only the notes referenced in the selection.
    s.set_selection(Selection {
        anchor: Pos::new(2, 0),
        focus: Pos::new(2, s.para_text(2).unwrap().len()),
    });
    assert_eq!(
        s.convert_notes(NoteConversion::ToEndnotes, true).unwrap(),
        1
    );
    assert_eq!(
        note_list(&s),
        vec![
            (false, 2, "Chú thích A".to_owned()),
            (true, 2, "Ghi chú C".to_owned()),
            (true, 3, "Chú thích B".to_owned()),
        ]
    );
    assert_eq!(
        s.convert_notes(NoteConversion::ToFootnotes, true).unwrap(),
        1
    );
    // Back as a footnote, numbered after footnote 2.
    assert_eq!(note_list(&s)[2], (false, 3, "Chú thích B".to_owned()));
}

#[test]
fn converted_notes_without_styles_use_superscript() {
    let mut d = notes_doc();
    d.styles.styles.retain(|s| !s.id.starts_with("Endnote"));
    let mut s = Session::new(d);
    s.convert_notes(NoteConversion::ToEndnotes, false).unwrap();
    let body = s.doc().sections[0].blocks[0].as_paragraph().unwrap();
    assert_eq!(body.runs[1].props.style, None);
    assert_eq!(body.runs[1].props.vert_align, Some(VertAlign::Superscript));
    let moved = s.doc().endnotes[&3][0].as_paragraph().unwrap();
    assert_eq!(moved.props.style, None);
}

fn plain_doc(paras: &[&str]) -> Document {
    doc_of(paras.iter().map(|t| marked(t, &[])).collect())
}

fn instrs(s: &Session, pid: usize) -> Vec<String> {
    let path = body_paragraphs(s.doc())[pid].clone();
    fields::para_fields(&para::flatten(para_at(s.doc(), &path).unwrap()))
        .into_iter()
        .map(|f| f.0)
        .collect()
}

#[test]
fn mark_entries_and_citations() {
    let m = IndexMark {
        main: "Hợp đồng".into(),
        sub: "mua bán".into(),
        bold: true,
        ..IndexMark::default()
    };
    assert_eq!(m.instr().unwrap(), r#"XE "Hợp đồng:mua bán" \b"#);
    let see = IndexMark {
        main: "Giao kết".into(),
        cross_ref: "Xem Hợp đồng".into(),
        italic: true,
        ..IndexMark::default()
    };
    assert_eq!(
        see.instr().unwrap(),
        r#"XE "Giao kết" \t "Xem Hợp đồng" \i"#
    );
    let range = IndexMark {
        main: "Phạt".into(),
        bookmark: "dieu5".into(),
        ..IndexMark::default()
    };
    assert_eq!(range.instr().unwrap(), r#"XE "Phạt" \r dieu5"#);
    assert_eq!(IndexMark::default().instr(), None);

    let mut s = Session::new(plain_doc(&[
        "Hợp đồng mua bán",
        "Điều khoản của hợp đồng",
        "Hợp đồngX",
        "Thanh lý Hợp đồng và Hợp đồng",
    ]));
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(0, "Hợp đồng".len()),
    });
    let mark = IndexMark {
        main: "Hợp đồng".into(),
        ..IndexMark::default()
    };
    // Whole words, same case, the first one of each paragraph.
    assert_eq!(s.mark_index_entry(&mark, true).unwrap(), 2);
    assert_eq!(instrs(&s, 0), vec![r#"XE "Hợp đồng""#]);
    assert_eq!(instrs(&s, 1), Vec::<String>::new());
    assert_eq!(instrs(&s, 2), Vec::<String>::new());
    assert_eq!(instrs(&s, 3), vec![r#"XE "Hợp đồng""#]);
    assert_eq!(
        s.para_text(3).unwrap(),
        "Thanh lý Hợp đồng\u{FFFC} và Hợp đồng"
    );
    // Marking again skips the paragraphs already marked.
    assert_eq!(s.mark_index_entry(&mark, true).unwrap(), 1);
    assert!(s.undo() && s.undo());
    assert_eq!(instrs(&s, 3), Vec::<String>::new());

    let cite = CitationMark {
        long: "Bộ luật Dân sự 2015".into(),
        short: "BLDS".into(),
        category: 2,
    };
    assert_eq!(
        cite.instr().unwrap(),
        r#"TA \l "Bộ luật Dân sự 2015" \s "BLDS" \c 2"#
    );
    let mut s = Session::new(plain_doc(&[
        "Theo Bộ luật Dân sự 2015 thì",
        "BLDS quy định",
        "Không liên quan",
        "Bộ luật Dân sự 2015 cũng vậy",
    ]));
    let start = "Theo ".len();
    s.set_selection(Selection {
        anchor: Pos::new(0, start),
        focus: Pos::new(0, start + "Bộ luật Dân sự 2015".len()),
    });
    assert_eq!(s.mark_citation(&cite, true).unwrap(), 3);
    assert_eq!(instrs(&s, 0), vec![cite.instr().unwrap()]);
    assert_eq!(instrs(&s, 1), vec![r#"TA \s "BLDS""#]);
    assert_eq!(instrs(&s, 2), Vec::<String>::new());
    assert_eq!(instrs(&s, 3), vec![r#"TA \s "BLDS""#]);
}

#[test]
fn insert_index_in_two_columns_and_update() {
    let mut s = Session::new(doc_of(vec![
        marked("Giới thiệu", &["XE \"Bình\"", "XE \"An\""]),
        marked("Nội dung", &[]),
    ]));
    let len = s.para_text(1).unwrap().len();
    s.set_selection(Selection::caret(Pos::new(1, len)));
    let mut pages_of = |d: &Document| vec![1; body_paragraphs(d).len()];
    s.insert_reference_table(
        r#"INDEX \c "2" \z "1066""#,
        Some(TabLeader::Dot),
        2,
        &mut pages_of,
    )
    .unwrap();
    let d = s.doc();
    assert_eq!(d.sections.len(), 3);
    assert_eq!(d.sections[1].columns.count, 2);
    assert_eq!(d.sections[1].break_type, SectionBreak::Continuous);
    assert_eq!(d.sections[2].break_type, SectionBreak::Continuous);
    let lines: Vec<String> = d.sections[1]
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(Paragraph::plain_text)
        .collect();
    assert_eq!(lines, vec!["An, 1", "Bình, 1"]);
    // Updating again changes nothing.
    let before = s.doc().clone();
    s.update_fields(&mut pages_of).unwrap();
    assert_eq!(s.doc().sections, before.sections);
    // One undo step removes the index and its sections.
    assert!(s.undo());
    assert_eq!(s.doc().sections.len(), 1);
    assert!(s
        .insert_reference_table("PAGE", None, 1, &mut pages_of)
        .is_err());
}
