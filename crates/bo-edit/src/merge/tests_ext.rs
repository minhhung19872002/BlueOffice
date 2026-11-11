//! DOC-MERGE-001: rule fields, recipient list editing, envelopes and labels.
//! Expected texts and geometry are derived by hand from the templates and data below.

use super::recipients::{self, FilterOp, FilterRule, RecipientOptions, SortKey};
use super::rules::{self, Answers, MergeRule, MergeRun, RulePart};
use super::sheets::{self, LabelMode, LabelSheet};
use super::{placeholder, MergeData};
use crate::para::Atom;
use bo_model::{Block, Document, Paragraph, RunContent, RunProps, Section};

fn data() -> MergeData {
    let r = |a: &str, b: &str, c: &str, d: &str| vec![a.into(), b.into(), c.into(), d.into()];
    MergeData {
        name: "ds.csv".into(),
        fields: vec![
            "Họ tên".into(),
            "GioiTinh".into(),
            "SoTien".into(),
            "TinhThanh".into(),
        ],
        rows: vec![
            r("Nguyễn Văn An", "Nam", "1500000", "Hà Nội"),
            r("Trần Thị Bình", "Nữ", "800", "Đà Nẵng"),
            r("Lê Văn Cường", "Nam", "2500", "Hà Nội"),
            r("Phạm Thị Dung", "Nữ", "500", "Hà Nội"),
            r("Hoàng Văn Em", "Nam", "3000", "Hà Nội"),
        ],
    }
}

fn t(s: &str) -> Atom {
    (RunProps::default(), RunContent::Text(s.into()))
}

/// A simple field (§17.16.19 fldSimple).
fn f(instr: &str) -> Atom {
    (
        RunProps::default(),
        RunContent::Field {
            instr: instr.into(),
            cached: String::new(),
            locked: false,
            codes: false,
        },
    )
}

fn name() -> Atom {
    f("MERGEFIELD \"Họ tên\" \\* MERGEFORMAT")
}

/// A complex field `IF/SKIPIF/NEXTIF … {MERGEFIELD field} …`.
fn cx(parts: &[RulePart]) -> Vec<Atom> {
    rules::complex_field(parts, "", &RunProps::default())
}

fn txt(s: &str) -> RulePart {
    RulePart::Text(s.into())
}

fn fld(s: &str) -> RulePart {
    RulePart::Field(s.into())
}

fn doc(paras: Vec<Vec<Atom>>) -> Document {
    Document {
        sections: vec![Section::new(
            paras
                .into_iter()
                .map(|a| {
                    Block::Paragraph(Paragraph::new(
                        bo_model::ParaProps::default(),
                        crate::para::rebuild(a),
                    ))
                })
                .collect(),
        )],
        ..Document::default()
    }
}

fn texts(doc: &Document, run: MergeRun<'_>) -> Vec<String> {
    rules::merge_each(doc, run)
        .into_iter()
        .map(|m| m.doc.plain_text())
        .collect()
}

fn all(d: &MergeData) -> Vec<usize> {
    (0..d.rows.len()).collect()
}

#[test]
fn if_branches_with_nested_merge_fields() {
    let d = data();
    let order = all(&d);
    let answers = Answers::default();
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    // Kính gửi {IF {MERGEFIELD GioiTinh} = "Nam" "Ông" "Bà"} «Họ tên».
    let mut p = vec![t("Kính gửi ")];
    p.extend(cx(&[
        txt(" IF "),
        fld("GioiTinh"),
        txt(" = \"Nam\" \"Ông\" \"Bà\" \\* MERGEFORMAT "),
    ]));
    p.extend([t(" "), name(), t(".")]);
    let tpl = doc(vec![p]);
    assert_eq!(
        texts(&tpl, run),
        [
            "Kính gửi Ông Nguyễn Văn An.",
            "Kính gửi Bà Trần Thị Bình.",
            "Kính gửi Ông Lê Văn Cường.",
            "Kính gửi Bà Phạm Thị Dung.",
            "Kính gửi Ông Hoàng Văn Em.",
        ]
    );
    // Numbers compare as numbers; a SET bookmark is an operand; an empty field compares
    // with "" (the nested result is quoted, so the operand is not lost).
    let mut p = vec![f("SET Nguong \"1000\"")];
    p.extend(cx(&[
        txt(" IF "),
        fld("SoTien"),
        txt(" >= Nguong \"Lớn\" \"Nhỏ\" "),
    ]));
    p.push(t("|"));
    p.extend(cx(&[
        txt(" IF "),
        fld("Thiếu"),
        txt(" = \"\" \"trống\" \"có\" "),
    ]));
    p.push(t("|"));
    p.push(f("IF 5 > 3 \"đúng\" \"sai\""));
    p.push(t("|"));
    // The comparison text may hold the nested field inside quotes (\* Upper applies).
    p.extend(cx(&[
        txt(" IF \"x"),
        fld("GioiTinh"),
        txt("\" = \"xN*\" \"chữ N\" \"khác\" \\* Upper "),
    ]));
    let tpl = doc(vec![p]);
    assert_eq!(
        texts(&tpl, run)[..2],
        ["Lớn|trống|đúng|CHỮ N", "Nhỏ|trống|đúng|CHỮ N"]
    );
}

#[test]
fn skipif_drops_records_and_numbering_follows_the_filtered_list() {
    let d = data();
    let answers = Answers::default();
    // {SKIPIF {MERGEFIELD SoTien} < 1000}{MERGEREC}/{MERGESEQ} «Họ tên»
    let mut p = cx(&[txt(" SKIPIF "), fld("SoTien"), txt(" < 1000 ")]);
    p.extend([f("MERGEREC"), t("/"), f("MERGESEQ"), t(" "), name()]);
    let tpl = doc(vec![p]);
    let all_rows = all(&d);
    let run = MergeRun {
        data: &d,
        order: &all_rows,
        answers: &answers,
    };
    // Bình (800) and Dung (500) are skipped.
    assert_eq!(
        texts(&tpl, run),
        ["1/1 Nguyễn Văn An", "3/2 Lê Văn Cường", "5/3 Hoàng Văn Em"]
    );
    // Filter TinhThanh = Hà Nội → rows 0, 2, 3, 4; Dung (position 3) is skipped.
    let opts = RecipientOptions {
        filter: vec![FilterRule {
            field: "TinhThanh".into(),
            op: FilterOp::Eq,
            value: "Hà Nội".into(),
            or: false,
        }],
        ..RecipientOptions::default()
    };
    let order = recipients::active(&d, &opts);
    assert_eq!(order, [0, 2, 3, 4]);
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    assert_eq!(
        texts(&tpl, run),
        ["1/1 Nguyễn Văn An", "2/2 Lê Văn Cường", "4/3 Hoàng Văn Em"]
    );
    // Sorted by amount, descending: An 1500000, Em 3000, Cường 2500, Dung 500.
    let opts = RecipientOptions {
        sort: vec![SortKey {
            field: "SoTien".into(),
            descending: true,
        }],
        ..opts
    };
    let order = recipients::active(&d, &opts);
    assert_eq!(order, [0, 4, 2, 3]);
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    assert_eq!(
        texts(&tpl, run),
        ["1/1 Nguyễn Văn An", "2/2 Hoàng Văn Em", "3/3 Lê Văn Cường"]
    );
    // One merged document: three sections.
    let one = super::combine(&tpl, rules::merge_each(&tpl, run));
    assert_eq!(one.sections.len(), 3);
}

#[test]
fn nextif_and_next_share_a_document() {
    let d = data();
    let answers = Answers::default();
    // A: «Họ tên» | {NEXTIF {MERGEFIELD TinhThanh} = "Hà Nội"}B: «Họ tên»
    let mut p = vec![t("A: "), name(), t(" | ")];
    p.extend(cx(&[
        txt(" NEXTIF "),
        fld("TinhThanh"),
        txt(" = \"Hà Nội\" "),
    ]));
    p.extend([t("B: "), name()]);
    let tpl = doc(vec![p]);
    let order = all(&d);
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    let merged = rules::merge_each(&tpl, run);
    let got: Vec<String> = merged.iter().map(|m| m.doc.plain_text()).collect();
    assert_eq!(
        got,
        [
            "A: Nguyễn Văn An | B: Trần Thị Bình",
            "A: Lê Văn Cường | B: Phạm Thị Dung",
            "A: Hoàng Văn Em | B: ",
        ]
    );
    assert_eq!(merged[0].records, [0, 1]);
    assert_eq!(merged[2].records, [4]);
    // Without An: Bình (Đà Nẵng) does not move on.
    let order = [1, 2, 3, 4];
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    assert_eq!(
        texts(&tpl, run),
        [
            "A: Trần Thị Bình | B: Trần Thị Bình",
            "A: Lê Văn Cường | B: Phạm Thị Dung",
            "A: Hoàng Văn Em | B: ",
        ]
    );
}

/// Label texts of a label sheet document, row by row (spacer cells left out).
fn label_texts(doc: &Document, s: &LabelSheet) -> Vec<String> {
    let mut out = Vec::new();
    for sec in &doc.sections {
        for b in &sec.blocks {
            let Block::Table(tb) = b else { continue };
            for row in &tb.rows {
                if row.props.height.map(|h| h.0) != Some(sheets::mm(s.label_height)) {
                    continue;
                }
                for c in &row.cells {
                    if c.props.width == Some(bo_model::TableWidth::Dxa(sheets::mm(s.label_width))) {
                        let text: Vec<String> = c
                            .blocks
                            .iter()
                            .filter_map(|b| match b {
                                Block::Paragraph(p) => Some(p.plain_text()),
                                _ => None,
                            })
                            .collect();
                        out.push(text.join("/"));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn labels_one_per_record_and_same_label() {
    let d = data();
    let answers = Answers::default();
    let order = all(&d);
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    // A 2 × 2 sheet with gaps: 5 records → two pages, the second with one label.
    let s = LabelSheet {
        id: "custom".into(),
        page_width: 100.0,
        page_height: 60.0,
        rows: 2,
        cols: 2,
        top: 5.0,
        left: 5.0,
        label_width: 40.0,
        label_height: 20.0,
        h_pitch: 50.0,
        v_pitch: 25.0,
    };
    let tpl = sheets::labels_document(&s, "«Họ tên»\n«TinhThanh»", LabelMode::PerRecord).unwrap();
    let pages = rules::merge_each(&tpl, run);
    assert_eq!(pages.len(), 2);
    assert_eq!(
        label_texts(&pages[0].doc, &s),
        [
            "Nguyễn Văn An/Hà Nội",
            "Trần Thị Bình/Đà Nẵng",
            "Lê Văn Cường/Hà Nội",
            "Phạm Thị Dung/Hà Nội"
        ]
    );
    assert_eq!(
        label_texts(&pages[1].doc, &s),
        ["Hoàng Văn Em/Hà Nội", "/", "/", "/"]
    );
    // Same label on a full page: one page per record.
    let tpl = sheets::labels_document(&s, "«Họ tên»", LabelMode::Same).unwrap();
    let pages = rules::merge_each(&tpl, run);
    assert_eq!(pages.len(), 5);
    assert_eq!(label_texts(&pages[1].doc, &s), ["Trần Thị Bình"; 4]);
    // Placeholders before merging.
    let shown = rules::preview_run(&tpl, run, None);
    assert_eq!(label_texts(&shown, &s)[0], placeholder("Họ tên"));
}

#[test]
fn label_sheet_geometry() {
    let presets = sheets::label_presets();
    for p in &presets {
        p.validate().unwrap();
    }
    let s = presets.iter().find(|p| p.id == "a4-3x8").unwrap();
    // x = 7.25 + 1 × 66, y = 12.9 + 2 × 33.9.
    let (x, y, w, h) = s.rect(2, 1);
    assert!((x - 73.25).abs() < 1e-9 && (y - 80.7).abs() < 1e-9);
    assert!((w - 63.5).abs() < 1e-9 && (h - 33.9).abs() < 1e-9);
    // Last label: 7.25 + 132 = 139.25, 12.9 + 7 × 33.9 = 250.2.
    let (x, y, _, _) = s.rect(7, 2);
    assert!((x - 139.25).abs() < 1e-9 && (y - 250.2).abs() < 1e-9);
    let d = sheets::labels_document(s, "«Họ tên»", LabelMode::PerRecord).unwrap();
    let sec = &d.sections[0];
    // 210 mm = 11906 tw; 297 mm = 16838 tw; 7.25 mm = 411 tw; 12.9 mm = 731 tw.
    assert_eq!(sec.page.width.0, 11_906);
    assert_eq!(sec.page.height.0, 16_838);
    assert_eq!(sec.page.margin_left.0, 411);
    assert_eq!(sec.page.margin_top.0, 731);
    let Block::Table(tb) = &sec.blocks[0] else {
        panic!("label table")
    };
    // 63.5 mm = 3600 tw, gap 2.5 mm = 142 tw; table 3 × 3600 + 2 × 142 = 11084 tw.
    let grid: Vec<i32> = tb.grid.iter().map(|g| g.0).collect();
    assert_eq!(grid, [3600, 142, 3600, 142, 3600]);
    assert_eq!(sec.page.margin_right.0, 11_906 - 411 - 11_084);
    // No vertical gap: 8 rows of 33.9 mm = 1922 tw, exact.
    assert_eq!(tb.rows.len(), 8);
    assert_eq!(
        tb.rows[0].props.height,
        Some((bo_model::Twips(1922), bo_model::HeightRule::Exact))
    );
    // Every label after the first starts with NEXT (24 − 1).
    let nexts = tb
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .flat_map(|c| &c.blocks)
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p),
            _ => None,
        })
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.content)
        .filter(|c| matches!(c, RunContent::Field { instr, .. } if instr == "NEXT"))
        .count();
    assert_eq!(nexts, 23);
    // Labels that do not fit are refused.
    let bad = LabelSheet {
        rows: 9,
        ..s.clone()
    };
    assert!(bad.validate().is_err());
}

#[test]
fn envelope_places_the_delivery_address() {
    let dl = sheets::envelope_presets()
        .into_iter()
        .find(|e| e.id == "dl")
        .unwrap();
    // Default position: 40 % of 220 = 88 mm across, 45 % of 110 = 49.5 mm down.
    assert_eq!(sheets::delivery_default(&dl), (88.0, 49.5));
    let d = sheets::envelope_document(
        &dl,
        "«Họ tên»\n«TinhThanh»",
        "Công ty BlueStar\nHà Nội",
        None,
    )
    .unwrap();
    let sec = &d.sections[0];
    // 220 mm = 12472 tw, 110 mm = 6236 tw, margins 10 mm = 567 tw.
    assert_eq!((sec.page.width.0, sec.page.height.0), (12_472, 6_236));
    assert_eq!(sec.page.margin_left.0, 567);
    let Some(Block::Table(tb)) = sec.blocks.get(2) else {
        panic!("address table")
    };
    let fl = tb.props.float.as_ref().unwrap();
    // 88 mm = 4989 tw, 49.5 mm = 2806 tw; width 220 − 88 − 10 = 122 mm = 6917 tw.
    assert_eq!((fl.x.0, fl.y.0), (4989, 2806));
    assert_eq!(fl.horz_anchor.as_deref(), Some("page"));
    assert_eq!(tb.grid[0].0, 6917);
    let d2 = data();
    let order = all(&d2);
    let answers = Answers::default();
    let run = MergeRun {
        data: &d2,
        order: &order,
        answers: &answers,
    };
    let env = rules::merge_each(&d, run);
    assert_eq!(env.len(), 5);
    assert_eq!(
        env[1].doc.plain_text(),
        // Cell paragraphs join with a space in `plain_text`.
        "Công ty BlueStar\nHà Nội\nTrần Thị Bình Đà Nẵng\n"
    );
    assert!(sheets::envelope_document(&dl, "x", "", Some((215.0, 20.0))).is_err());
}

#[test]
fn ask_fillin_answers_once_or_per_record() {
    let d = data();
    let tpl = doc(vec![vec![
        f("ASK NgayHop \"Ngày họp?\" \\d \"01/12/2026\" \\o"),
        t("Họp ngày "),
        f("REF NgayHop"),
        t(", ghi chú: "),
        f("FILLIN \"Ghi chú cho người nhận?\""),
    ]]);
    let ps = rules::prompts(&tpl);
    assert_eq!(ps.len(), 2);
    assert_eq!(ps[0].key, "ASK:NgayHop");
    assert_eq!(ps[0].default, "01/12/2026");
    assert!(ps[0].once);
    assert_eq!(ps[1].key, "FILLIN:Ghi chú cho người nhận?");
    assert!(!ps[1].once);
    let order = [0, 1, 2];
    let none = Answers::default();
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &none,
    };
    assert_eq!(texts(&tpl, run)[0], "Họp ngày 01/12/2026, ghi chú: ");
    let mut answers = Answers::default();
    answers
        .0
        .insert("ASK:NgayHop".into(), vec!["05/12/2026".into()]);
    answers.0.insert(
        "FILLIN:Ghi chú cho người nhận?".into(),
        vec!["a".into(), "b".into(), "c".into()],
    );
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    assert_eq!(
        texts(&tpl, run),
        [
            "Họp ngày 05/12/2026, ghi chú: a",
            "Họp ngày 05/12/2026, ghi chú: b",
            "Họp ngày 05/12/2026, ghi chú: c"
        ]
    );
}

#[test]
fn inserted_rules_merge() {
    let d = data();
    let order = all(&d);
    let answers = Answers::default();
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    let rule = MergeRule::If {
        field: "GioiTinh".into(),
        op: "=".into(),
        value: "Nữ".into(),
        then: "Kính gửi bà".into(),
        otherwise: "Kính gửi ông".into(),
    };
    let (parts, shown) = rule.parts().unwrap();
    assert_eq!(shown, "Kính gửi bà");
    let mut p = rules::complex_field(&parts, &shown, &RunProps::default());
    p.extend([t(" "), name()]);
    let tpl = doc(vec![p]);
    // Before merging the template shows the rule's own result only (nested fields
    // inside the instruction show nothing).
    assert_eq!(tpl.plain_text(), "Kính gửi bà ");
    let got = texts(&tpl, run);
    assert_eq!(got[0], "Kính gửi ông Nguyễn Văn An");
    assert_eq!(got[1], "Kính gửi bà Trần Thị Bình");
    // The preview keeps the field and shows the record's branch.
    let pv = rules::preview_run(&tpl, run, Some(3));
    assert_eq!(pv.plain_text(), "Kính gửi bà Phạm Thị Dung");
    assert!(MergeRule::If {
        field: "x".into(),
        op: "~".into(),
        value: String::new(),
        then: String::new(),
        otherwise: String::new()
    }
    .parts()
    .is_none());
    // JSON form used by the command bridge.
    let r: MergeRule =
        serde_json::from_str(r#"{"kind":"skipIf","field":"SoTien","op":"<","value":"1000"}"#)
            .unwrap();
    assert!(matches!(r, MergeRule::SkipIf { .. }));
}

#[test]
fn unclosed_fields_stay_as_they_are() {
    let d = data();
    let order = all(&d);
    let answers = Answers::default();
    let run = MergeRun {
        data: &d,
        order: &order,
        answers: &answers,
    };
    // A field that ends in a later paragraph (TOC style) is left alone; the merge field
    // after it is still filled.
    let mut p = rules::complex_field(&[txt(" TOC \\o ")], "", &RunProps::default());
    p.pop(); // no end mark in this paragraph
    p.push(name());
    let tpl = doc(vec![p]);
    let m = rules::merge_each(&tpl, run);
    assert_eq!(m[0].doc.plain_text(), "Nguyễn Văn An");
}

#[test]
fn filter_sort_exclude_duplicates() {
    let d = data();
    let rule = |field: &str, op, value: &str, or| FilterRule {
        field: field.into(),
        op,
        value: value.into(),
        or,
    };
    // (Hà Nội AND ≥ 1000) OR (Nữ AND < 600).
    let opts = RecipientOptions {
        filter: vec![
            rule("TinhThanh", FilterOp::Eq, "hà nội", false),
            rule("SoTien", FilterOp::Ge, "1000", false),
            rule("GioiTinh", FilterOp::Eq, "Nữ", true),
            rule("SoTien", FilterOp::Lt, "600", false),
        ],
        ..RecipientOptions::default()
    };
    assert_eq!(recipients::active(&d, &opts), [0, 2, 3, 4]);
    let contains = RecipientOptions {
        filter: vec![rule("Họ tên", FilterOp::Contains, "văn", false)],
        ..RecipientOptions::default()
    };
    assert_eq!(recipients::active(&d, &contains), [0, 2, 4]);
    let not_empty = RecipientOptions {
        filter: vec![rule("Thiếu", FilterOp::NotEmpty, "", false)],
        ..RecipientOptions::default()
    };
    assert_eq!(recipients::active(&d, &not_empty), Vec::<usize>::new());
    // Province ascending (Đà Nẵng before Hà Nội in Vietnamese order), then name
    // descending (Phạm > Nguyễn > Lê > Hoàng).
    let key = |f: &str, descending| SortKey {
        field: f.into(),
        descending,
    };
    let sorted = RecipientOptions {
        sort: vec![key("TinhThanh", false), key("Họ tên", true)],
        ..RecipientOptions::default()
    };
    assert_eq!(recipients::active(&d, &sorted), [1, 3, 0, 2, 4]);
    let excluded = RecipientOptions {
        excluded: vec![3],
        ..sorted
    };
    assert_eq!(recipients::active(&d, &excluded), [1, 0, 2, 4]);
    // Amounts sort as numbers: 500, 800, 2500, 3000, 1500000.
    let by_amount = RecipientOptions {
        sort: vec![key("SoTien", false)],
        ..RecipientOptions::default()
    };
    assert_eq!(recipients::active(&d, &by_amount), [3, 1, 2, 4, 0]);
    // Duplicates: case, spacing and trimming do not matter.
    let people = MergeData {
        name: "a.csv".into(),
        fields: vec!["Họ tên".into(), "Email".into()],
        rows: vec![
            vec!["Nguyễn Văn An".into(), "an@x.vn".into()],
            vec!["Trần Bình".into(), "b@x.vn".into()],
            vec!["nguyễn văn  An ".into(), "AN@x.vn".into()],
            vec!["Trần Bình".into(), "khac@x.vn".into()],
        ],
    };
    assert_eq!(recipients::duplicates(&people, &[]), [vec![0, 2]]);
    assert_eq!(
        recipients::duplicates(&people, &["Họ tên".into()]),
        [vec![0, 2], vec![1, 3]]
    );
}
